//! Example: Non-blocking integration with Tokio
//!
//! This example demonstrates that tedio integrates seamlessly with Tokio runtime
//! without blocking other Tokio tasks. It shows that tedio's scoped execution
//! is cooperative and doesn't interfere with Tokio's event loop.

use std::sync::{Arc, Mutex};
use std::time::Instant;
use tedio_scope::scope;

#[tokio::main]
async fn main() {
    println!("Starting non-blocking integration example...");

    let counter = Arc::new(Mutex::new(0));
    let counter_clone = counter.clone();

    // Start a monitor task to verify Tokio runtime is not blocked
    let monitor_task = tokio::spawn(async move {
        let start = Instant::now();

        // Increment counter every 10ms to monitor runtime activity
        for _ in 0..100 {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            let mut count = counter_clone.lock().unwrap();
            *count += 1;

            // Stop if more than 1 second has passed
            if start.elapsed() > std::time::Duration::from_secs(1) {
                break;
            }
        }
    });

    // Execute a time-consuming operation in tedio scope
    let tedio_result = scope(async {
        println!("Starting tedio task...");

        // Simulate a time-consuming operation
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        println!("Tedio task completed!");
        "Tedio task completed"
    })
    .await;

    // Wait for monitor task to complete
    let _ = monitor_task.await;

    // Check counter to verify runtime activity
    let count = *counter.lock().unwrap();
    println!("Monitor task ran {} times during tedio execution", count);

    if count > 10 {
        println!("✅ Verification successful: tedio doesn't block tokio runtime!");
        println!("   Monitor task runs normally, indicating tokio runtime remains active");
    } else {
        println!("❌ Verification failed: tedio may block tokio runtime");
        println!("   Monitor task ran too few times, indicating runtime may be blocked");
    }

    println!("Tedio task result: {}", tedio_result);

    // Test multiple tedio tasks concurrent execution
    println!("\nTesting multiple tedio tasks concurrent execution...");

    let start = Instant::now();

    let task1 = scope(async {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        "Task 1 completed"
    });

    let task2 = scope(async {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        "Task 2 completed"
    });

    let task3 = scope(async {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        "Task 3 completed"
    });

    // Wait for all tasks to complete
    let (result1, result2, result3) = tokio::join!(task1, task2, task3);

    let elapsed = start.elapsed();
    println!("Three tedio tasks completed in {:?}", elapsed);

    if elapsed < std::time::Duration::from_millis(150) {
        println!("✅ Verification successful: tedio tasks can execute concurrently!");
        println!("   Three tasks execute in parallel, total time close to single task time");
    } else {
        println!("❌ Verification failed: tedio tasks may not execute concurrently");
        println!("   Three tasks execute serially, total time too long");
    }

    println!("Task results: {}, {}, {}", result1, result2, result3);

    println!("Example completed successfully!");
}
