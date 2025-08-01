//! Example: Core borrowing capabilities
//!
//! This example demonstrates the fundamental borrowing capabilities of tedio-scope:
//! - Shared borrowing with & (multiple tasks borrowing same data)
//! - Mutable borrowing with &mut (tasks modifying data)
//! - Complex borrowing patterns (combining & and &mut)

use std::io::Write;
use tedio_scope::scope;
use tempfile::NamedTempFile;

#[tokio::main]
async fn main() {
    println!("Starting Tedio + Tokio integration example...");

    // Example 1: Shared borrowing with &
    let shared_data = vec![1, 2, 3, 4, 5];

    // Multiple tasks can borrow the same data
    let task1 = scope(async {
        let sum: i32 = shared_data.iter().sum(); // Borrows &shared_data
        format!("Sum: {}", sum)
    });

    let task2 = scope(async {
        let count = shared_data.len(); // Borrows &shared_data
        format!("Count: {}", count)
    });

    let (result1, result2) = tokio::join!(task1, task2);
    println!("Result 1: {}", result1);
    println!("Result 2: {}", result2);

    // Example 2: Mutable borrowing with &mut
    let mut counter = 0;
    let result = scope(async {
        counter += 1; // Borrows &mut counter
        format!("Counter: {}", counter)
    })
    .await;

    println!("Result 2: {}", result);
    println!("Final counter: {}", counter);

    // Example 3: Complex borrowing patterns
    let mut data = vec![1, 2, 3];
    let info = "processing";
    let result = scope(async {
        data.push(4); // Borrows &mut data
        let sum: i32 = data.iter().sum(); // Borrows &data
        format!("{}: sum = {}", info, sum) // Borrows &info
    })
    .await;

    println!("Result 3: {}", result);
    println!("Final data: {:?}", data);

    // Example 4: File I/O with Tokio
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(temp_file, "Hello from Tedio!").unwrap();
    let file_path = temp_file.path().to_path_buf();

    let result = scope(async {
        // Use Tokio's async file operations
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        format!("Read file content: {}", content.trim())
    })
    .await;

    println!("Result 4: {}", result);

    // Example 5: Timer operations
    let result = scope(async {
        // Use Tokio's async timer
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        "Timer completed"
    })
    .await;

    println!("Result 5: {}", result);

    // Example 6: Network operations
    let result = scope(async {
        // Use Tokio's async networking
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Connect in another task
        let connect_task =
            tokio::spawn(async move { tokio::net::TcpStream::connect(addr).await.unwrap() });

        // Accept connection
        let (_, _) = listener.accept().await.unwrap();
        let _stream = connect_task.await.unwrap();

        "Network test completed"
    })
    .await;

    println!("Result 6: {}", result);

    // Example 7: Complex scoped tasks with local borrowing
    let local_string = "local string".to_string();
    let result = scope(async {
        // Can borrow local variables in async context
        let mut results = Vec::new();

        // Process multiple async tasks in parallel
        let task1 = async { format!("Task 1: {}", &local_string) };
        let task2 = async {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            format!("Task 2: {}", &local_string)
        };
        let task3 = async { format!("Task 3: {}", &local_string) };

        results.push(task1.await);
        results.push(task2.await);
        results.push(task3.await);

        results.join(" | ")
    })
    .await;

    println!("Result 7: {}", result);

    println!("Example completed successfully!");
}
