# Tedio

A minimal single-threaded async runtime with support for scoped, non-`'static` tasks.

> **WIP**: I originally intended to build a scope-based runtime without cross-thread scheduling, but when I completed the scope part of the code, I realized this is runtime-agnostic. So I thought I could organize it first, hoping it would be useful.

[![Crates.io](https://img.shields.io/crates/v/tedio)](https://crates.io/crates/tedio)
[![Documentation](https://docs.rs/tedio/badge.svg)](https://docs.rs/tedio)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)

## Overview

Tedio is a lightweight, self-contained async runtime designed for scenarios where you need to handle non-`'static` lifetimes in async contexts. It provides scoped execution capabilities that integrate seamlessly with the Tokio ecosystem.

## Features

- **📦 Scoped execution**: Support for non-`'static` lifetime tasks
- **🚀 Non-blocking**: Doesn't block Tokio runtime when used together

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
tedio-scope = "0.1.0"
tokio = { version = "1.0", features = ["full"] }
```

## Core Features

### 1. Borrowing with `&` (Shared References)

```rust
use tedio_scope::scope;

#[tokio::main]
async fn main() {
    let shared_data = vec![1, 2, 3, 4, 5];
    
    // Multiple tasks can borrow the same data
    let task1 = scope(async {
        let sum: i32 = shared_data.iter().sum();
        format!("Sum: {}", sum)
    });
    
    let task2 = scope(async {
        let count = shared_data.len();
        format!("Count: {}", count)
    });
    
    let (result1, result2) = tokio::join!(task1, task2);
    println!("{}", result1); // Sum: 15
    println!("{}", result2); // Count: 5
}
```

### 2. Mutating with `&mut` (Mutable References)

```rust
use tedio_scope::scope;

#[tokio::main]
async fn main() {
    let mut counter = 0;
    
    // Task can mutate the data
    let result = scope(async {
        counter += 1;
        format!("Counter: {}", counter)
    }).await;
    
    println!("{}", result); // Counter: 1
    println!("Final counter: {}", counter); // Final counter: 1
}
```

### 3. Complex Borrowing Patterns

```rust
use tedio_scope::scope;

#[tokio::main]
async fn main() {
    let mut data = vec![1, 2, 3];
    let shared_info = "processing";
    
    let result = scope(async {
        // Can borrow both & and &mut
        data.push(4);
        let sum: i32 = data.iter().sum();
        format!("{}: sum = {}", shared_info, sum)
    }).await;
    
    println!("{}", result); // processing: sum = 10
    println!("Final data: {:?}", data); // Final data: [1, 2, 3, 4]
}
```

## Usage Examples

### File I/O with Tokio

```rust
use tedio_scope::scope;
use std::io::Write;
use tempfile::NamedTempFile;

#[tokio::main]
async fn main() {
    // Create a temporary file
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(temp_file, "Hello from Tedio!").unwrap();
    let file_path = temp_file.path().to_path_buf();
    
    let result = scope(async {
        // Use tokio::fs in tedio scope
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        format!("Read: {}", content.trim())
    }).await;
    
    println!("{}", result); // Output: Read: Hello from Tedio!
}
```

## Integration with Tokio

Tedio integrates seamlessly with Tokio ecosystem. You can directly await tedio's scope tasks in Tokio runtime:

```rust
use tedio_scope::scope;

#[tokio::main]
async fn main() {
    // Direct integration - no spawn_blocking needed!
    let result = scope(async {
        // Use any tokio async primitives
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        tokio::fs::read_to_string("file.txt").await.unwrap();
        "Processing completed"
    }).await;
    
    // Other tokio tasks run concurrently
    tokio::spawn(async {
        // This runs concurrently with tedio task
    });
}
```

## API Reference

### Core Functions

#### `scope<F>(future: F) -> ScopeJoinHandle<F>`

Creates a scoped task that can borrow from its environment. The key feature is that the future can borrow both `&` and `&mut` references from the surrounding scope.

```rust
// Borrowing & (shared reference)
let data = vec![1, 2, 3];
let handle = scope(async {
    let sum: i32 = data.iter().sum(); // Borrows &data
    format!("Sum: {}", sum)
});

// Borrowing &mut (mutable reference)
let mut counter = 0;
let handle = scope(async {
    counter += 1; // Borrows &mut counter
    format!("Counter: {}", counter)
});

// Complex borrowing
let mut items = vec![1, 2];
let info = "processing";
let handle = scope(async {
    items.push(3); // Borrows &mut items
    let count = items.len(); // Borrows &items
    format!("{}: count = {}", info, count) // Borrows &info
});
```

#### `join_all<I>(futures: I) -> JoinAll<I::Item>`

Waits for all futures to complete concurrently.

```rust
let data = vec![1, 2, 3];
let futures = vec![
    scope(async { data[0] * 2 }), // Borrows &data
    scope(async { data[1] * 3 }), // Borrows &data
    scope(async { data[2] * 4 }), // Borrows &data
];
let results = join_all(futures).await;
```

## Examples

Tedio provides examples demonstrating its core borrowing capabilities:

### Basic Borrowing Example

Demonstrates fundamental borrowing patterns:

```bash
cd crates/tedio-scope
cargo run --example direct_await_in_tokio
```

This example shows:

- **Shared borrowing (`&`)**: Multiple tasks borrowing the same data
- **Mutable borrowing (`&mut`)**: Tasks modifying data in scope
- **Complex patterns**: Combining `&` and `&mut` in the same task
- **Real-world usage**: File I/O, timers, and network operations

### Non-blocking Integration Example

Demonstrates that tedio integrates seamlessly with Tokio without blocking:

```bash
cd crates/tedio-scope
cargo run --example non_blocking_tedio
```

This example verifies:

- Tedio doesn't block Tokio's runtime
- Other Tokio tasks can run concurrently
- Multiple tedio tasks can execute in parallel
- Cooperative integration with Tokio's event loop
