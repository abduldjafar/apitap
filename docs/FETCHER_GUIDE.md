# Fetcher Module Guide (`src/http/fetcher.rs`)

## Overview

The `fetcher.rs` module provides the HTTP data fetching capabilities for APITap, including support for various pagination strategies, streaming data processing, and integration with DataFusion for SQL transformations.

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Key Components](#key-components)
3. [Pagination Strategies](#pagination-strategies)
4. [Data Flow](#data-flow)
5. [Memory Management](#memory-management)
6. [API Reference](#api-reference)
7. [Usage Examples](#usage-examples)
8. [Best Practices](#best-practices)

---

## Architecture Overview

### High-Level Design

```
┌─────────────────────────────────────────────────────────┐
│                   PaginatedFetcher                      │
│                                                         │
│  Manages HTTP requests + pagination + concurrency      │
└──────────────────┬──────────────────────────────────────┘
                   │
                   ├── HTTP Layer (reqwest)
                   │   └─ Handles actual network requests
                   │
                   ├── Pagination Logic
                   │   ├─ LimitOffset
                   │   ├─ PageNumber
                   │   ├─ PageOnly
                   │   └─ Cursor
                   │
                   └── Page Writer (trait)
                       ├─ DataFusionPageWriter
                       └─ Custom implementations
```

### Core Responsibilities

1. **HTTP Request Management** - Execute paginated API requests
2. **Data Streaming** - Stream responses without loading everything into memory
3. **Pagination** - Support multiple pagination patterns
4. **Concurrency** - Fetch multiple pages in parallel
5. **Error Handling** - Graceful error handling with retries
6. **SQL Transformation** - Integrate with DataFusion for data processing

---

## Key Components

### 1. `PaginatedFetcher`

The main struct that orchestrates HTTP fetching and pagination.

```rust
pub struct PaginatedFetcher {
    client: Client,              // HTTP client
    base_url: String,            // API endpoint
    concurrency: usize,          // Max parallel requests
    pagination_config: Pagination, // Pagination strategy
    batch_size: usize,           // Items per batch
}
```

**Key Methods:**

- `new()` - Create a new fetcher
- `with_limit_offset()` - Configure limit/offset pagination
- `with_page_number()` - Configure page number pagination
- `fetch_limit_offset()` - Fetch using limit/offset
- `fetch_page_number()` - Fetch using page numbers

### 2. `Pagination` Enum

Defines supported pagination strategies:

```rust
pub enum Pagination {
    LimitOffset {
        limit_param: String,    // e.g., "limit"
        offset_param: String,   // e.g., "offset"
    },
    PageNumber {
        page_param: String,     // e.g., "page"
        per_page_param: String, // e.g., "per_page"
    },
    PageOnly {
        page_param: String,     // e.g., "page"
    },
    Cursor {
        cursor_param: String,        // e.g., "cursor"
        page_size_param: Option<String>, // e.g., "size"
    },
    Default,
}
```

### 3. `PageWriter` Trait

Interface for processing fetched data:

```rust
#[async_trait]
pub trait PageWriter: Send + Sync {
    // Write a batch of data
    async fn write_page(
        &self,
        page_number: u64,
        data: Vec<Value>,
        write_mode: WriteMode,
    ) -> Result<()>;

    // Write streaming data
    async fn write_page_stream(
        &self,
        stream_data: Pin<Box<dyn Stream<Item = Result<Value>> + Send>>,
        write_mode: WriteMode,
    ) -> Result<()>;

    // Handle errors
    async fn on_page_error(&self, page_number: u64, error: String) -> Result<()>;

    // Transaction management
    async fn begin(&self) -> Result<()>;
    async fn commit(&self) -> Result<()>;
}
```

### 4. `DataFusionPageWriter`

Concrete implementation that applies SQL transformations:

```rust
pub struct DataFusionPageWriter {
    table_name: String,         // Table name for SQL
    sql: String,                // SQL query to apply
    final_writer: Arc<dyn DataWriter>, // Destination writer
}
```

---

## Pagination Strategies

### 1. Limit/Offset Pagination

**Pattern:** `GET /api/items?limit=100&offset=0`

```rust
let fetcher = PaginatedFetcher::new(client, "https://api.example.com/items", 5)
    .with_limit_offset("limit", "offset");

fetcher.fetch_limit_offset(
    100,          // limit
    Some("/data"), // JSON path
    None,         // total_hint
    writer,       // PageWriter
    write_mode,   // append/replace
    &retry_config
).await?;
```

**How It Works:**
1. Fetches page 1: `?limit=100&offset=0`
2. Fetches page 2: `?limit=100&offset=100`
3. Continues until empty page received
4. Pages can be fetched concurrently

### 2. Page Number Pagination

**Pattern:** `GET /api/items?page=1&per_page=50`

```rust
let fetcher = PaginatedFetcher::new(client, "https://api.example.com/items", 10)
    .with_page_number("page", "per_page");

fetcher.fetch_page_number(
    50,           // per_page
    Some("/results"), // JSON path
    Some(TotalHint::Pages { pointer: "/total_pages" }),
    writer,
    write_mode,
    &retry_config
).await?;
```

**How It Works:**
1. Fetches page 1 to get total pages
2. Fetches remaining pages concurrently
3. Uses `total_pages` or `total_items` from response

### 3. Streaming Integration

All pagination methods support streaming:

```rust
// Creates a continuous stream across all pages
let stream = fetcher.limit_offset_stream(100, Some("/data"), &retry_config).await?;

// Stream items from all pages
while let Some(item) = stream.next().await {
    let value = item?;
    // Process item
}
```

---

## Data Flow

### Complete Request Flow

```
┌─────────────────────────────────────────────────────────┐
│ 1. Configuration                                        │
│    - Base URL, pagination params                        │
│    - Concurrency limits                                 │
│    - Retry configuration                                │
└──────────────────┬──────────────────────────────────────┘
                   ↓
┌─────────────────────────────────────────────────────────┐
│ 2. HTTP Request (via ndjson_stream_qs)                 │
│    - Execute GET request                                │
│    - Handle NDJSON or JSON response                     │
│    - Stream response body                               │
└──────────────────┬──────────────────────────────────────┘
                   ↓
┌─────────────────────────────────────────────────────────┐
│ 3. Data Extraction                                      │
│    - Apply JSON pointer (e.g., /data)                   │
│    - Handle arrays vs objects                           │
│    - Stream individual items                            │
└──────────────────┬──────────────────────────────────────┘
                   ↓
┌─────────────────────────────────────────────────────────┐
│ 4. Buffer in Channel                                    │
│    - Channel buffer: 8192 items (configurable)          │
│    - Sample first 100 for schema                        │
│    - Stream rest to DataFusion                          │
└──────────────────┬──────────────────────────────────────┘
                   ↓
┌─────────────────────────────────────────────────────────┐
│ 5. DataFusion Processing                                │
│    - Register table with stream factory                 │
│    - Execute SQL transformation                         │
│    - Convert to RecordBatches                           │
└──────────────────┬──────────────────────────────────────┘
                   ↓
┌─────────────────────────────────────────────────────────┐
│ 6. Write to Destination                                 │
│    - PostgreSQL (batch inserts)                         │
│    - File output                                        │
│    - Custom writers                                     │
└─────────────────────────────────────────────────────────┘
```

### Memory Flow (Critical for High Concurrency)

```
HTTP Response
      ↓
Streaming Reader (tokio_util::StreamReader)
      ↓
Line-by-Line (FramedRead + LinesCodec)
      ↓
JSON Parse (per line)
      ↓
Channel Buffer (8192 items) ← MEMORY BOTTLENECK!
      ↓
Schema Sampling (100 items)
      ↓
Stream Factory (Arc<Mutex<channel>>)
      ↓
DataFusion Table Provider
      ↓
SQL Execution
      ↓
Writer (batched)
```

**Memory Optimization:**
- Current: 8192-item buffer = ~8 MB per pipeline
- Minimal: 256-item buffer = ~256 KB per pipeline
- For 1000 pipelines: 256 MB vs 8 GB

---

## Memory Management

### Current Implementation

```rust
async fn write_page_stream(&self, json_stream: ...) -> Result<()> {
    // 1. Create bounded channel
    let (tx, mut rx) = tokio::sync::mpsc::channel(8192);
    //                                             ^^^^
    //                                    CONFIGURABLE!
    
    // 2. Spawn task to consume HTTP stream (once)
    let _stream_task = tokio::spawn(async move {
        while let Some(item) = json_stream.next().await {
            tx.send(item).await; // Bounded - backpressure!
        }
    });
    
    // 3. Sample for schema (first 100 items)
    let mut samples = Vec::new();
    while samples.len() < 100 {
        if let Some(Ok(v)) = rx.recv().await {
            samples.push(v);
        }
    }
    
    // 4. Create stream factory from channel
    let stream_factory = {
        let prefix = Arc::new(Mutex::new(VecDeque::from(samples)));
        let rx_arc = Arc::new(Mutex::new(rx));
        
        move || {
            // Factory creates stream from buffered channel
            // NOT new HTTP requests!
            create_stream_from_channel(prefix.clone(), rx_arc.clone())
        }
    };
    
    // 5. Register with DataFusion
    ctx.register_table(table_name, JsonStreamTableProvider::new(
        Arc::new(stream_factory),
        schema
    ))?;
}
```

### Memory Optimization Strategies

**For High Concurrency (1000+ pipelines):**

```rust
// Option 1: Environment-based configuration
let buffer_size = std::env::var("APITAP_CHANNEL_BUFFER")
    .unwrap_or("256".to_string())
    .parse()
    .unwrap_or(256);

let (tx, rx) = tokio::sync::mpsc::channel(buffer_size);

// Option 2: Adaptive based on concurrency
fn adaptive_buffer_size(concurrent_pipelines: usize) -> usize {
    match concurrent_pipelines {
        0..=10 => 8192,   // High throughput
        11..=100 => 1024,  // Balanced
        101..=1000 => 256, // Memory efficient
        _ => 64,          // Ultra minimal
    }
}
```

---

## API Reference

### `ndjson_stream_qs()`

Stream an HTTP response as NDJSON with optional JSON pointer extraction.

```rust
pub async fn ndjson_stream_qs(
    client: &reqwest::Client,
    url: &str,
    query: &[(String, String)],
    data_path: Option<&str>,
    config_retry: &crate::pipeline::Retry,
) -> Result<BoxStream<'static, Result<Value>>>
```

**Parameters:**
- `client` - HTTP client with retry configuration
- `url` - API endpoint URL
- `query` - Query parameters (e.g., page, limit)
- `data_path` - JSON pointer to data array (e.g., "/data", "/results")
- `config_retry` - Retry configuration

**Returns:** Stream of JSON values

**Behavior:**
- Detects NDJSON vs JSON from Content-Type header
- For JSON: Parses entire response, extracts array
- For NDJSON: Streams line-by-line
- Applies JSON pointer if provided

### `PaginatedFetcher::fetch_limit_offset()`

Fetch data using limit/offset pagination.

```rust
pub async fn fetch_limit_offset(
    &self,
    limit: u64,
    data_path: Option<&str>,
    _total_hint: Option<TotalHint>,
    writer: Arc<dyn PageWriter>,
    write_mode: WriteMode,
    config_retry: &crate::pipeline::Retry,
) -> Result<FetchStats>
```

**Algorithm:**
1. Create continuous stream across pages
2. Starts with offset=0
3. Fetches limit items per page
4. Increments offset by limit
5. Stops when page returns 0 items

### `PaginatedFetcher::fetch_page_number()`

Fetch data using page number pagination.

```rust
pub async fn fetch_page_number(
    &self,
    per_page: u64,
    data_path: Option<&str>,
    total_hint: Option<TotalHint>,
    writer: Arc<dyn PageWriter>,
    write_mode: WriteMode,
    config_retry: &crate::pipeline::Retry,
) -> Result<FetchStats>
```

**Algorithm:**
1. Fetch page 1 to determine total pages
2. If total known, fetch pages 2..N concurrently
3. If total unknown, fetch sequentially until empty

---

## Usage Examples

### Example 1: Basic Limit/Offset Fetch

```rust
use apitap::http::fetcher::{PaginatedFetcher, DataFusionPageWriter};

// 1. Create HTTP client
let client = reqwest::Client::new();

// 2. Create fetcher with limit/offset
let fetcher = PaginatedFetcher::new(client, "https://api.example.com/users", 5)
    .with_limit_offset("limit", "offset")
    .with_batch_size(256);

// 3. Create writer
let writer = Arc::new(DataFusionPageWriter::new(
    "users",
    "SELECT * FROM users WHERE age > 18",
    db_writer
));

// 4. Fetch all pages
let stats = fetcher.fetch_limit_offset(
    100,    // 100 items per page
    Some("/data"), // Extract from /data JSON path
    None,   // Auto-detect when done
    writer,
    WriteMode::Append,
    &retry_config
).await?;

println!("Fetched {} items", stats.total_items);
```

### Example 2: Page Number with Total Hint

```rust
let fetcher = PaginatedFetcher::new(client, "https://api.example.com/posts", 10)
    .with_page_number("page", "per_page");

let stats = fetcher.fetch_page_number(
    50,     // 50 items per page
    Some("/results"),
    Some(TotalHint::Items {
        pointer: "/pagination/total".to_string()
    }),
    writer,
    WriteMode::Replace,
    &retry_config
).await?;
```

### Example 3: Streaming Without Writer

```rust
// Get raw stream of items
let stream = fetcher.limit_offset_stream(
    100,
    Some("/data"),
    &retry_config
).await?;

// Process items as they arrive
while let Some(result) = stream.next().await {
    match result {
        Ok(value) => {
            // Process value
            println!("{:?}", value);
        },
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }
}
```

### Example 4: Custom Page Writer

```rust
struct CustomWriter {
    // Your fields
}

#[async_trait]
impl PageWriter for CustomWriter {
    async fn write_page(
        &self,
        page_number: u64,
        data: Vec<Value>,
        _write_mode: WriteMode,
    ) -> Result<()> {
        // Custom processing
        for item in data {
            // Do something with item
        }
        Ok(())
    }
    
    async fn write_page_stream(
        &self,
        stream_data: Pin<Box<dyn Stream<Item = Result<Value>> + Send>>,
        _write_mode: WriteMode,
    ) -> Result<()> {
        // Stream processing
        let mut stream = stream_data;
        while let Some(item) = stream.next().await {
            // Process item
        }
        Ok(())
    }
}
```

---

## Best Practices

### 1. Choose Appropriate Concurrency

```rust
// Low: More sequential, less memory
let fetcher = PaginatedFetcher::new(client, url, 2);

// Medium: Balanced
let fetcher = PaginatedFetcher::new(client, url, 5);

// High: More parallel, more memory
let fetcher = PaginatedFetcher::new(client, url, 20);
```

**Guidelines:**
- **1-5**: For rate-limited APIs
- **5-10**: For most use cases
- **10-20**: For high-throughput scenarios
- **>20**: Only if API and network can handle it

### 2. Configure Batch Size

```rust
// Small batches: Lower memory, more overhead
let fetcher = fetcher.with_batch_size(100);

// Medium batches: Balanced
let fetcher = fetcher.with_batch_size(256);

// Large batches: Higher memory, less overhead
let fetcher = fetcher.with_batch_size(1000);
```

### 3. Use JSON Pointers Correctly

```rust
// Extract from nested structure
let stream = ndjson_stream_qs(
    &client,
    url,
    &[],
    Some("/data/items"),  // Response: {"data": {"items": [...]}}
    &retry_config
).await?;

// No extraction needed
let stream = ndjson_stream_qs(
    &client,
    url,
    &[],
    None,  // Response is already an array
    &retry_config
).await?;
```

### 4. Handle Errors Gracefully

```rust
impl PageWriter for MyWriter {
    async fn on_page_error(&self, page: u64, error: String) -> Result<()> {
        // Log error
        error!("Page {} failed: {}", page, error);
        
        // Store for retry
        self.failed_pages.lock().await.push(page);
        
        // Don't propagate - continue with other pages
        Ok(())
    }
}
```

### 5. Monitor Memory Usage

```rust
// For high concurrency scenarios
async fn write_page_stream(&self, stream: ...) -> Result<()> {
    // Use smaller buffer
    let buffer_size = if concurrent_pipelines > 100 {
        256  // Minimal
    } else {
        8192 // Default
    };
    
    let (tx, rx) = tokio::sync::mpsc::channel(buffer_size);
    // ...
}
```

---

## Performance Considerations

### Throughput vs Memory Trade-offs

| Configuration | Throughput | Memory (per pipeline) | Use Case |
|---------------|------------|----------------------|----------|
| Buffer: 8192, Batch: 5000 | 100% | ~18 MB | Single large pipeline |
| Buffer: 1024, Batch: 2000 | 85% | ~3 MB | 10-100 pipelines |
| Buffer: 256, Batch: 500 | 70% | ~1.5 MB | 1000+ pipelines |

### Optimization Tips

1. **Reduce buffer size** for high concurrency
2. **Increase concurrency** for faster completion
3. **Use NDJSON** when available (less memory than JSON)
4. **Monitor channel saturation** - if always full, increase size
5. **Profile with flamegraph** to identify bottlenecks

---

## Related Documentation

- [STREAMING_ARCHITECTURE.md](STREAMING_ARCHITECTURE.md) - Overall streaming architecture
- [TABLE_PROVIDER_GUIDE.md](TABLE_PROVIDER_GUIDE.md) - DataFusion integration
- [EXECUTION_GUIDE.md](EXECUTION_GUIDE.md) - Execution plan details
- [MEMORY_OPTIMIZATION_GUIDE.md](MEMORY_OPTIMIZATION_GUIDE.md) - Memory tuning

---

**Last Updated:** November 15, 2025  
**Module:** `src/http/fetcher.rs`  
**Complexity:** Medium-High
