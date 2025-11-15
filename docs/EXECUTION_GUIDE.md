# Execution Plan Guide (`src/utils/execution.rs`)

## Overview

The `execution.rs` module implements a custom DataFusion `ExecutionPlan` that converts streaming JSON data into Arrow RecordBatches. This is the final piece that enables SQL queries over HTTP streaming data.

---

## Table of Contents

1. [Purpose & Role](#purpose--role)
2. [Architecture](#architecture)
3. [Key Components](#key-components)
4. [Execution Flow](#execution-flow)
5. [Memory Management](#memory-management)
6. [Implementation Details](#implementation-details)
7. [Performance Characteristics](#performance-characteristics)
8. [Debugging & Monitoring](#debugging--monitoring)

---

## Purpose & Role

### What is an ExecutionPlan?

In DataFusion, an `ExecutionPlan` is the **physical representation** of how to execute a query. It's the compiled, optimized, and ready-to-run version of your SQL.

### The Problem

DataFusion's built-in execution plans work with:
- Files (Parquet, CSV, JSON files)
- Databases (via connectors)
- In-memory tables

But APITap needs to query **streaming HTTP responses** that:
- Arrive incrementally
- Can't be randomly accessed
- Aren't stored on disk
- Come as JSON, not Arrow format

### The Solution

`Exec` is a custom `ExecutionPlan` that:
1. Accepts a stream factory (produces JSON streams)
2. Converts JSON → Arrow RecordBatches on-the-fly
3. Integrates seamlessly with DataFusion
4. Enables true streaming execution

---

## Architecture

### Position in the Stack

```
┌──────────────────────────────────────────────────────┐
│            DataFusion Query Engine                   │
│  SQL → Logical Plan → Physical Plan → Execute       │
└────────────────────┬─────────────────────────────────┘
                     │
                     ↓ execute()
┌──────────────────────────────────────────────────────┐
│                 Exec (Custom ExecutionPlan)          │
│                                                      │
│  • Stores: stream_factory + schema                  │
│  • Implements: ExecutionPlan trait                  │
│  • Converts: JSON → RecordBatches                   │
└────────────────────┬─────────────────────────────────┘
                     │
                     ↓ Calls factory
┌──────────────────────────────────────────────────────┐
│              Stream Factory                          │
│  Returns: Stream<Item = Result<Value>>             │
└────────────────────┬─────────────────────────────────┘
                     │
                     ↓ Converts
┌──────────────────────────────────────────────────────┐
│         stream_json_to_batches()                     │
│  JSON Stream → RecordBatch Stream                   │
└────────────────────┬─────────────────────────────────┘
                     │
                     ↓ Returns
┌──────────────────────────────────────────────────────┐
│        SendableRecordBatchStream                     │
│  DataFusion can consume this!                       │
└──────────────────────────────────────────────────────┘
```

### Data Transformation Flow

```
JSON Value
{"id": 1, "name": "Alice", "age": 30}
         ↓
Arrow Array (per column)
id_array: [1]
name_array: ["Alice"]
age_array: [30]
         ↓
RecordBatch (collection of columns)
+----+-------+-----+
| id | name  | age |
+----+-------+-----+
| 1  | Alice | 30  |
+----+-------+-----+
         ↓
Stream of RecordBatches
[Batch1, Batch2, Batch3, ...]
         ↓
DataFusion Query Results
```

---

## Key Components

### 1. `Exec` Struct

```rust
pub struct Exec {
    stream_factory: JsonStreamFactory,  // Creates JSON streams
    pub projected_schema: SchemaRef,    // Output schema
    pub cache: PlanProperties,          // Plan metadata
}
```

**Fields:**
- `stream_factory` - Factory that creates JSON value streams
- `projected_schema` - Arrow schema (after column projection)
- `cache` - Cached plan properties (partitioning, ordering, etc.)

### 2. `JsonStreamFactory` Type

```rust
pub type JsonStreamFactory = Arc<
    dyn Fn() -> Pin<Box<dyn Stream<Item = Result<Value>> + Send>> 
    + Send + Sync
>;
```

Same type used in `TableProvider` - enables factory reuse.

### 3. Plan Properties

```rust
pub struct PlanProperties {
    eq_properties: EquivalenceProperties,  // Column equivalences
    partitioning: Partitioning,            // Data partitioning strategy
    emission_type: EmissionType,           // Streaming or batch
    boundedness: Boundedness,              // Bounded or unbounded
}
```

---

## Execution Flow

### DataFusion Execution Lifecycle

```
1. SQL Parsing
   "SELECT * FROM table WHERE x > 10"
         ↓
2. Logical Planning
   Scan(table) → Filter(x > 10) → Projection(*)
         ↓
3. Optimization
   Filter pushdown, projection pruning, etc.
         ↓
4. Physical Planning
   TableProvider::scan() creates Exec
         ↓
5. Execution (OUR CODE RUNS HERE!)
   Exec::execute() called
         ↓
6. Stream Processing
   RecordBatches flow through operators
         ↓
7. Results
   Final output to user
```

### Detailed execute() Flow

```rust
impl ExecutionPlan for Exec {
    fn execute(
        &self,
        _partition: usize,
        _context: Arc<TaskContext>,
    ) -> Result<SendableRecordBatchStream> {
        // Step 1: Get schema and factory
        let schema = self.projected_schema.clone();
        let stream_factory = self.stream_factory.clone();
        let schema_c = schema.clone();

        // Step 2: Create async stream
        let record_batch_stream = async_stream::try_stream! {
            // Step 2a: Call factory to get JSON stream
            let json_stream = (stream_factory)();
            
            // Step 2b: Convert JSON → RecordBatches
            let batch_stream = streaming::stream_json_to_batches(
                json_stream,
                schema_c.clone(),
                StreamConfig {
                    batch_size: 256,
                    max_buffered_items: 512,
                    true_streaming: true,
                },
            ).await?;
            
            // Step 2c: Yield each RecordBatch
            let mut pinned = std::pin::pin!(batch_stream);
            while let Some(batch) = pinned.next().await {
                yield batch?;
            }
        };

        // Step 3: Wrap in adapter
        let adapter = RecordBatchStreamAdapter::new(
            schema,
            Box::pin(record_batch_stream)
        );
        
        // Step 4: Return to DataFusion
        Ok(Box::pin(adapter))
    }
}
```

### Key Points

1. **Factory Called Once Per Partition** - For 1 partition, factory called once
2. **Stream Created Lazily** - Only when execute() is called
3. **True Streaming** - No intermediate buffering
4. **Error Propagation** - Errors bubble up to DataFusion
5. **Backpressure** - Downstream consumers control flow

---

## Memory Management

### Stream Configuration

```rust
StreamConfig {
    batch_size: 256,           // Rows per RecordBatch
    max_buffered_items: 512,   // Max JSON items buffered
    true_streaming: true,      // Don't buffer entire dataset
}
```

**Memory Impact:**

```
batch_size: 256 rows
row_size: ~1 KB (average)
RecordBatch size: ~256 KB

max_buffered_items: 512
buffer_size: ~512 KB

Total: ~768 KB per execution
```

### For High Concurrency

```rust
// Option 1: Reduce batch size
StreamConfig {
    batch_size: 128,           // Half the rows
    max_buffered_items: 256,   // Half the buffer
    true_streaming: true,
}
// Memory: ~384 KB per execution

// Option 2: Ultra minimal
StreamConfig {
    batch_size: 64,
    max_buffered_items: 128,
    true_streaming: true,
}
// Memory: ~192 KB per execution
```

### Memory Per Concurrent Query

| Concurrent Queries | Config | Memory per Query | Total Memory |
|--------------------|--------|------------------|--------------|
| 1 | Default | 768 KB | 768 KB |
| 10 | Default | 768 KB | 7.68 MB |
| 100 | Reduced | 384 KB | 38.4 MB |
| 1000 | Minimal | 192 KB | 192 MB |

---

## Implementation Details

### ExecutionPlan Trait Methods

#### 1. `schema() -> SchemaRef`

```rust
fn schema(&self) -> SchemaRef {
    self.projected_schema.clone()
}
```

Returns the output schema after column projection.

#### 2. `properties() -> &PlanProperties`

```rust
fn properties(&self) -> &PlanProperties {
    &self.cache
}
```

Returns cached plan metadata (partitioning, equivalences, etc.).

#### 3. `execute() -> Result<SendableRecordBatchStream>`

```rust
fn execute(
    &self,
    _partition: usize,
    _context: Arc<TaskContext>,
) -> Result<SendableRecordBatchStream>
```

**The Core Method** - Creates and returns the data stream.

**Parameters:**
- `_partition` - Partition index (0 for single partition)
- `_context` - Task execution context

**Returns:** Stream of RecordBatches

#### 4. `with_new_children()`

```rust
fn with_new_children(
    self: Arc<Self>,
    _children: Vec<Arc<dyn ExecutionPlan>>,
) -> Result<Arc<dyn ExecutionPlan>>
```

For plan tree modifications (our plan has no children).

### Helper: `compute_properties()`

```rust
fn compute_properties(schema: SchemaRef) -> PlanProperties {
    let eq_properties = EquivalenceProperties::new(schema);

    PlanProperties::new(
        eq_properties,
        Partitioning::UnknownPartitioning(1),  // Single partition
        EmissionType::Both,                    // Can emit incrementally or batch
        Boundedness::Bounded,                  // Finite data
    )
}
```

**Metadata Explained:**
- **EquivalenceProperties** - Column relationships (none in our case)
- **UnknownPartitioning(1)** - Single partition, don't know internal structure
- **EmissionType::Both** - Can stream or batch results
- **Boundedness::Bounded** - Stream will end (vs. infinite stream)

---

## Performance Characteristics

### Throughput

```rust
// Factors affecting throughput:
// 1. Batch size - Larger = fewer batches = less overhead
// 2. Buffer size - Larger = less blocking
// 3. JSON parsing - Bottleneck for complex objects
// 4. Arrow conversion - Usually fast

// Example timings (approximate):
// - JSON parse: ~10-50 μs per item
// - Arrow convert: ~5-20 μs per item
// - Total: ~15-70 μs per item
// 
// For 256-item batch:
// - Parsing: ~2.56-12.8 ms
// - Converting: ~1.28-5.12 ms
// - Total: ~3.84-17.92 ms per batch
// 
// Throughput: ~14,000-67,000 items/second
```

### Latency

```rust
// First RecordBatch latency:
// - Factory call: <1 μs
// - Stream creation: <1 μs
// - Fill first batch (256 items): ~4-18 ms
// - Total: ~4-18 ms
//
// Subsequent batches: ~4-18 ms each
```

### Memory

```rust
// Peak memory per execution:
// - JSON items buffer: ~512 KB
// - RecordBatch: ~256 KB
// - Arrow builders: ~256 KB (temporary)
// - Total: ~1 MB peak, ~768 KB sustained
```

---

## Integration with streaming.rs

### The `stream_json_to_batches()` Function

```rust
pub async fn stream_json_to_batches(
    json_stream: Pin<Box<dyn Stream<Item = Result<Value>> + Send>>,
    schema: SchemaRef,
    config: StreamConfig,
) -> Result<Pin<Box<dyn Stream<Item = Result<RecordBatch>> + Send>>>
```

**What It Does:**

1. **Buffer JSON Items** - Accumulates up to `batch_size` items
2. **Build Arrow Arrays** - Converts JSON values → typed Arrow arrays
3. **Create RecordBatch** - Combines arrays into batch
4. **Stream Batches** - Yields batches as they're ready

**Example Flow:**

```
Input JSON Stream:
{"id": 1, "name": "A"}
{"id": 2, "name": "B"}
{"id": 3, "name": "C"}
...

Buffer (batch_size=2):
[{"id": 1, "name": "A"}, {"id": 2, "name": "B"}]
         ↓
Build Arrays:
id_array = Int64Array([1, 2])
name_array = StringArray(["A", "B"])
         ↓
Create RecordBatch:
RecordBatch {
    schema,
    columns: [id_array, name_array]
}
         ↓
Yield to DataFusion
```

---

## Usage Examples

### Example 1: Basic Execution

```rust
use apitap::utils::execution::Exec;

// 1. Create factory
let factory = Arc::new(move || {
    let data = vec![
        json!({"id": 1, "value": 100}),
        json!({"id": 2, "value": 200}),
    ];
    Box::pin(futures::stream::iter(data.into_iter().map(Ok)))
});

// 2. Define schema
let schema = Arc::new(Schema::new(vec![
    Field::new("id", DataType::Int64, false),
    Field::new("value", DataType::Int64, false),
]));

// 3. Create Exec
let exec = Exec::new(schema.clone(), None, move || factory())?;

// 4. Execute
let stream = exec.execute(0, context)?;

// 5. Consume results
while let Some(batch) = stream.next().await {
    let batch = batch?;
    println!("Batch with {} rows", batch.num_rows());
}
```

### Example 2: With Column Projection

```rust
// Only select columns 0 and 2 (id and email, skip name)
let projection = Some(vec![0, 2]);

let exec = Exec::new(
    full_schema,
    projection.as_ref(),  // Project to fewer columns
    move || factory()
)?;

// projected_schema now only has 2 columns
assert_eq!(exec.projected_schema.fields().len(), 2);
```

### Example 3: Error Handling

```rust
let exec = Exec::new(schema, None, move || {
    Box::pin(async_stream::try_stream! {
        for i in 0..10 {
            if i == 5 {
                // Simulate error
                return Err(ApitapError::PipelineError("Oops!".into()));
            }
            yield json!({"id": i});
        }
    })
})?;

let mut stream = exec.execute(0, context)?;
while let Some(result) = stream.next().await {
    match result {
        Ok(batch) => println!("Got batch"),
        Err(e) => {
            eprintln!("Stream error: {}", e);
            break;
        }
    }
}
```

---

## Debugging & Monitoring

### Enable Debug Logging

```rust
// Set environment variable
std::env::set_var("RUST_LOG", "apitap::utils::execution=debug");

// Or use tracing subscriber
tracing_subscriber::fmt()
    .with_max_level(tracing::Level::DEBUG)
    .init();
```

### Monitor Execution

```rust
impl ExecutionPlan for Exec {
    fn execute(&self, partition: usize, _context: Arc<TaskContext>) 
        -> Result<SendableRecordBatchStream> 
    {
        tracing::debug!(
            partition = partition,
            schema_fields = self.projected_schema.fields().len(),
            "executing plan"
        );
        
        let start = std::time::Instant::now();
        let stream = /* ... */;
        
        tracing::debug!(
            setup_time_ms = start.elapsed().as_millis(),
            "execution plan setup complete"
        );
        
        Ok(stream)
    }
}
```

### Profile Performance

```rust
// Count batches and rows
let mut batch_count = 0;
let mut row_count = 0;
let start = std::time::Instant::now();

while let Some(batch) = stream.next().await {
    let batch = batch?;
    batch_count += 1;
    row_count += batch.num_rows();
}

let elapsed = start.elapsed();
println!("Processed {} batches, {} rows in {:?}", 
    batch_count, row_count, elapsed);
println!("Throughput: {:.0} rows/sec", 
    row_count as f64 / elapsed.as_secs_f64());
```

---

## Best Practices

### 1. Choose Appropriate Batch Size

```rust
// Small batches (64-128)
// + Lower latency
// + Less memory
// - More overhead

// Medium batches (256-512)
// + Balanced
// + Good for most cases

// Large batches (1024-2048)
// + Higher throughput
// - Higher latency
// - More memory
```

### 2. Handle Errors Gracefully

```rust
// In stream implementation
let record_batch_stream = async_stream::try_stream! {
    let json_stream = (stream_factory)();
    
    let batch_stream = match streaming::stream_json_to_batches(...).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "failed to create batch stream");
            return Err(DataFusionError::External(e.into()));
        }
    };
    
    // ...
};
```

### 3. Monitor Memory Usage

```rust
// Add memory tracking
#[cfg(feature = "memory-profiling")]
{
    let mem_before = get_current_memory_mb();
    // Execute
    let mem_after = get_current_memory_mb();
    tracing::info!(
        memory_delta_mb = mem_after - mem_before,
        "execution memory usage"
    );
}
```

### 4. Test with Different Data Volumes

```rust
#[cfg(test)]
mod tests {
    // Test with small dataset
    #[tokio::test]
    async fn test_exec_small() {
        let data = vec![json!({"id": 1})];
        // ...
    }
    
    // Test with large dataset
    #[tokio::test]
    async fn test_exec_large() {
        let data: Vec<_> = (0..10000)
            .map(|i| json!({"id": i}))
            .collect();
        // ...
    }
}
```

---

## Common Issues & Solutions

### Issue 1: Out of Memory

**Symptom:** Process killed with OOM

**Solution:**
```rust
// Reduce batch and buffer sizes
StreamConfig {
    batch_size: 64,           // Smaller batches
    max_buffered_items: 128,  // Smaller buffer
    true_streaming: true,
}
```

### Issue 2: Slow Performance

**Symptom:** Low throughput

**Solution:**
```rust
// Increase batch size
StreamConfig {
    batch_size: 1024,          // Larger batches
    max_buffered_items: 2048,  // Larger buffer
    true_streaming: true,
}
```

### Issue 3: High Latency

**Symptom:** First results take long time

**Solution:**
```rust
// Use smaller batches for lower latency
StreamConfig {
    batch_size: 128,  // Results arrive sooner
    max_buffered_items: 256,
    true_streaming: true,
}
```

---

## Related Documentation

- [STREAMING_ARCHITECTURE.md](STREAMING_ARCHITECTURE.md) - Overall architecture
- [TABLE_PROVIDER_GUIDE.md](TABLE_PROVIDER_GUIDE.md) - Table provider details
- [FETCHER_GUIDE.md](FETCHER_GUIDE.md) - HTTP data fetching
- [MEMORY_OPTIMIZATION_GUIDE.md](MEMORY_OPTIMIZATION_GUIDE.md) - Memory tuning

---

**Last Updated:** November 15, 2025  
**Module:** `src/utils/execution.rs`  
**Complexity:** Medium-High  
**Lines of Code:** ~150
