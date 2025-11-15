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

**WHAT are the Fundamentals?**
These are the building blocks of Rust programming - variables, ownership, and how data moves around in your program. Understanding these fundamentals is crucial because they affect EVERYTHING you write in Rust.

**WHY are these important?**
Unlike other languages where you can ignore memory management, Rust makes you think about it from day one. But this isn't busywork - it prevents an entire class of bugs (memory leaks, use-after-free, data races) at compile time!

### 1. Variables and Mutability

**WHAT is mutability?**
Mutability is whether a variable's value can be changed after it's created. By default in Rust, all variables are **immutable** (can't change).

**WHY immutable by default?**
1. **Safety**: Prevents accidental changes to data
2. **Concurrency**: Immutable data is automatically thread-safe
3. **Reasoning**: Easier to understand code (value can't change unexpectedly)
4. **Optimization**: Compiler can optimize better

**WHEN to use `mut`?**
Only when you NEED to change a value:
- Accumulating results in a loop
- Updating counters
- Modifying collections
- Building up data structures

**HOW does it work?**

```rust
// Immutable by default (can't change)
let x = 5;
// x = 10; // ❌ ERROR! Can't assign twice to immutable variable
//            Compile error: cannot assign twice to immutable variable

// Mutable (can change)
let mut y = 5;  // "mut" keyword makes it mutable
y = 10; // ✅ OK! Can change because it's mut
y = 15; // ✅ Can change again

// Real example from src/http/fetcher.rs:
let mut samples = Vec::new();  // Mutable vector
samples.push(item);  // Can add items because it's mut
samples.push(item2); // Can keep adding

// Real-world analogy:
// Immutable = Written in pen (can read, can't change)
// Mutable = Written in pencil (can read AND erase/change)
```

**Best Practice:**
```rust
// ✅ Start immutable, add mut only if needed
let count = 0;
// ... Later you realize you need to change it...
// Change to: let mut count = 0;

// ❌ Don't make everything mutable "just in case"
let mut x = 5;  // Unnecessary if never changed
```

### 2. Ownership Rules

**WHAT is ownership?**
Ownership is Rust's system for managing memory. Every value has ONE owner, and when the owner goes away, the value is automatically cleaned up.

**The Three Golden Rules:**
1. Each value has an owner
2. Only one owner at a time  
3. When owner goes out of scope, value is dropped

**WHY three rules?**
These rules prevent:
- ❌ Memory leaks (forgot to free memory)
- ❌ Double free (freeing same memory twice)
- ❌ Use after free (using memory after it's freed)
- ❌ Data races (two threads modifying same data)

**WHEN does ownership matter?**
- Passing data to functions
- Returning data from functions
- Storing data in structs
- Working with threads

**HOW does it work?**

```rust
// Example from src/http/fetcher.rs:
pub struct PaginatedFetcher {
    client: Client,        // PaginatedFetcher OWNS this Client
    base_url: String,      // OWNS this String
    concurrency: usize,    // OWNS this usize
}

// When PaginatedFetcher is dropped, ALL these are dropped too
// Automatically! No manual cleanup needed.

// Detailed example:
{
    let s = String::from("hello");  // s OWNS the String
    // s is valid here
    // Can use s
} // ← s goes out of scope here
  // String's memory is automatically freed
  // No need for: free(s) or delete s or s = null

// Real-world analogy:
// Ownership is like owning a car:
// - You own it (you're the owner)
// - Only ONE person owns it at a time (can't have two owners)
// - When you sell it, it's not yours anymore (transfer ownership)
// - When you junk it, it's gone (drop/cleanup)
```

### 3. Move Semantics

**WHAT is a move?**
A move transfers ownership from one variable to another. The original variable becomes invalid - you can no longer use it.

**WHY moves instead of copies?**
- **Performance**: Moving is free (just change ownership, don't copy data)
- **Safety**: Can't accidentally have two owners
- **Clear intent**: Explicit about data flow

**WHEN does a move happen?**
- Assigning one variable to another (for non-Copy types)
- Passing to a function (unless it takes a reference)
- Returning from a function
- Using the `move` keyword with closures

**HOW does it work?**

```rust
// Example of move:
let s1 = String::from("hello");  // s1 owns the String
let s2 = s1;  // ← MOVE! Ownership transfers to s2
              // s1 is now INVALID
// println!("{}", s1);  // ❌ ERROR! s1 no longer valid
//                       // Compile error: value borrowed after move
println!("{}", s2);  // ✅ OK! s2 owns it now

// WHY can't use s1?
// - String is heap-allocated
// - Moving is cheap (just pointer copy)
// - If both s1 and s2 were valid, we'd free the STRING TWICE!
// - Rust prevents this at compile time

// Real example from code:
tokio::spawn(async move {
    //               ^^^^^ move keyword: move ownership into closure
    // json_stream is MOVED into this async block
    while let Some(item) = json_stream.next().await {
        tx.send(item).await;
    }
    // json_stream is dropped here (out of scope)
});
// ❌ Can't use json_stream here anymore - it MOVED into closure

// Real-world analogy:
// Move is like giving away your backpack:
// - You give it to your friend (move ownership)
// - You can't reach into it anymore (it's not yours!)
// - Your friend now has it (new owner)
// - Only ONE person has the backpack at a time
```

**Copy vs Move:**
```rust
// Simple types are COPIED (not moved)
let x = 5;       // i32 implements Copy
let y = x;       // COPIED! x is still valid
println!("{} {}", x, y);  // ✅ Both work!

// Complex types are MOVED (not copied)
let s1 = String::from("hello");
let s2 = s1;     // MOVED! s1 is now invalid
// println!("{}", s1);  // ❌ Error
println!("{}", s2);     // ✅ OK
```

---

## Ownership & Borrowing

**WHAT is Ownership?**
Ownership is Rust's most unique feature. It's a set of rules that govern how Rust manages memory. Instead of garbage collection (like Java/Python) or manual memory management (like C/C++), Rust uses ownership to track which part of code is responsible for cleaning up data.

**WHY does Rust use ownership?**
1. **Memory Safety**: Prevents use-after-free bugs, double-free bugs, and memory leaks
2. **No Garbage Collector**: Programs are faster because there's no GC pause
3. **Thread Safety**: Makes it impossible to have data races at compile time
4. **Zero Cost**: These guarantees have no runtime overhead!

**WHEN is ownership checked?**
At compile time! The Rust compiler checks ownership rules before your program even runs. If there's a problem, your code won't compile.

**The Three Rules of Ownership:**
1. **Each value has an owner** - Every piece of data has exactly one variable that owns it
2. **Only one owner at a time** - You can't have two variables owning the same data
3. **When the owner goes out of scope, the value is dropped** - Memory is automatically freed

### References: Borrowing Without Owning

**WHAT is borrowing?**
Borrowing is like lending someone a book - you still own it, but they can read it. In Rust, you create a reference (`&`) to data without taking ownership.

**WHY borrow instead of own?**
- The function doesn't need to keep the data
- You want to use the data after the function call
- You want multiple parts of code to read the same data
- More efficient (no moving/copying data)

**WHEN to use borrowing?**
- When a function only needs to READ data → use `&T`
- When a function needs to MODIFY data → use `&mut T`
- When you want to pass data without giving up ownership

**WHERE do we see this in APITap?**
Almost every function parameter! We pass references to avoid unnecessary copying.

**HOW does it work?**

```rust
// Immutable borrow (&T) - Read-only access
fn print_length(s: &String) {  
    //              ^ Borrow, don't take ownership
    println!("Length: {}", s.len());
    // Can READ s, but CANNOT modify it
}  // s goes out of scope, but the actual String ISN'T DROPPED
   // because we never owned it!

let my_string = String::from("hello");
print_length(&my_string);  
//           ^ The & symbol means "borrow this"
//             We're LENDING my_string to the function

println!("{}", my_string);  
// Still valid! We still own my_string because we only borrowed it

// Real-world analogy:
// You lend your textbook to a classmate to read
// They can read it but can't write in it
// When they're done, you still have your book
```

```rust
// Mutable borrow (&mut T) - Can modify the data
fn add_exclamation(s: &mut String) {
    //                 ^^^^ Mutable borrow
    s.push('!');  // Can modify because we have &mut
}

let mut my_string = String::from("hello");
//  ^^^ Must be mutable to lend it mutably

add_exclamation(&mut my_string);  
//              ^^^^ Borrow as mutable

println!("{}", my_string);  // "hello!"
// Data was modified, but we still own it

// Real-world analogy:
// You lend your notebook to a classmate
// They can write in it and make changes
// When they return it, you still own it (but it's been modified)
```

**Common Borrowing Rules:**
```rust
// ✅ Can have MANY immutable borrows at once
let s = String::from("hello");
let r1 = &s;  // OK
let r2 = &s;  // OK - multiple readers
let r3 = &s;  // OK - as many as you want
println!("{} {} {}", r1, r2, r3);

// ✅ Can have ONE mutable borrow
let mut s = String::from("hello");
let r1 = &mut s;  // OK - one writer
r1.push('!');

// ❌ CANNOT have mutable and immutable borrows at same time
let mut s = String::from("hello");
let r1 = &s;      // Immutable borrow
let r2 = &mut s;  // ERROR! Can't borrow as mutable while r1 exists
println!("{}", r1);

// WHY these rules?
// - Multiple readers: Safe! They just read
// - One writer: Safe! No one else interfering
// - Reader + writer: UNSAFE! Writer could change data while reader uses it
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

**WHAT is `async`?**
The `async` keyword transforms a regular function into an asynchronous function. Instead of executing immediately and blocking the thread, an async function returns a `Future` - a promise that the work will be done eventually.

**WHY use `async`?**
Without async, your program would freeze waiting for slow operations (like network requests or file I/O). With async, while one task is waiting, other tasks can make progress. This is crucial for APITap because we're fetching data from multiple URLs simultaneously.

**WHEN to use `async`?**
Use `async` whenever your function needs to:
- Wait for network responses (HTTP requests)
- Read/write files
- Wait for database queries
- Perform any I/O operation
- Call other async functions

**WHERE do we use it in APITap?**
Almost everywhere! The entire data pipeline is async:
- HTTP fetching (`fetch_limit_offset`)
- Database writes (`write_stream`)
- Stream processing (`write_page_stream`)

**HOW does it work?**

```rust
// Regular function - BLOCKS the thread
fn do_something() -> Result<String> {
    // This waits and BLOCKS everything else
    std::thread::sleep(Duration::from_secs(2));
    Ok("done".to_string())
}
// Problem: While sleeping, NOTHING else can run!

// Async function - YIELDS control
async fn do_something_async() -> Result<String> {
    // This yields control while waiting
    tokio::time::sleep(Duration::from_secs(2)).await;
    //                                          ^^^^^ 
    //                                          Gives control back to runtime
    Ok("done".to_string())
}
// Benefit: While this sleeps, OTHER tasks can run!

// Calling async function:
// ❌ WRONG:
let result = do_something_async();  
// This just creates a Future - DOESN'T run the code!
// Like having a recipe but not cooking

// ✅ CORRECT:
let result = do_something_async().await;  
// This actually executes the function
// Like following the recipe and cooking the food

// WHY the difference?
// Async functions are LAZY - they do NOTHING until .await
// This allows the runtime to control WHEN and WHERE they execute
```

**Real-World Analogy:**
- **Synchronous (blocking)**: You order food at a restaurant and stand at the counter waiting. You can't do anything else until your food arrives.
- **Asynchronous (non-blocking)**: You order food, get a buzzer, and sit down. While your food cooks, you can read, talk, or check your phone. When it's ready, the buzzer alerts you.

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

**WHAT are Traits?**
Traits are Rust's way of defining shared behavior. Think of them as contracts that types can implement. They're similar to interfaces in Java/C# or protocols in Swift. A trait defines method signatures, and any type can implement that trait by providing the actual code for those methods.

**WHY do we use Traits?**
1. **Polymorphism**: Write code that works with many different types
2. **Code Reuse**: Share behavior across different types
3. **Abstraction**: Define what something CAN DO, not what it IS
4. **Trait Bounds**: Constrain generic types to have certain capabilities
5. **Zero Cost**: No runtime overhead compared to direct method calls

**WHEN to use Traits?**
- When multiple types need to share the same behavior
- When you want to write generic code that works with many types
- When you need to define capabilities (e.g., "can be printed", "can be compared")
- When you want to abstract over implementations

**WHERE do we use them in APITap?**
- `PageWriter` trait - Different ways to write data (Database, File, etc.)
- `DataWriter` trait - Abstract over different output destinations
- Standard traits like `Send`, `Sync` - Thread safety guarantees
- `Clone`, `Debug` - Standard Rust functionality

**HOW do Traits work?**

```rust
// 1. DEFINE a trait - What can this thing do?
trait Describable {
    // Method signature (no implementation)
    fn describe(&self) -> String;
    
    // Can have default implementations
    fn shout(&self) -> String {
        self.describe().to_uppercase()
    }
}

// 2. IMPLEMENT the trait for a type
struct Person {
    name: String,
}

impl Describable for Person {
    // Provide actual implementation
    fn describe(&self) -> String {
        format!("Person named {}", self.name)
    }
    // shout() is inherited from trait's default implementation
}

// 3. USE the trait
fn print_description<T: Describable>(item: T) {
    //                   ^^^^^^^^^^^^ Trait bound
    //                   T must implement Describable
    println!("{}", item.describe());
}

let person = Person { name: "Alice".to_string() };
print_description(person);  // Works!

// Real-world analogy:
// Trait is like a job description
// - Job: "Can Drive" (trait)
// - Requirements: has drive() method
// - People who can drive: Car driver, Truck driver, Bus driver
// - All implement the same "Can Drive" capability differently
```

**Trait Types:**
```rust
// 1. Marker Traits (no methods, just properties)
trait Printable {}  // Just marks a type as printable

// 2. Method Traits (define behavior)
trait Writable {
    fn write(&self, data: &str);
}

// 3. Associated Type Traits
trait Container {
    type Item;  // Associated type
    fn get(&self) -> Self::Item;
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

**WHAT is Error Handling in Rust?**
Rust uses the `Result` type for error handling instead of exceptions (like Java/Python) or error codes (like C). A `Result` is an enum that explicitly represents either success (`Ok`) or failure (`Err`). This forces you to handle errors - you can't accidentally ignore them!

**WHY does Rust use `Result` instead of exceptions?**
1. **Explicit**: You can see in the function signature that it can fail
2. **Type Safety**: The compiler ensures you handle all error cases
3. **No Hidden Control Flow**: No invisible exceptions jumping up the call stack
4. **Zero Cost**: When there's no error, `Result` has no runtime overhead
5. **Composable**: Easy to chain operations with `?` operator

**WHEN to use `Result`?**
- When an operation can fail (file I/O, network requests, parsing, etc.)
- When you want callers to handle errors
- Almost always in APIs/libraries
- Instead of panicking (crashing) the program

**WHERE do we use it in APITap?**
Everywhere! Every function that can fail returns `Result`:
- HTTP requests: `fetch_limit_offset() -> Result<FetchStats>`
- Database operations: `write_stream() -> Result<()>`
- Parsing: `parse_config() -> Result<Config>`

**HOW does it work?**

### The `Result` Type

```rust
// Result is an enum with TWO variants:
enum Result<T, E> {
    Ok(T),   // Success! Contains value of type T
    Err(E),  // Failure! Contains error of type E
}

// T = Success type (what you want)
// E = Error type (what went wrong)

// Example: Division that can fail
fn divide(a: i32, b: i32) -> Result<i32, String> {
    //                       ^^^^^^^^^^^^^^^^^^^^^^
    //                       Returns Result:
    //                       - Ok(i32) on success
    //                       - Err(String) on failure
    if b == 0 {
        // Return error variant
        Err("Division by zero".to_string())
    } else {
        // Return success variant
        Ok(a / b)
    }
}

// Using it:
match divide(10, 2) {
    Ok(result) => println!("Result: {}", result),  // Prints: Result: 5
    Err(err) => println!("Error: {}", err),
}

match divide(10, 0) {
    Ok(result) => println!("Result: {}", result),
    Err(err) => println!("Error: {}", err),  // Prints: Error: Division by zero
}

// Real-world analogy:
// Result is like a package delivery
// - Ok(package): Delivery successful, here's your package
// - Err(note): Delivery failed, here's why (wrong address, recipient not home, etc.)
// You MUST check which one you got before opening the "package"
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
