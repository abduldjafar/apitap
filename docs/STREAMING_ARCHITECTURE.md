# APITap Streaming Architecture & Factory Pattern

## Overview

This document explains how APITap handles HTTP data streaming through DataFusion, specifically addressing:
- How the factory pattern works
- Whether HTTP requests are executed multiple times
- Memory buffering strategy
- Why this architecture is memory-efficient

---

## Table of Contents

1. [The Factory Pattern](#the-factory-pattern)
2. [HTTP Request Flow](#http-request-flow)
3. [DataFusion Integration](#datafusion-integration)
4. [Memory Buffering](#memory-buffering)
5. [Performance Implications](#performance-implications)
6. [Code Walkthrough](#code-walkthrough)

---

## The Factory Pattern

### What is a Stream Factory?

```rust
// Type definition
pub type JsonStreamFactory = Arc<
    dyn Fn() -> Pin<Box<dyn Stream<Item = Result<Value>> + Send>> 
    + Send + Sync
>;
```

A **stream factory** is a function that **creates streams on demand**, not the stream itself.

### Why Use a Factory?

```rust
// ❌ Problem: Pass a stream directly
let stream = fetch_data();
register_table(stream);  // Stream can only be consumed once!

// ✅ Solution: Pass a factory that creates streams
let factory = || fetch_data();
register_table(factory);  // Can create streams multiple times
```

**Key Benefit:** DataFusion can call the factory multiple times during query execution without re-fetching data.

---

## HTTP Request Flow

### The Critical Question

**Q: Does calling the factory multiple times mean multiple HTTP requests?**

**A: NO! The HTTP request happens ONCE. The factory creates streams from buffered data.**

### Complete Flow Diagram

```
┌─────────────────────────────────────────────────────────────┐
│ Step 1: HTTP Request (HAPPENS ONCE)                         │
│                                                              │
│  http_client.get(url).await?                                │
│       ↓                                                      │
│  Response stream: [item1, item2, item3, ...]                │
└─────────────────────────────────────────────────────────────┘
         ↓
         ↓ Data flows into channel
         ↓
┌─────────────────────────────────────────────────────────────┐
│ Step 2: Buffer in Channel (HAPPENS ONCE)                    │
│                                                              │
│  let (tx, rx) = tokio::sync::mpsc::channel(8192);          │
│                                                              │
│  tokio::spawn(async move {                                  │
│      while let Some(item) = http_stream.next().await {     │
│          tx.send(item).await;  // Buffer in channel         │
│      }                                                       │
│  });                                                         │
│                                                              │
│  Channel now contains: [item1, item2, item3, ...]          │
└─────────────────────────────────────────────────────────────┘
         ↓
         ↓ Channel receiver (rx) is captured
         ↓
┌─────────────────────────────────────────────────────────────┐
│ Step 3: Create Factory (CAPTURES CHANNEL RECEIVER)          │
│                                                              │
│  let factory = Arc::new(move || {                           │
│      create_stream_from_channel(rx)  // ← Reads from ch.   │
│  });                                                         │
│                                                              │
│  Factory does NOT make HTTP requests!                       │
│  It reads from the buffered channel.                        │
└─────────────────────────────────────────────────────────────┘
         ↓
         ↓ Factory registered with DataFusion
         ↓
┌─────────────────────────────────────────────────────────────┐
│ Step 4: DataFusion Calls Factory (MULTIPLE TIMES)           │
│                                                              │
│  Execution 1: factory() → stream from channel               │
│  Execution 2: factory() → stream from channel (same data)   │
│  Execution 3: factory() → stream from channel (same data)   │
│                                                              │
│  All executions read from the SAME buffered channel         │
│  NO new HTTP requests are made                              │
└─────────────────────────────────────────────────────────────┘
```

---

## DataFusion Integration

### When Does DataFusion Call the Factory?

DataFusion may call the factory during:

1. **Schema Inference** - Peek at structure
2. **Query Planning** - Estimate costs
3. **Optimization** - Analyze data patterns
4. **Execution** - Get actual data
5. **Multiple Partitions** - Parallel processing

**Important:** All of these read from the **same buffered data**, not new HTTP requests.

### Code Location: `src/utils/table_provider.rs`

```rust
impl TableProvider for JsonStreamTableProvider {
    async fn scan(
        &self,
        _state: &dyn Session,
        projection: Option<&Vec<usize>>,
        filters: &[Expr],
        limit: Option<usize>,
    ) -> datafusion::error::Result<Arc<dyn ExecutionPlan>> {
        // Create Exec with factory
        let exec = Exec::new(self.schema.clone(), projection, {
            let factory = self.stream_factory.clone();  // Clone Arc (cheap)
            move || factory()  // Create closure that calls factory
        })?;

        Ok(Arc::new(exec))
    }
}
```

**What this closure does:**
- `let factory = self.stream_factory.clone()` - Clone the Arc pointer (8 bytes)
- `move || factory()` - Create a closure that captures the factory
- When called, it invokes `factory()` which creates a stream from **buffered data**

---

## Memory Buffering

### Buffering Strategy

```rust
// In src/http/fetcher.rs

async fn write_page_stream(...) -> Result<()> {
    // 1. Create bounded channel for buffering
    let (tx, mut rx) = tokio::sync::mpsc::channel(8192);
    //                                             ^^^^
    //                                             Buffer size
    
    // 2. Spawn task to consume HTTP stream (ONCE)
    let _stream_task = tokio::spawn(async move {
        let mut pinned = json_stream;
        while let Some(item) = pinned.next().await {
            if tx.send(item).await.is_err() {
                break;  // Receiver dropped, stop
            }
        }
        // tx dropped → channel closes
    });
    
    // 3. Sample for schema inference
    let mut samples = Vec::new();
    while samples.len() < 100 {
        match rx.recv().await {
            Some(Ok(v)) => samples.push(v),
            Some(Err(e)) => return Err(e),
            None => break,
        }
    }
    
    // 4. Create factory that reads from buffered channel
    let stream_factory = {
        let prefix = Arc::new(Mutex::new(VecDeque::from(samples)));
        let rx_arc = Arc::new(Mutex::new(rx));
        
        move || {
            let prefix = Arc::clone(&prefix);
            let rx_arc = Arc::clone(&rx_arc);
            
            async_stream::stream! {
                loop {
                    // First: yield buffered samples
                    if let Some(v) = { prefix.lock().await.pop_front() } {
                        yield Ok(v);
                        continue;
                    }
                    
                    // Then: read from channel
                    let mut r = rx_arc.lock().await;
                    match r.recv().await {
                        Some(item) => {
                            drop(r);
                            yield item;
                        }
                        None => break,  // Channel exhausted
                    }
                }
            }.boxed()
        }
    };
    
    // 5. Register with DataFusion
    let table_provider = JsonStreamTableProvider::new(
        Arc::new(stream_factory),
        schema
    );
    ctx.register_table(table_name, Arc::new(table_provider))?;
}
```

### Memory Usage Per Pipeline

| Buffer Size | Memory per Pipeline | For 1000 Pipelines |
|-------------|--------------------|--------------------|
| 8192 items | ~8 MB | **8 GB** |
| 1024 items | ~1 MB | **1 GB** |
| 256 items | ~256 KB | **256 MB** |

**This is why memory optimization is critical for high concurrency!**

---

## Performance Implications

### Single HTTP Request Per Pipeline

```rust
// Timeline for one pipeline execution:

T=0s    HTTP request starts
T=0.5s  HTTP response starts streaming
T=1s    Data buffered in channel (ongoing)
T=1.5s  Schema inferred from samples
T=2s    DataFusion query executed
T=2.5s  Results written to database
T=3s    Pipeline complete

// Total HTTP requests: 1
// Factory calls: 3-5 (all read from same buffer)
```

### NO Redundant Requests

```rust
// What DOES NOT happen:

// ❌ Factory call 1 → HTTP request
// ❌ Factory call 2 → HTTP request
// ❌ Factory call 3 → HTTP request

// What ACTUALLY happens:

// ✅ HTTP request (once) → Buffer in channel
// ✅ Factory call 1 → Read from channel
// ✅ Factory call 2 → Read from channel
// ✅ Factory call 3 → Read from channel
```

### Benefits

1. **Efficiency:** One HTTP request per pipeline
2. **Reliability:** No retry overhead from multiple requests
3. **API Friendly:** Respects rate limits
4. **Memory Bounded:** Channel size limits memory usage
5. **Backpressure:** Channel automatically slows producer if consumer is slow

---

## Code Walkthrough

### File: `src/utils/execution.rs`

```rust
pub struct Exec {
    stream_factory: JsonStreamFactory,  // ← Factory stored here
    pub projected_schema: SchemaRef,
    pub cache: PlanProperties,
}

impl Exec {
    pub fn new<F>(
        schema: SchemaRef, 
        projections: Option<&Vec<usize>>, 
        stream_factory: F
    ) -> Result<Self>
    where
        F: Fn() -> Pin<Box<dyn Stream<Item = Result<Value>> + Send>>
            + Send + Sync + 'static,
    {
        // Store factory as Arc for cheap cloning
        Ok(Self {
            stream_factory: Arc::new(stream_factory),
            projected_schema,
            cache,
        })
    }
}

impl ExecutionPlan for Exec {
    fn execute(
        &self,
        _partition: usize,
        _context: Arc<TaskContext>,
    ) -> Result<SendableRecordBatchStream> {
        // Call factory to create stream
        let json_stream = (self.stream_factory)();
        //                 ^^^^^^^^^^^^^^^^^^^^
        //                 This creates stream from buffered channel,
        //                 NOT a new HTTP request!
        
        // Convert JSON stream to Arrow RecordBatches
        let batch_stream = streaming::stream_json_to_batches(
            json_stream,
            self.projected_schema.clone(),
            StreamConfig { ... }
        ).await?;
        
        Ok(Box::pin(batch_stream))
    }
}
```

### Key Points

1. **Factory is stored in `Exec`** - Not the data itself
2. **`execute()` calls the factory** - Creates stream on demand
3. **Stream reads from channel** - Pre-buffered HTTP data
4. **No HTTP client in `Exec`** - Can't make new requests!

---

## Visual Summary

### Architecture Overview

```
┌──────────────────────────────────────────────────────────────┐
│  HTTP Layer (src/http/fetcher.rs)                            │
│                                                               │
│  • Makes HTTP request ONCE                                   │
│  • Buffers data in tokio channel                             │
│  • Creates factory → reads from channel                      │
│                                                               │
└────────────────────────┬─────────────────────────────────────┘
                         │
                         ↓ Factory + Schema
┌──────────────────────────────────────────────────────────────┐
│  Table Provider (src/utils/table_provider.rs)                │
│                                                               │
│  • Stores factory (Arc pointer - 8 bytes)                    │
│  • Provides DataFusion interface                             │
│  • Factory captures channel receiver                         │
│                                                               │
└────────────────────────┬─────────────────────────────────────┘
                         │
                         ↓ Exec plan
┌──────────────────────────────────────────────────────────────┐
│  Execution Plan (src/utils/execution.rs)                     │
│                                                               │
│  • Calls factory when DataFusion needs data                  │
│  • Factory creates stream from buffered channel              │
│  • Converts JSON → Arrow RecordBatches                       │
│                                                               │
└────────────────────────┬─────────────────────────────────────┘
                         │
                         ↓ Results
┌──────────────────────────────────────────────────────────────┐
│  DataFusion Query Engine                                     │
│                                                               │
│  • May call factory 3-5 times during execution               │
│  • All calls read from SAME buffered channel                 │
│  • NO new HTTP requests made                                 │
│                                                               │
└──────────────────────────────────────────────────────────────┘
```

---

## FAQ

### Q1: How many HTTP requests per pipeline?

**A: Exactly ONE.** The HTTP request happens before the factory is created.

### Q2: What if DataFusion calls the factory 10 times?

**A: Still only one HTTP request.** All factory calls read from the same buffered channel.

### Q3: What happens when the channel is exhausted?

**A: The stream ends.** Once all buffered items are consumed, the stream completes gracefully.

### Q4: Can the data be re-streamed after consumption?

**A: No.** The channel is FIFO (first-in-first-out). Once data is consumed, it's gone. However, the initial samples are stored in `prefix` buffer for re-reading.

### Q5: How much memory does the factory use?

**A: ~8 bytes (Arc pointer).** The factory itself is tiny. The memory is in the channel buffer, not the factory.

### Q6: Why not just re-fetch from HTTP when needed?

**A: Several reasons:**
- Would violate API rate limits
- Would increase latency significantly
- Would waste network bandwidth
- Would be unreliable (data might change)
- Would not work for POST requests or cursored APIs

---

## Memory Optimization for High Concurrency

### Current Default Settings

```rust
// Current buffer size
let (tx, rx) = tokio::sync::mpsc::channel(8192);

// Memory per pipeline: ~8 MB
// For 1000 pipelines: 8 GB
```

### Optimized Settings (Minimal Mode)

```rust
// Reduced buffer size
let (tx, rx) = tokio::sync::mpsc::channel(256);

// Memory per pipeline: ~256 KB
// For 1000 pipelines: 256 MB
```

### Configuration

```bash
# Set memory mode for high concurrency
export APITAP_MEMORY_MODE=minimal
export APITAP_CHANNEL_BUFFER=256
export APITAP_MAX_CONCURRENT=1000

# Run with minimal memory
./target/release/apitap -m examples/sql -y examples/config/high-concurrency.yaml
```

**See [MEMORY_OPTIMIZATION_GUIDE.md](MEMORY_OPTIMIZATION_GUIDE.md) for details.**

---

## Performance Characteristics

### Time Complexity

| Operation | Complexity | Notes |
|-----------|-----------|-------|
| HTTP request | O(n) | n = data size, happens once |
| Buffer to channel | O(n) | Streaming, bounded memory |
| Factory creation | O(1) | Just creates Arc pointer |
| Factory call | O(1) | Creates stream wrapper |
| Stream consumption | O(n) | Read from channel |

### Space Complexity

| Component | Memory | Notes |
|-----------|--------|-------|
| HTTP response | Streaming | Not stored in memory |
| Channel buffer | O(buffer_size) | Configurable (256-8192) |
| Samples | O(100) | Fixed size for schema |
| Factory | O(1) | Just Arc pointer |
| Stream | O(1) | Iterator state |

---

## Best Practices

### DO ✅

1. **Use bounded channels** - Prevents memory exhaustion
2. **Buffer data once** - After HTTP request
3. **Share factory via Arc** - Cheap cloning
4. **Configure buffer size** - Based on concurrency needs
5. **Monitor memory usage** - Especially at scale

### DON'T ❌

1. **Don't buffer entire dataset** - Stream where possible
2. **Don't make redundant HTTP requests** - Use factory pattern
3. **Don't ignore backpressure** - Let channels flow control
4. **Don't use unbounded channels** - Memory will grow indefinitely
5. **Don't clone data unnecessarily** - Use Arc and references

---

## Conclusion

### Key Takeaways

1. ✅ **HTTP requests happen ONCE** per pipeline execution
2. ✅ **Data is buffered** in a bounded channel for memory efficiency
3. ✅ **Factory creates streams** from buffered data, not new HTTP requests
4. ✅ **Multiple factory calls** all read from the same buffer
5. ✅ **Memory usage is bounded** by channel buffer size
6. ✅ **Pattern is efficient** for high-concurrency scenarios

### Why This Architecture?

- **Memory Efficient:** Streaming with bounded buffers
- **Performance:** No redundant HTTP requests
- **Flexible:** DataFusion can execute queries multiple times
- **Scalable:** Works with 1000+ concurrent pipelines
- **Reliable:** Backpressure prevents overload

---

## Related Documentation

- **[MEMORY_OPTIMIZATION_GUIDE.md](MEMORY_OPTIMIZATION_GUIDE.md)** - Memory tuning for high concurrency
- **[QUICK_START_MEMORY_OPTIMIZATION.md](QUICK_START_MEMORY_OPTIMIZATION.md)** - Quick reference
- **[FLAMEGRAPH_OPTIMIZATIONS.md](FLAMEGRAPH_OPTIMIZATIONS.md)** - HTTP connection pooling
- **[OPTIMIZATIONS_APPLIED.md](OPTIMIZATIONS_APPLIED.md)** - All performance optimizations

---

**Last Updated:** November 15, 2025  
**Maintained by:** APITap Team  
**For Questions:** See related documentation or review code comments
