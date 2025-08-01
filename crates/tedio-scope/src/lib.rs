//! # Tedio Scope
//!
//! A minimal single-threaded async runtime with support for scoped, non-`'static` tasks.
//!
//! ## Overview
//!
//! Tedio Scope is a lightweight, self-contained async runtime designed for scenarios where you need to handle non-`'static` lifetimes in async contexts. It provides scoped execution capabilities that integrate seamlessly with the Tokio ecosystem.
//!
//! ## Features
//!
//! - **📦 Scoped execution**: Support for non-`'static` lifetime tasks
//! - **🚀 Non-blocking**: Doesn't block Tokio runtime when used together
//!
//! ## Quick Start
//!
//! ```rust
//! use tedio_scope::scope;
//! use futures::executor::block_on;
//!
//! // Shared borrowing with &
//! let data = vec![1, 2, 3, 4, 5];
//! let result1 = block_on(scope(async {
//!     let sum: i32 = data.iter().sum(); // Borrows &data
//!     format!("Sum: {}", sum)
//! }));
//!
//! // Mutable borrowing with &mut
//! let mut counter = 0;
//! let result2 = block_on(scope(async {
//!     counter += 1; // Borrows &mut counter
//!     format!("Counter: {}", counter)
//! }));
//!
//! println!("{}", result1); // Output: Sum: 15
//! println!("{}", result2); // Output: Counter: 1
//! ```

#![feature(context_ext, local_waker, ptr_as_ref_unchecked)]

pub mod scope;
pub mod waker;

// Re-export commonly used items for convenience
pub use scope::{ScopeGuard, scope};
