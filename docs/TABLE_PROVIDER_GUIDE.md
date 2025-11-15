# Table Provider Guide (`src/utils/table_provider.rs`)

## Overview

The `table_provider.rs` module implements a custom DataFusion `TableProvider` that enables SQL queries over streaming JSON data. This is the bridge between HTTP data streams and DataFusion's query engine.

---

## Table of Contents

1. [Purpose & Design](#purpose--design)
2. [Architecture](#architecture)
3. [Key Components](#key-components)
4. [How It Works](#how-it-works)
5. [Integration Points](#integration-points)
6. [Implementation Details](#implementation-details)
7. [Usage Examples](#usage-examples)
8. [Best Practices](#best-practices)

---

## Purpose & Design

### What Problem Does It Solve?

**Challenge:** DataFusion expects data from files or databases, but APITap needs to query streaming HTTP responses.

**Solution:** `JsonStreamTableProvider` wraps a stream factory and presents it as a table that DataFusion can query.

### Design Goals

1. **Zero-Copy Streaming** - Data flows through without buffering entire dataset
2. **Lazy Evaluation** - Streams created only when DataFusion needs them
3. **Reusability** - Factory pattern allows multiple query executions
4. **Memory Efficient** - Bounded buffers prevent memory exhaustion
5. **SQL Compatible** - Full DataFusion SQL support over streaming data

---

## Architecture

### High-Level Flow

```
HTTP Stream
     ↓
Stream Factory (Fn() -> Stream)
     ↓
JsonStreamTableProvider (implements TableProvider)
     ↓
DataFusion Context (register_table)
     ↓
SQL Query Execution
     ↓
Results
```

### Component Relationships

```
┌──────────────────────────────────────────────────────┐
│                  DataFusion Context                  │
│                                                      │
│  ctx.sql("SELECT * FROM my_table WHERE x > 10")    │
└────────────────────┬─────────────────────────────────┘
                     │
                     ↓ Queries table "my_table"
┌──────────────────────────────────────────────────────┐
│            JsonStreamTableProvider                   │
│                                                      │
│  • Stores: stream_factory + schema                  │
│  • Implements: TableProvider trait                  │
│  • Creates: Exec (execution plan)                   │
└────────────────────┬─────────────────────────────────┘
                     │
                     ↓ Calls factory
┌──────────────────────────────────────────────────────┐
│              Stream Factory                          │
│                                                      │
│  Arc<dyn Fn() -> Stream<Item = Result<Value>>>     │
│  • Captured: channel receiver                       │
│  • Returns: Stream from buffered HTTP data          │
└────────────────────┬─────────────────────────────────┘
                     │
                     ↓ Reads from
┌──────────────────────────────────────────────────────┐
│           Channel (HTTP Data Buffer)                 │
│                                                      │
│  • Filled by: HTTP response (once)                  │
│  • Read by: Factory (multiple times)                │
│  • Size: Configurable (256-8192 items)              │
└──────────────────────────────────────────────────────┘
```

---

## Key Components

### 1. `JsonStreamTableProvider` Struct

```rust
pub struct JsonStreamTableProvider {
    stream_factory: JsonStreamFactory,  // Creates streams on demand
    schema: SchemaRef,                  // Arrow schema for the data
}
```

**Fields:**
- `stream_factory` - Function that creates new data streams
- `schema` - Arrow schema defining column types and names

**Why These Fields?**
- `stream_factory` is **Arc<Fn() -> Stream>** - cheap to clone, reusable
- `schema` is **SchemaRef** (Arc) - shared across executions

### 2. `JsonStreamFactory` Type Alias

```rust
pub type JsonStreamFactory = Arc<
    dyn Fn() -> Pin<Box<dyn Stream<Item = Result<Value>> + Send>> 
    + Send + Sync
>;
```

**Breakdown:**
- `Arc<...>` - Thread-safe shared ownership
- `dyn Fn()` - Function trait (can be called multiple times)
- `-> Pin<Box<...>>` - Returns pinned boxed stream
- `Stream<Item = Result<Value>>` - Stream of JSON values
- `+ Send + Sync` - Safe to use across threads

### 3. Trait Implementation

```rust
#[async_trait::async_trait]
impl TableProvider for JsonStreamTableProvider
```

DataFusion's `TableProvider` trait requires:
- `schema()` - Return table schema
- `table_type()` - Define table type (View)
- `scan()` - Create execution plan
- `supports_filters_pushdown()` - Declare filter support

---

## How It Works

### Step 1: Creation

```rust
// In src/http/fetcher.rs
let stream_factory = {
    let prefix = Arc::new(Mutex::new(VecDeque::from(samples)));
    let rx_arc = Arc::new(Mutex::new(rx));
    
    move || {
        // This closure IS the factory
        let prefix = Arc::clone(&prefix);
        let rx_arc = Arc::clone(&rx_arc);
        
        async_stream::stream! {
            // Yield buffered samples first
            while let Some(v) = prefix.lock().await.pop_front() {
                yield Ok(v);
            }
            
            // Then read from channel
            let mut r = rx_arc.lock().await;
            while let Some(item) = r.recv().await {
                yield item;
            }
        }.boxed()
    }
};

// Create table provider
let table_provider = JsonStreamTableProvider::new(
    Arc::new(stream_factory),
    schema
);
```

### Step 2: Registration

```rust
// Register with DataFusion
ctx.register_table("my_table", Arc::new(table_provider))?;
```

**What Happens:**
- Table name "my_table" → provider mapping stored
- Provider holds factory (Arc) and schema
- **No data has been streamed yet!**

### Step 3: Query Execution

```rust
// User executes SQL
let df = ctx.sql("SELECT * FROM my_table WHERE x > 10").await?;
```

**DataFusion Internal Flow:**
1. Parse SQL → Logical Plan
2. Optimize → Optimized Logical Plan
3. Convert → Physical Plan
4. **Call `TableProvider::scan()`** ← This is where our code runs!

### Step 4: Scan Method

```rust
async fn scan(
    &self,
    _state: &dyn Session,
    projection: Option<&Vec<usize>>,  // Which columns needed
    filters: &[Expr],                  // WHERE clauses
    limit: Option<usize>,              // LIMIT clause
) -> datafusion::error::Result<Arc<dyn ExecutionPlan>> {
    // Log filter/limit pushdowns
    if !filters.is_empty() {
        tracing::debug!(filters = ?filters, "filters pushed down");
    }
    
    // Create Exec (execution plan)
    let exec = Exec::new(self.schema.clone(), projection, {
        let factory = self.stream_factory.clone();  // Clone Arc (cheap!)
        move || factory()  // Closure that calls factory
    })?;
    
    Ok(Arc::new(exec))
}
```

**Key Points:**
- `projection` - Only requested columns (e.g., SELECT id, name)
- `filters` - WHERE conditions (not pushed down in our impl)
- Returns `Exec` - Custom execution plan
- **Factory is cloned (Arc), not called yet!**

### Step 5: Factory Closure Pattern

```rust
{
    let factory = self.stream_factory.clone();  // Clone Arc pointer (8 bytes)
    move || factory()  // New closure captures factory
}
```

**Why This Pattern?**

```rust
// What we COULD do (doesn't work):
Exec::new(schema, projection, self.stream_factory)?;
// Problem: self.stream_factory is Arc<Fn() -> Stream>
// But Exec expects F: Fn() -> Stream

// What we DO (works):
Exec::new(schema, projection, { 
    let factory = self.stream_factory.clone();
    move || factory()  
})?;
// This creates a NEW closure: Fn() -> Stream
// That internally calls: Arc<Fn() -> Stream>
```

**Result:** Exec stores a closure that, when called, calls the factory, which creates a stream from the buffered HTTP data.

---

## Integration Points

### With HTTP Fetcher

```rust
// In src/http/fetcher.rs

// 1. HTTP request completed, data in channel
let (tx, rx) = tokio::sync::mpsc::channel(8192);

// 2. Create factory from channel
let stream_factory = create_stream_from_channel(rx);

// 3. Infer schema from samples
let schema = infer_schema_from_values(&samples)?;

// 4. Create table provider
let table_provider = JsonStreamTableProvider::new(
    Arc::new(stream_factory),
    schema
);

// 5. Register with DataFusion
ctx.register_table(table_name, Arc::new(table_provider))?;
```

### With Execution Plan

```rust
// In src/utils/execution.rs

impl ExecutionPlan for Exec {
    fn execute(&self, ...) -> Result<SendableRecordBatchStream> {
        // Call factory to get stream
        let json_stream = (self.stream_factory)();
        //                 ^^^^^^^^^^^^^^^^^^^^
        //                 Factory was stored by TableProvider
        
        // Convert JSON → Arrow RecordBatches
        let batch_stream = stream_json_to_batches(json_stream, ...);
        
        Ok(batch_stream)
    }
}
```

### With DataFusion Query Engine

```
SQL Query
    ↓
Logical Plan: Scan(table="my_table", projection=[0,1], filter=...)
    ↓
Physical Plan: Exec(schema, factory)
    ↓
Execute: factory() → stream → RecordBatches
    ↓
Results
```

---

## Implementation Details

### Trait Methods

#### 1. `as_any(&self) -> &dyn Any`

```rust
fn as_any(&self) -> &dyn Any {
    self
}
```

**Purpose:** Type downcasting for DataFusion internals.

#### 2. `schema(&self) -> SchemaRef`

```rust
fn schema(&self) -> SchemaRef {
    self.schema.clone()  // Arc clone - cheap
}
```

**Purpose:** Return table schema to DataFusion.

#### 3. `table_type(&self) -> TableType`

```rust
fn table_type(&self) -> TableType {
    TableType::View
}
```

**Purpose:** Indicate this is a view (not a base table).

**Why View?**
- Data is computed/streamed, not stored
- Ephemeral - exists only during query execution
- No persistence or indexing

#### 4. `scan(...) -> Result<Arc<dyn ExecutionPlan>>`

```rust
async fn scan(
    &self,
    _state: &dyn Session,
    projection: Option<&Vec<usize>>,
    filters: &[Expr],
    limit: Option<usize>,
) -> datafusion::error::Result<Arc<dyn ExecutionPlan>>
```

**Purpose:** Create execution plan for query.

**Parameters Explained:**
- `_state` - Session context (unused in our impl)
- `projection` - Column indices to include (e.g., [0, 2] for 1st and 3rd columns)
- `filters` - WHERE clause expressions
- `limit` - LIMIT value if present

**Return:** Arc<Exec> - Custom execution plan

#### 5. `supports_filters_pushdown(...)`

```rust
fn supports_filters_pushdown(
    &self,
    _filters: &[&Expr],
) -> datafusion::error::Result<Vec<TableProviderFilterPushDown>> {
    Ok(vec![])  // No filters pushed down
}
```

**Purpose:** Tell DataFusion which filters we can handle.

**Current Implementation:** Returns empty vector (no pushdown support).

**Potential Enhancement:**
```rust
// Could implement for simple filters
Ok(filters.iter().map(|_| {
    TableProviderFilterPushDown::Inexact
}).collect())
```

---

## Usage Examples

### Example 1: Basic Usage

```rust
use apitap::utils::table_provider::JsonStreamTableProvider;

// 1. Have a stream factory
let factory = Arc::new(move || {
    let data = vec![
        json!({"id": 1, "name": "Alice"}),
        json!({"id": 2, "name": "Bob"}),
    ];
    Box::pin(futures::stream::iter(data.into_iter().map(Ok)))
});

// 2. Define schema
let schema = Arc::new(arrow::datatypes::Schema::new(vec![
    Field::new("id", DataType::Int64, false),
    Field::new("name", DataType::Utf8, false),
]));

// 3. Create provider
let provider = JsonStreamTableProvider::new(factory, schema);

// 4. Register with DataFusion
ctx.register_table("users", Arc::new(provider))?;

// 5. Query it!
let df = ctx.sql("SELECT * FROM users WHERE id > 1").await?;
let results = df.collect().await?;
```

### Example 2: With HTTP Stream

```rust
// In fetcher.rs, after HTTP request
async fn write_page_stream(&self, json_stream: ...) -> Result<()> {
    // Buffer data
    let (tx, rx) = tokio::sync::mpsc::channel(8192);
    
    tokio::spawn(async move {
        while let Some(item) = json_stream.next().await {
            tx.send(item).await;
        }
    });
    
    // Sample for schema
    let mut samples = vec![];
    for _ in 0..100 {
        if let Some(Ok(v)) = rx.recv().await {
            samples.push(v);
        }
    }
    
    let schema = infer_schema_from_values(&samples)?;
    
    // Create factory
    let factory = {
        let prefix = Arc::new(Mutex::new(VecDeque::from(samples)));
        let rx_arc = Arc::new(Mutex::new(rx));
        
        Arc::new(move || {
            let p = Arc::clone(&prefix);
            let r = Arc::clone(&rx_arc);
            
            Box::pin(async_stream::stream! {
                // Yield samples
                while let Some(v) = p.lock().await.pop_front() {
                    yield Ok(v);
                }
                
                // Yield channel data
                let mut rx = r.lock().await;
                while let Some(item) = rx.recv().await {
                    yield item;
                }
            })
        })
    };
    
    // Create and register provider
    let provider = JsonStreamTableProvider::new(factory, schema);
    ctx.register_table(table_name, Arc::new(provider))?;
    
    // Now can run SQL!
    let df = ctx.sql(&self.sql).await?;
    let results = df.execute_stream().await?;
    
    // Write results to destination
    self.final_writer.write_stream(results, write_mode).await?;
}
```

### Example 3: Multiple Tables

```rust
// Register multiple streams as different tables
ctx.register_table("users", Arc::new(users_provider))?;
ctx.register_table("orders", Arc::new(orders_provider))?;

// Join them!
let df = ctx.sql(r#"
    SELECT 
        u.name,
        COUNT(o.id) as order_count
    FROM users u
    LEFT JOIN orders o ON u.id = o.user_id
    GROUP BY u.name
"#).await?;
```

---

## Best Practices

### 1. Schema Management

```rust
// ✅ DO: Infer schema from actual data
let schema = infer_schema_from_values(&samples)?;

// ❌ DON'T: Hardcode schema (fragile)
let schema = Arc::new(Schema::new(vec![
    Field::new("id", DataType::Int64, false),
    // Changes if API changes!
]));
```

### 2. Factory Lifetime

```rust
// ✅ DO: Use Arc for factory
let factory = Arc::new(move || create_stream());

// ❌ DON'T: Box alone (can't clone)
let factory = Box::new(move || create_stream());
```

### 3. Memory Management

```rust
// ✅ DO: Use bounded channels
let (tx, rx) = tokio::sync::mpsc::channel(256);  // Bounded

// ❌ DON'T: Unbounded (memory leak risk)
let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
```

### 4. Error Handling

```rust
// ✅ DO: Propagate errors in stream
async_stream::stream! {
    for item in data {
        match process(item) {
            Ok(v) => yield Ok(v),
            Err(e) => yield Err(e),  // Propagate
        }
    }
}

// ❌ DON'T: Swallow errors
async_stream::stream! {
    for item in data {
        if let Ok(v) = process(item) {
            yield Ok(v);  // Errors disappear!
        }
    }
}
```

### 5. Table Naming

```rust
// ✅ DO: Use SQL-safe names
let table_name = format!("table_{}", unique_id.replace("-", "_"));

// ❌ DON'T: Use special characters
let table_name = "my-table";  // Invalid SQL identifier
```

---

## Performance Considerations

### Memory Footprint

**Per Table Provider:**
- Factory: 8 bytes (Arc pointer)
- Schema: Shared via Arc (minimal overhead)
- **Total:** ~8 bytes per provider!

**Channel Buffers (where memory actually is):**
- 8192 items × ~1KB = ~8 MB (default)
- 256 items × ~1KB = ~256 KB (minimal mode)

### Factory Call Overhead

```rust
// Calling factory is cheap:
// 1. Dereference Arc: O(1)
// 2. Call function pointer: O(1)
// 3. Clone Arc<Mutex<channel>>: O(1)
// 
// Total: Nanoseconds
```

### Stream Creation

```rust
// Factory creates stream: O(1)
// - No HTTP request
// - No disk I/O
// - Just wraps existing channel
```

---

## Common Patterns

### Pattern 1: Single-Use Stream

```rust
// HTTP data → Table → Query → Output
let provider = JsonStreamTableProvider::new(factory, schema);
ctx.register_table("data", Arc::new(provider))?;
let df = ctx.sql("SELECT * FROM data").await?;
let results = df.collect().await?;
// Table can be dropped after use
```

### Pattern 2: Reusable Stream

```rust
// Register once, query multiple times
ctx.register_table("data", Arc::new(provider))?;

// Query 1
let df1 = ctx.sql("SELECT COUNT(*) FROM data").await?;

// Query 2 - factory called again!
let df2 = ctx.sql("SELECT AVG(value) FROM data").await?;
```

### Pattern 3: Temporary Tables

```rust
// Use unique names to avoid conflicts
let unique_name = format!("temp_{}",nanoid::nanoid!(10));
ctx.register_table(&unique_name, Arc::new(provider))?;

let sql = format!("SELECT * FROM {}", unique_name);
let df = ctx.sql(&sql).await?;

// Clean up
ctx.deregister_table(&unique_name)?;
```

---

## Debugging Tips

### Enable Tracing

```rust
// In scan() method
tracing::debug!(
    table = %self.schema.fields().iter()
        .map(|f| f.name().as_str())
        .collect::<Vec<_>>()
        .join(", "),
    projection = ?projection,
    filters = ?filters,
    limit = ?limit,
    "table scan requested"
);
```

### Verify Factory

```rust
// Test that factory works
let stream = (factory)();
let mut count = 0;
while let Some(item) = stream.next().await {
    count += 1;
    if count > 10 { break; }  // Sample first 10
}
assert!(count > 0, "Factory produced no items");
```

### Check Schema

```rust
// Verify schema matches data
let schema = provider.schema();
println!("Schema: {:?}", schema);
for field in schema.fields() {
    println!("  {}: {:?}", field.name(), field.data_type());
}
```

---

## Related Documentation

- [STREAMING_ARCHITECTURE.md](STREAMING_ARCHITECTURE.md) - Overall architecture
- [FETCHER_GUIDE.md](FETCHER_GUIDE.md) - HTTP data fetching
- [EXECUTION_GUIDE.md](EXECUTION_GUIDE.md) - Execution plan details
- [MEMORY_OPTIMIZATION_GUIDE.md](MEMORY_OPTIMIZATION_GUIDE.md) - Memory tuning

---

**Last Updated:** November 15, 2025  
**Module:** `src/utils/table_provider.rs`  
**Complexity:** Medium  
**Lines of Code:** ~90
