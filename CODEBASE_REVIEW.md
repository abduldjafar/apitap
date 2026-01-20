# ApiTap Codebase Review & Deep Dive

## 1. Project Overview

**ApiTap** is a high-performance ETL engine written in Rust. Its primary purpose is to stream JSON data from REST APIs, transform it using SQL (via Apache DataFusion), and load it into a PostgreSQL data warehouse. It is designed to be lightweight yet performant, leveraging async I/O, streaming processing, and optimized database writes.

## 2. Architecture Deep Dive

The architecture follows a classic ETL (Extract, Transform, Load) pattern, but with a focus on streaming and minimizing memory overhead.

### Data Flow
1.  **CLI (`src/cmd`)**: The entry point. It parses arguments, discovers `.sql` transformation modules, and loads the YAML configuration.
2.  **Configuration & Templating (`src/config`)**:
    *   Uses **Minijinja** to render SQL templates.
    *   Captures `{{ sink(...) }}` and `{{ use_source(...) }}` calls to link SQL logic with the YAML configuration.
    *   Parses `pipelines.yaml` to define API sources (URL, pagination, retries) and Database targets.
3.  **Extraction (HTTP Layer - `src/http`)**:
    *   **`PaginatedFetcher`**: The core component that handles HTTP requests.
    *   It supports different pagination strategies (`LimitOffset`, `PageNumber`) to iterate through API pages.
    *   It uses `reqwest` for async HTTP requests and handles retries.
    *   **NDJSON Support**: It can parse newline-delimited JSON or flatten standard JSON arrays into a stream of items.
4.  **Transformation (DataFusion Layer - `src/utils/datafusion_ext.rs`)**:
    *   Instead of loading all data into memory, it uses **Apache DataFusion**.
    *   **`DataFusionPageWriter`** acts as a bridge. It takes a page of fetched data (or a stream), infers its schema, and registers it as a temporary table in DataFusion.
    *   It executes the user's SQL query on this data.
    *   **`JsonStreamTableProvider`**: A custom DataFusion table provider that feeds data from the HTTP stream directly into the SQL engine.
5.  **Loading (Writer Layer - `src/writer`)**:
    *   **`PostgresWriter`**: Receives the transformed data (results of the SQL query) and writes it to PostgreSQL.
    *   It handles table creation (schema inference), `INSERT`, and `MERGE` (Upsert).
    *   It intelligently adapts to the PostgreSQL version (using native `MERGE` for PG 15+ and `INSERT ... ON CONFLICT` for older versions).

## 3. Key Components Analysis

### 3.1. `PaginatedFetcher` (`src/http/fetcher.rs`)
This struct orchestrates the fetching process.
*   **Strengths**: It abstracts away the complexity of pagination. The `fetch_page_number` and `fetch_limit_offset` methods handle the logic of calculating next pages/offsets.
*   **Concurrency**: It uses `buffer_unordered` to fetch pages concurrently when total pages are known, significantly speeding up extraction.
*   **Streaming**: It produces a stream of `Result<Value>`, allowing downstream components to process data as it arrives.

### 3.2. `DataFusionPageWriter` & `JsonStreamTableProvider`
These are the "glue" between HTTP and SQL.
*   **Schema Inference**: `infer_schema_from_values` samples the JSON data to determine the Arrow schema required by DataFusion.
*   **Streaming Execution**: `JsonStreamTableProvider` allows DataFusion to pull data from the HTTP stream on demand. This is crucial for keeping memory usage low. It uses a custom `Exec` plan (`src/utils/execution.rs`) to drive this process.

### 3.3. `PostgresWriter` (`src/writer/postgres.rs`)
This component handles the "Load" phase.
*   **Version Compatibility**: The `merge_batch` function checks the Postgres version and chooses the optimal strategy (`MERGE` vs `ON CONFLICT`). This makes the tool robust across different environments.
*   **Schema Evolution**: It automatically creates tables if they don't exist based on the inferred schema.
*   **Optimization**: It uses `sqlx` and explicit batching (`batch_size = 5000`) to maximize write throughput.

### 3.4. `TrueStreamingProcessor` (`src/utils/streaming.rs`)
This utility ensures "true streaming".
*   It converts a stream of JSON `Value`s directly into Arrow `RecordBatch`es without building a massive intermediate vector of all items. This is key for handling large datasets within limited memory.

## 4. Strengths

1.  **Performance**:
    *   **Async/Await**: Built on `tokio` for efficient I/O handling.
    *   **Zero-Copy Intent**: The architecture strives to minimize copying data, passing streams through the pipeline.
    *   **Optimized Writes**: Large batch sizes and prepared statements in Postgres.
2.  **Flexibility**:
    *   **SQL Transformations**: Users can write standard SQL to clean/restructure data, which is more accessible than writing Rust code.
    *   **Configurable**: YAML config separates environment details (auth, URLs) from logic.
3.  **Modern Stack**: usage of `DataFusion` places it on the bleeding edge of Rust data engineering tools.

## 5. Areas for Improvement / Potential Issues

1.  **Unimplemented Pagination**:
    *   The `Pagination` enum defines `PageOnly` and `Cursor` types, but `run_fetch` in `src/pipeline/run.rs` returns empty `FetchStats` for them. These features appear to be "planned but not implemented".
2.  **Hardcoded Constants**:
    *   `src/cmd/mod.rs` defines `CONCURRENCY = 5`, `DEFAULT_PAGE_SIZE = 50`, etc. While some are passed down, more granular control via config would be better.
    *   `PostgresWriter` has a default batch size of 5000 hardcoded in `new`.
3.  **Error Handling Granularity**:
    *   While there is a custom `ApitapError`, some error paths (like in `on_page_error`) just log the error and continue. Depending on the use case, a "fail-fast" option might be desirable.
4.  **Testing**:
    *   Integration tests exist, but unit test coverage for complex logic (like the pagination state machines) could be strengthened.

## 6. Code Walkthrough (Trace)

1.  User runs `apitap -m examples/sql -y pipelines.yaml`.
2.  `cmd::run_pipeline` starts. It discovers `examples/sql/posts.sql`.
3.  It renders `posts.sql`. `{{ sink("postgres_sink") }}` tells it to look for a target named `postgres_sink`. `{{ use_source("json_placeholder_posts") }}` links to the source.
4.  It initializes `Http` client and `PostgresWriter`.
5.  It calls `pipeline::run::run_fetch`.
6.  `run_fetch` sees `LimitOffset` pagination. It calls `fetcher.fetch_limit_offset`.
7.  `fetcher` makes HTTP requests. It gets a JSON array.
8.  It passes this data to `DataFusionPageWriter`.
9.  `DataFusionPageWriter` infers schema, registers a table in DataFusion context.
10. DataFusion executes the SQL (e.g., `SELECT ... FROM ... WHERE userId > 5`).
11. The result stream is piped to `PostgresWriter`.
12. `PostgresWriter` buffers rows into batches of 5000.
13. For each batch, it generates a `MERGE` or `INSERT` statement and executes it against the DB.
14. Logs verify success: "✅ Module Completed".

## 7. Conclusion

ApiTap is a well-structured, modern Rust ETL tool. It successfully leverages the ecosystem's best libraries (Tokio, Reqwest, DataFusion, SQLx) to solve a common problem efficiently. The main actionable items for the future are completing the missing pagination implementations and exposing more configuration options to the user.
