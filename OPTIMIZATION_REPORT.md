# Performance Optimization Report - ApiTap

## Executive Summary
After analyzing the codebase, I've identified several performance bottlenecks and optimization opportunities. The main areas for improvement are:

1. **PostgreSQL Write Performance** (CRITICAL)
2. **HTTP Streaming & Buffering** (HIGH)
3. **Schema Inference** (MEDIUM)
4. **DataFusion Query Processing** (MEDIUM)

---

## 1. PostgreSQL Write Performance (CRITICAL IMPACT)

### Current Issues:
- **Batch size too small**: Default 100 rows per INSERT
- **Individual INSERTs**: Using parameterized INSERT statements instead of COPY
- **Query rebuild overhead**: Each batch rebuilds the entire SQL string
- **No pipelining**: Batches are written sequentially

### Recommended Optimizations:

#### A. Use PostgreSQL COPY for Bulk Inserts (10-100x faster)
```rust
// Instead of: INSERT INTO table VALUES ($1,$2), ($3,$4)...
// Use: COPY table FROM STDIN WITH (FORMAT CSV)
```
**Expected improvement**: 10-100x faster for large datasets

#### B. Increase Batch Sizes
```rust
// Current: batch_size: 100
// Recommended: 
.with_batch_size(10000)  // For INSERTs
.with_batch_size(50000)  // For COPY
```

#### C. Use Prepared Statements (for INSERT mode)
- Pre-compile SQL once per schema instead of rebuilding each batch
- Reduces parsing overhead

#### D. Pipeline Writes (Async Batching)
- Don't wait for each batch to complete before preparing the next
- Use tokio::spawn for parallel batch processing

---

## 2. HTTP Streaming & Buffering (HIGH IMPACT)

### Current Issues:
```rust
// src/http/fetcher.rs line 1076
let count = Arc::new(Mutex::new(0usize));  // Mutex contention!
```
- Using `Arc<Mutex<>>` for simple counters causes unnecessary locking
- Channel buffer size of 256 may be too small for high-throughput

### Recommended Optimizations:

#### A. Use Atomic Counters
```rust
use std::sync::atomic::{AtomicUsize, Ordering};

let count = Arc::new(AtomicUsize::new(0));
count.fetch_add(1, Ordering::Relaxed);
```
**Expected improvement**: Eliminates mutex contention overhead

#### B. Increase Channel Buffer Size
```rust
// Current: let (tx, mut rx) = tokio::sync::mpsc::channel::<Result<serde_json::Value>>(256);
// Recommended:
let (tx, mut rx) = tokio::sync::mpsc::channel::<Result<serde_json::Value>>(8192);
```

#### C. HTTP Connection Reuse
- Ensure reqwest Client is properly reused (currently using Arc, good!)
- Consider connection pool tuning

---

## 3. Schema Inference (MEDIUM IMPACT)

### Current Implementation:
```rust
// Samples 100 items for schema inference
const MIN_SAMPLES: usize = 100;
```

### Optimizations:
- Current approach is reasonable
- Could reduce to 50 samples for faster startup on large datasets
- Consider caching inferred schemas per source

---

## 4. DataFusion Query Processing (MEDIUM IMPACT)

### Current Issues:
- Creates unique table names per query (nanoid overhead minimal)
- Full table scans for aggregations
- Stream processing good, but could be optimized

### Recommendations:
- Enable DataFusion's streaming execution (already enabled)
- Consider partitioning for very large datasets
- Monitor memory usage for large aggregations

---

## 5. HTTP Fetching Configuration

### Current Defaults:
```rust
pub struct FetchOpts {
    pub concurrency: usize,        // Default not shown
    pub default_page_size: usize,  // Default not shown
    pub fetch_batch_size: usize,   // Default: 256
}
```

### Recommended Tuning:
```rust
FetchOpts {
    concurrency: 10,               // Parallel HTTP requests
    default_page_size: 1000,       // Items per API page
    fetch_batch_size: 1000,        // Batch processing size
}
```

---

## Implementation Priority

### Phase 1 (Immediate - Highest ROI):
1. ✅ Add debug symbols to Cargo.toml (DONE)
2. 🔧 Implement PostgreSQL COPY for bulk inserts
3. 🔧 Replace Arc<Mutex<>> with AtomicUsize
4. 🔧 Increase batch sizes (10x improvement)

### Phase 2 (Next):
5. 🔧 Increase channel buffer sizes
6. 🔧 Add async batch pipelining
7. 🔧 Use prepared statements

### Phase 3 (Polish):
8. 🔧 Add performance metrics/tracing
9. 🔧 Schema caching
10. 🔧 Configuration tuning guide

---

## Profiling Setup

### For Future Profiling:

#### macOS with DTrace:
```bash
# Requires sudo for DTrace
sudo cargo flamegraph --release --root -- -m examples/sql -y examples/config/pipelines.yaml
```

#### Alternative: cargo-instruments (macOS):
```bash
cargo install cargo-instruments
cargo instruments -t time --release -- -m examples/sql -y examples/config/pipelines.yaml
```

#### Linux with perf:
```bash
cargo flamegraph --release -- -m examples/sql -y examples/config/pipelines.yaml
```

---

## Expected Performance Improvements

| Optimization | Expected Gain | Effort |
|-------------|---------------|--------|
| PostgreSQL COPY | 10-100x | High |
| Atomic counters | 2-5% | Low |
| Larger batches | 2-5x | Low |
| Channel buffers | 5-10% | Low |
| Prepared statements | 10-20% | Medium |
| Async pipelining | 20-50% | High |

---

## Monitoring Recommendations

Add these metrics to track performance:
1. Records/second throughput
2. HTTP request latency (p50, p95, p99)
3. Database write latency
4. Memory usage
5. Channel backpressure

Use the existing tracing infrastructure to add spans around critical sections.
