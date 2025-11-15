# Learning Rust Through APITap

## Welcome to Rust! 🦀

This guide will help you understand Rust by walking through the APITap codebase. We'll explain every concept, pattern, and idiom you'll encounter.

---

## Table of Contents

1. [Rust Fundamentals in This Project](#rust-fundamentals)
2. [Understanding Ownership & Borrowing](#ownership--borrowing)
3. [Async/Await & Tokio](#asyncawait--tokio)
4. [Traits: Rust's Interfaces](#traits)
5. [Error Handling](#error-handling)
6. [Pattern Matching](#pattern-matching)
7. [Type System & Generics](#type-system--generics)
8. [Memory Management](#memory-management)
9. [Common Patterns Explained](#common-patterns)
10. [Reading the Codebase](#reading-the-codebase)

---

## Rust Fundamentals

### 1. Variables and Mutability

```rust
// Immutable by default (can't change)
let x = 5;
// x = 10; // ERROR!

// Mutable (can change)
let mut y = 5;
y = 10; // OK!

// Real example from src/http/fetcher.rs:
let mut samples = Vec::new();  // Mutable vector
samples.push(item);  // Can modify because it's mut
```

**Why?** Rust defaults to immutable to prevent bugs. You explicitly choose mutability.

### 2. Ownership Rules

**Three Golden Rules:**
1. Each value has an owner
2. Only one owner at a time
3. When owner goes out of scope, value is dropped

```rust
// Example from src/http/fetcher.rs:
pub struct PaginatedFetcher {
    client: Client,        // PaginatedFetcher OWNS this Client
    base_url: String,      // OWNS this String
    concurrency: usize,    // OWNS this usize
}

// When PaginatedFetcher is dropped, ALL these are dropped too
```

### 3. Move Semantics

```rust
// Example of move:
let s1 = String::from("hello");
let s2 = s1;  // s1 MOVED to s2
// println!("{}", s1);  // ERROR! s1 no longer valid

// Real example from code:
tokio::spawn(async move {
    // "move" keyword moves ownership INTO the async block
    while let Some(item) = json_stream.next().await {
        tx.send(item).await;
    }
    // json_stream is dropped here (out of scope)
});
```

---

## Ownership & Borrowing

### References: Borrowing Without Owning

```rust
// Immutable borrow (&T)
fn print_length(s: &String) {  // Borrow, don't take ownership
    println!("Length: {}", s.len());
}  // s goes out of scope, but STRING ISN'T DROPPED

let my_string = String::from("hello");
print_length(&my_string);  // Lend it
println!("{}", my_string);  // Still valid!
```

```rust
// Mutable borrow (&mut T)
fn add_exclamation(s: &mut String) {
    s.push('!');  // Can modify
}

let mut my_string = String::from("hello");
add_exclamation(&mut my_string);  // Lend mutably
println!("{}", my_string);  // "hello!"
```

### Real Example from `src/http/fetcher.rs`

```rust
pub async fn ndjson_stream_qs(
    client: &reqwest::Client,  // ← Borrow (don't take ownership)
    url: &str,                  // ← Borrow string slice
    query: &[(String, String)], // ← Borrow slice of tuples
    data_path: Option<&str>,    // ← Option of borrowed str
    config_retry: &crate::pipeline::Retry,  // ← Borrow
) -> Result<BoxStream<'static, Result<Value>>>

// Why borrow? Function doesn't need to OWN these values!
// Caller can reuse them after function returns
```

---

## Async/Await & Tokio

### What is Async?

**Synchronous (blocking):**
```rust
fn fetch_data() -> String {
    // Wait here (blocks thread)
    std::thread::sleep(Duration::from_secs(2));
    "data".to_string()
}
```

**Asynchronous (non-blocking):**
```rust
async fn fetch_data() -> String {
    // Yields to other tasks while waiting
    tokio::time::sleep(Duration::from_secs(2)).await;
    "data".to_string()
}
```

### The `async` Keyword

```rust
// Regular function
fn do_something() -> Result<String> {
    Ok("done".to_string())
}

// Async function (returns a Future)
async fn do_something_async() -> Result<String> {
    tokio::time::sleep(Duration::from_secs(1)).await;
    Ok("done".to_string())
}

// Calling async function:
// Wrong:
// let result = do_something_async();  // Just gets Future, doesn't run!

// Correct:
let result = do_something_async().await;  // Actually runs it
```

### Real Example from `src/http/fetcher.rs`

```rust
pub async fn fetch_limit_offset(
    //   ^^^^^ This function is async
    &self,
    limit: u64,
    // ... parameters ...
) -> Result<FetchStats> {
    //   ^^^^^^ Returns a Future that resolves to Result<FetchStats>
    
    // Call another async function
    let json_stream = self
        .limit_offset_stream(limit, data_path, config_retry)
        .await;  // ← Must await to get result
    //  ^^^^^
    
    // More async operations
    self.write_streamed_page(...)
        .await?;  // ← await and ? at the same time!
    //      ^    (await, then unwrap Result)
    
    Ok(stats)
}
```

### Tokio Runtime

```rust
// Tokio provides the async runtime
#[tokio::main]  // ← Macro that sets up async runtime
async fn main() {
    // Can now use .await inside main
    let result = fetch_data().await;
}

// Or manually:
fn main() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let result = fetch_data().await;
    });
}
```

### Spawning Tasks (Async "threads")

```rust
// From src/http/fetcher.rs:
let _stream_task = tokio::spawn(async move {
    //                ^^^^^^^^^^^^ Spawn new async task
    //                             (like spawning a thread)
    let mut pinned = json_stream;
    while let Some(item) = pinned.next().await {
        if tx.send(item).await.is_err() {
            break;
        }
    }
});
// Task runs concurrently with rest of code
```

---

## Traits

### What are Traits?

**Traits = Interfaces in other languages**

```rust
// Define a trait
trait Describable {
    fn describe(&self) -> String;
}

// Implement it for a type
struct Person {
    name: String,
}

impl Describable for Person {
    fn describe(&self) -> String {
        format!("Person named {}", self.name)
    }
}
```

### Real Example: `PageWriter` Trait

```rust
// From src/http/fetcher.rs:
#[async_trait]  // ← Macro for async trait methods
pub trait PageWriter: Send + Sync {
    //                 ^^^^^^^^^^^
    //                 Trait bounds: must be Send and Sync
    //                 (safe to send between threads)
    
    // Method signature (no implementation here)
    async fn write_page(
        &self,
        page_number: u64,
        data: Vec<Value>,
        write_mode: WriteMode,
    ) -> Result<()>;
    
    // Default implementation (can be overridden)
    async fn on_page_error(&self, page_number:u64, error: String) -> Result<()> {
        error!(page = page_number, %error, "error fetching page");
        Ok(())
    }
}
```

### Implementing the Trait

```rust
// From src/http/fetcher.rs:
pub struct DataFusionPageWriter {
    table_name: String,
    sql: String,
    final_writer: Arc<dyn DataWriter>,  // ← dyn = trait object
}

#[async_trait]
impl PageWriter for DataFusionPageWriter {
    //   ^^^^^^^^^^     ^^^^^^^^^^^^^^^^^^^^^
    //   Trait name     Type implementing trait
    
    async fn write_page(
        &self,
        page_number: u64,
        data: Vec<Value>,
        write_mode: WriteMode,
    ) -> Result<()> {
        // Implementation here
        let items = data.len();
        info!(table = %self.table_name, page = page_number, items = items);
        // Do work...
        Ok(())
    }
}
```

### Trait Objects (`dyn Trait`)

```rust
// From src/http/fetcher.rs:
async fn fetch_limit_offset(
    &self,
    // ...
    writer: Arc<dyn PageWriter>,  // ← Can be ANY type that implements PageWriter
    //          ^^^ dynamic dispatch
) -> Result<FetchStats> {
    // Call trait methods
    writer.begin().await?;
    writer.write_page(1, data, write_mode).await?;
    writer.commit().await?;
}

// Can pass different implementations:
let writer1: Arc<dyn PageWriter> = Arc::new(DataFusionPageWriter::new(...));
let writer2: Arc<dyn PageWriter> = Arc::new(CustomWriter::new(...));
// Both work!
```

---

## Error Handling

### The `Result` Type

```rust
// Result is an enum with two variants:
enum Result<T, E> {
    Ok(T),   // Success with value
    Err(E),  // Error with error value
}

// Usage:
fn divide(a: i32, b: i32) -> Result<i32, String> {
    if b == 0 {
        Err("Division by zero".to_string())
    } else {
        Ok(a / b)
    }
}
```

### The `?` Operator (Error Propagation)

```rust
// Without ?:
fn do_work() -> Result<String, Error> {
    let file = match File::open("data.txt") {
        Ok(f) => f,
        Err(e) => return Err(e),  // Propagate error
    };
    
    let content = match read_file(file) {
        Ok(c) => c,
        Err(e) => return Err(e),  // Propagate error
    };
    
    Ok(content)
}

// With ?:
fn do_work() -> Result<String, Error> {
    let file = File::open("data.txt")?;  // If Err, return early
    let content = read_file(file)?;      // If Err, return early
    Ok(content)
}
```

### Real Example from Code

```rust
// From src/http/fetcher.rs:
pub async fn fetch_page_number(...) -> Result<FetchStats> {
    //                                     ^^^^^^^^^^^^^^^^
    //                                     Can return error
    
    writer.begin().await?;  // If error, return immediately
    //                   ^
    
    let first_json: Value = self
        .client
        .get(&self.base_url)
        .query(&[(page_param.as_str(), "1".to_string())])
        .send()
        .await?  // Network error? Return it
        .error_for_status()?  // HTTP error? Return it
        .json()
        .await?;  // Parse error? Return it
    
    Ok(stats)  // Success!
}
```

### Custom Error Types

```rust
// From src/errors/mod.rs:
#[derive(Debug, thiserror::Error)]  // ← Derive Error trait
pub enum ApitapError {
    #[error("Pipeline error: {0}")]  // ← Error message
    PipelineError(String),
    
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),  // ← Automatic conversion
    //         ^^^^^
    
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
}

// Usage:
fn do_something() -> Result<()> {
    let response = reqwest::get("https://...").await?;
    // If reqwest::Error occurs, automatically converted to ApitapError
    Ok(())
}
```

---

## Pattern Matching

### The `match` Expression

```rust
// Basic match
let number = 5;
match number {
    1 => println!("One"),
    2 => println!("Two"),
    3..=5 => println!("Three to five"),  // Range
    _ => println!("Something else"),     // Catch-all
}
```

### Matching on Result

```rust
// From codebase pattern:
match result {
    Ok(value) => {
        // Do something with value
        println!("Success: {}", value);
    },
    Err(error) => {
        // Handle error
        eprintln!("Error: {}", error);
    },
}
```

### `if let` - Match One Pattern

```rust
// Instead of:
match some_option {
    Some(value) => println!("Got: {}", value),
    None => {},  // Don't care about None
}

// Use if let:
if let Some(value) = some_option {
    println!("Got: {}", value);
}

// Real example from src/http/fetcher.rs:
if let Some(p) = data_path {
    if let Some(arr) = first_json.pointer(p).and_then(|v| v.as_array()).cloned() {
        // Use arr
    }
}
```

### `while let` - Loop While Pattern Matches

```rust
// From src/http/fetcher.rs:
while let Some(item) = stream.next().await {
    //    ^^^^^^^^^^^^^^^ Pattern
    //                     ^^^^^^^^^^^^^ Expression
    // Continues while next() returns Some(item)
    // Stops when it returns None
    match item {
        Ok(v) => yield Ok(v),
        Err(e) => return Err(e),
    }
}
```

---

## Type System & Generics

### Generic Functions

```rust
// Generic function (works with any type T)
fn print_anything<T: std::fmt::Display>(item: T) {
    //            ^                       ^^^^^
    //            Type parameter          Uses T
    //              with trait bound
    println!("{}", item);
}

// Can call with different types:
print_anything(5);           // T = i32
print_anything("hello");     // T = &str
print_anything(3.14);        // T = f64
```

### Real Example from Code

```rust
// From src/utils/execution.rs:
impl Exec {
    pub fn new<F>(  // ← Generic over F
        schema: SchemaRef,
        projections: Option<&Vec<usize>>,
        stream_factory: F  // ← F is generic
    ) -> datafusion::error::Result<Self>
    where
        F: Fn() -> Pin<Box<dyn Stream<Item = Result<Value>> + Send>>
            + Send + Sync + 'static,
        // ↑ F must satisfy these trait bounds
    {
        Ok(Self {
            stream_factory: Arc::new(stream_factory),
            // Convert F to concrete type
            projected_schema,
            cache,
        })
    }
}
```

### Option<T> - Maybe Has a Value

```rust
// Option is an enum:
enum Option<T> {
    Some(T),  // Has a value
    None,     // No value
}

// Usage:
let maybe_number: Option<i32> = Some(5);
let no_number: Option<i32> = None;

// Real example:
pub async fn fetch_limit_offset(
    &self,
    limit: u64,
    data_path: Option<&str>,  // ← May or may not have a path
    // ...
) {
    // Check if has value
    if let Some(path) = data_path {
        // Use path
    } else {
        // No path provided
    }
}
```

### Arc<T> - Shared Ownership

```rust
// Arc = Atomic Reference Counted
// Multiple owners can share one value

use std::sync::Arc;

let data = Arc::new(vec![1, 2, 3]);
let data1 = Arc::clone(&data);  // Cheap clone (just increments counter)
let data2 = Arc::clone(&data);

// All three point to SAME data
// Data dropped when ALL Arcs dropped
```

```rust
// Real example from src/http/fetcher.rs:
pub type JsonStreamFactory = Arc<
    dyn Fn() -> Pin<Box<dyn Stream<Item = Result<Value>> + Send>> 
    + Send + Sync
>;
// Arc allows multiple owners of the factory
// Cheap to clone and pass around
```

---

## Memory Management

### Stack vs Heap

```rust
// Stack allocated (fixed size, fast)
let x = 5;                    // i32 on stack
let array = [1, 2, 3, 4, 5];  // Fixed-size array on stack

// Heap allocated (dynamic size, slower)
let vec = Vec::new();    // Allocates on heap, can grow
let string = String::from("hello");  // Heap-allocated string
let boxed = Box::new(10);  // Explicitly box (heap allocate)
```

### Smart Pointers

```rust
// Box<T> - Owned pointer to heap data
let boxed_value = Box::new(5);

// Arc<T> - Shared ownership (thread-safe)
let shared = Arc::new(vec![1, 2, 3]);

// Rc<T> - Shared ownership (NOT thread-safe)
let shared_local = Rc::new(vec![1, 2, 3]);
```

### Pin<T> - Prevent Moving

```rust
// From src/http/fetcher.rs:
pub type BoxStreamCustom<T> = Pin<Box<dyn Stream<Item = T> + Send + 'static>>;
//                            ^^^ Pin prevents moving in memory
//                                (required for self-referential types like async streams)

// Usage:
let stream: Pin<Box<dyn Stream<...>>> = Box::pin(async_stream::stream! {
    // Stream implementation
});
```

---

## Common Patterns

### Pattern 1: Builder Pattern

```rust
// From src/http/fetcher.rs:
impl PaginatedFetcher {
    pub fn new(...) -> Self {
        Self { /* fields */ }
    }
    
    // Builder methods (chainable)
    pub fn with_limit_offset(mut self, ...) -> Self {
        self.pagination_config = Pagination::LimitOffset { /* */ };
        self  // Return self for chaining
    }
    
    pub fn with_batch_size(mut self, n: usize) -> Self {
        self.batch_size = n.max(1);
        self
    }
}

// Usage:
let fetcher = PaginatedFetcher::new(client, url, 5)
    .with_limit_offset("limit", "offset")
    .with_batch_size(256);
//  ^^^^^^^^^^^^^^^^^ Chained!
```

### Pattern 2: Type State Pattern

```rust
// Encode state in types
struct Uninitialized;
struct Ready;

struct Connection<State> {
    // ...
    _state: PhantomData<State>,
}

impl Connection<Uninitialized> {
    fn connect(self) -> Connection<Ready> {
        // Can only call connect on Uninitialized
        // Returns Ready state
    }
}

impl Connection<Ready> {
    fn send(&self, data: &[u8]) {
        // Can only send when Ready
    }
}
```

### Pattern 3: Newtype Pattern

```rust
// Wrap existing type for type safety
struct UserId(u64);  // NOT just a u64!
struct PostId(u64);  // Different from UserId

fn get_user(id: UserId) { /* */ }

let user_id = UserId(123);
let post_id = PostId(456);

get_user(user_id);  // OK
// get_user(post_id);  // ERROR! Different types
```

### Pattern 4: Visitor Pattern (with Traits)

```rust
trait Visitor {
    fn visit_number(&mut self, n: i32);
    fn visit_string(&mut self, s: &str);
}

enum Value {
    Number(i32),
    Text(String),
}

impl Value {
    fn accept<V: Visitor>(&self, visitor: &mut V) {
        match self {
            Value::Number(n) => visitor.visit_number(*n),
            Value::Text(s) => visitor.visit_string(s),
        }
    }
}
```

---

## Reading the Codebase

### Start Here: `src/main.rs`

```rust
#[tokio::main]  // ← Entry point with async runtime
async fn main() -> anyhow::Result<()> {
    // 1. Initialize logging
    crate::log::init();
    
    // 2. Parse CLI arguments
    let args = Args::parse();
    
    // 3. Run the pipeline
    crate::cmd::run_pipeline(args).await?;
    //         ^^^^^^^^^^^^^ Main logic
    
    Ok(())
}
```

### Follow the Flow: `src/cmd/mod.rs`

```rust
pub async fn run_pipeline(args: Args) -> Result<()> {
    // 1. Load configuration
    let config = load_config(&args)?;
    
    // 2. For each SQL module
    for module in sql_modules {
        // 3. Create HTTP client
        let client = build_http_client(&config)?;
        
        // 4. Create fetcher
        let fetcher = PaginatedFetcher::new(...);
        
        // 5. Fetch data
        fetcher.fetch_limit_offset(...).await?;
    }
    
    Ok(())
}
```

### Understanding Module Structure

```
src/
├── main.rs              // Entry point
├── lib.rs               // Library root
├── cmd/                 // Command execution
│   └── mod.rs           // run_pipeline()
├── config/              // Configuration loading
│   ├── mod.rs
│   └── templating.rs    // ${VAR} substitution
├── errors/              // Error types
│   └── mod.rs
├── http/                // HTTP fetching
│   ├── mod.rs           // HTTP client setup
│   └── fetcher.rs       // PaginatedFetcher
├── pipeline/            // Pipeline orchestration
│   ├── mod.rs
│   ├── run.rs
│   └── sink.rs
├── utils/               // Utilities
│   ├── datafusion_ext.rs   // DataFusion extensions
│   ├── execution.rs        // ExecutionPlan
│   ├── table_provider.rs   // TableProvider
│   ├── schema.rs          // Schema inference
│   └── streaming.rs        // Stream utilities
└── writer/              // Data writers
    ├── mod.rs
    └── postgres.rs      // PostgreSQL writer
```

### Reading a Function

**Step-by-Step Example:**

```rust
// 1. Function signature
pub async fn ndjson_stream_qs(
//  ^^^^^                       async function
    client: &reqwest::Client,  // borrows client
    url: &str,                 // borrows string slice
    query: &[(String, String)],// borrows slice
    data_path: Option<&str>,   // optional path
    config_retry: &crate::pipeline::Retry,
) -> Result<BoxStream<'static, Result<Value>>> {
//   ^^^^^^                                      Returns Result
//          ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^      Success type (Stream)
    
    // 2. Function body
    let resp = client.get(url)  // Builder pattern
        .query(query)           // Method chaining
        .send()                 // Returns Future
        .await?;                // Await future, propagate error
    //      ^
    
    // 3. Pattern matching
    let is_ndjson = resp
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())  // Option chaining
        .map(|ct| ct.contains("ndjson"))  // Transform
        .unwrap_or(false);  // Default value
    
    // 4. Conditional logic
    if !is_ndjson {
        // Handle JSON
    } else {
        // Handle NDJSON
    }
    
    // 5. Return
    Ok(stream)
}
```

---

## Learning Resources

### Official Rust Resources

1. **The Rust Book** - https://doc.rust-lang.org/book/
   - Start here! Comprehensive introduction

2. **Rust by Example** - https://doc.rust-lang.org/rust-by-example/
   - Learn by seeing examples

3. **Rustlings** - https://github.com/rust-lang/rustlings
   - Interactive exercises

### Async Rust

1. **Async Book** - https://rust-lang.github.io/async-book/
2. **Tokio Tutorial** - https://tokio.rs/tokio/tutorial

### Practice Projects

1. Start with: Command-line tools
2. Move to: Web servers (using actix-web or axum)
3. Advanced: Async data processing (like APITap!)

---

## Exercises

### Exercise 1: Add a New Pagination Strategy

Try adding a "cursor" pagination strategy:

```rust
// In src/http/fetcher.rs

// 1. Add to Pagination enum
pub enum Pagination {
    // ... existing variants
    Cursor {
        cursor_param: String,
        next_cursor_path: String,  // JSON path to next cursor
    },
}

// 2. Implement fetch method
impl PaginatedFetcher {
    pub async fn fetch_cursor(
        &self,
        // ... parameters
    ) -> Result<FetchStats> {
        // Your implementation
    }
}
```

### Exercise 2: Create a Custom Writer

Build a writer that writes to files instead of database:

```rust
// Create new file: src/writer/file.rs

pub struct FileWriter {
    path: PathBuf,
}

#[async_trait]
impl PageWriter for FileWriter {
    async fn write_page(
        &self,
        page_number: u64,
        data: Vec<Value>,
        _write_mode: WriteMode,
    ) -> Result<()> {
        // Convert to JSON and write to file
        todo!()
    }
}
```

### Exercise 3: Add Metrics

Add a metrics counter:

```rust
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Metrics {
    requests_total: AtomicUsize,
    errors_total: AtomicUsize,
}

impl Metrics {
    pub fn increment_requests(&self) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn get_requests(&self) -> usize {
        self.requests_total.load(Ordering::Relaxed)
    }
}
```

---

## Common Mistakes & Solutions

### Mistake 1: Forgetting `.await`

```rust
// Wrong:
let result = async_function();  // Just gets Future!

// Correct:
let result = async_function().await;
```

### Mistake 2: Moving After Borrow

```rust
// Wrong:
let s = String::from("hello");
let r = &s;
let s2 = s;  // ERROR! Can't move while borrowed
println!("{}", r);

// Correct:
let s = String::from("hello");
let r = &s;
println!("{}", r);  // Use borrow first
let s2 = s;  // Now can move
```

### Mistake 3: Mutable and Immutable Borrows

```rust
// Wrong:
let mut v = vec![1, 2, 3];
let r1 = &v;
let r2 = &mut v;  // ERROR! Can't have mut ref while immut ref exists

// Correct:
let mut v = vec![1, 2, 3];
let r1 = &v;
println!("{:?}", r1);  // Use immutable ref
// r1 out of scope
let r2 = &mut v;  // Now OK
```

---

## Next Steps

1. **Read** [STREAMING_ARCHITECTURE.md](STREAMING_ARCHITECTURE.md) - Understand the big picture
2. **Study** [FETCHER_GUIDE.md](FETCHER_GUIDE.md) - Deep dive into HTTP fetching
3. **Explore** `src/http/fetcher.rs` - Read the actual code
4. **Modify** - Try making small changes
5. **Test** - Run `cargo test`
6. **Build** - Run `cargo build`

---

## Getting Help

- **Rust Discord** - https://discord.gg/rust-lang
- **Rust Users Forum** - https://users.rust-lang.org/
- **Stack Overflow** - Tag: [rust]
- **This Project** - Open an issue!

---

**Remember:** Everyone starts somewhere. Rust has a steep learning curve, but it's worth it! 🦀

**Last Updated:** November 15, 2025  
**For:** Rust Beginners Learning Through APITap
