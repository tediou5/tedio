//! Scoped async execution implementation.
//!
//! This module provides the core functionality for scoped task execution
//! with support for non-'static lifetimes.

use crate::waker::{ThreadNotify, waker_ref};
use pin_project::pin_project;
use std::cell::RefCell;
use std::future::Future;
use std::pin::{Pin, pin};
use std::rc::Rc;
use std::task::{Context, Poll};

// A thread-local instance of our waker-based parker.
// This is used by the `block_on` entry point to create a waker that can
// unpark the main thread.
std::thread_local! {
    static CURRENT_THREAD_NOTIFY: RefCell<Rc<ThreadNotify>> = RefCell::new(Rc::new(ThreadNotify::new()));
}

/// A handle to a scoped task.
///
/// The future is stored inside this handle, ensuring its lifetime is tied
/// to the scope where it was created.
/// If the handle is dropped, the task is cancelled.
#[pin_project]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct ScopeJoinHandle<F: Future> {
    task: Pin<Box<F>>,
    finished: Option<F::Output>,
}

impl<F: Future> ScopeJoinHandle<F> {
    /// Creates a new `ScopeJoinHandle` from a future.
    pub fn new(task: Pin<Box<F>>) -> Self {
        Self {
            task,
            finished: None,
        }
    }

    fn poll_task(&mut self, cx: &mut Context<'_>) -> Poll<F::Output> {
        if let Some(output) = self.finished.take() {
            return Poll::Ready(output);
        }

        // Simple polling without waker tracking
        self.task.as_mut().poll(cx)
    }
}

impl<F: Future> Future for ScopeJoinHandle<F> {
    type Output = F::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.poll_task(cx)
    }
}

/// Spawns a new scoped task.
///
/// The task can borrow from the local environment because its lifetime is
/// tied to the returned `ScopeJoinHandle`, not to a global `'static` context.
pub fn scope<F>(fut: F) -> ScopeJoinHandle<F>
where
    F: Future,
{
    let task = Box::pin(fut);
    let mut handle = ScopeJoinHandle::new(task);

    let waker = waker_ref(&CURRENT_THREAD_NOTIFY.with(|n| n.borrow().clone()));
    let mut cx = Context::from_waker(&waker);

    if let Poll::Ready(output) = handle.poll_task(&mut cx) {
        handle.finished = Some(output);
    }

    handle
}

#[pin_project]
/// A future that waits for all futures in a collection to complete.
/// The output is a vector of the outputs of the futures, in the original order.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct JoinAll<F: Future> {
    #[pin]
    futures: Vec<F>,
    results: Vec<Option<F::Output>>,
    remaining: usize,
}

/// Creates a new future that will run a collection of futures to completion concurrently.
pub fn join_all<I>(futures: I) -> JoinAll<I::Item>
where
    I: IntoIterator,
    I::Item: Future,
{
    let items: Vec<_> = futures.into_iter().collect();
    let len = items.len();
    JoinAll {
        futures: items,
        results: (0..len).map(|_| None).collect(),
        remaining: len,
    }
}

impl<F: Future> Future for JoinAll<F> {
    type Output = Vec<F::Output>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        let mut made_progress = false;

        for i in 0..this.futures.len() {
            if this.results[i].is_none() {
                // This is the safe way to get a Pin<&mut F> from a Pin<&mut Vec<F>>.
                // It is `unsafe` because the caller must guarantee that they will not
                // move the `F` out of the resulting `Pin`. We uphold this by only
                // ever calling `poll` on it.
                let future_pin = unsafe { this.futures.as_mut().map_unchecked_mut(|f| &mut f[i]) };
                if let Poll::Ready(res) = future_pin.poll(cx) {
                    this.results[i] = Some(res);
                    *this.remaining -= 1;
                    made_progress = true;
                }
            }
        }

        if *this.remaining == 0 {
            Poll::Ready(
                this.results
                    .iter_mut()
                    .map(|res| res.take().unwrap())
                    .collect(),
            )
        } else if made_progress {
            cx.waker().wake_by_ref();
            Poll::Pending
        } else {
            Poll::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    /// The core of our single-threaded runtime. It blocks the current thread
    /// until the given future completes.
    fn block_on<F: Future>(future: F) -> F::Output {
        // Pin the future onto the stack.
        let mut future = pin!(future);

        // Create a waker that can unpark this thread.
        let waker = waker_ref(&CURRENT_THREAD_NOTIFY.with(|n| n.borrow().clone()));
        let mut cx = Context::from_waker(&waker);

        loop {
            match future.as_mut().poll(&mut cx) {
                Poll::Ready(output) => return output,
                Poll::Pending => {
                    // The future is not ready. Park the thread until the waker is called.
                    CURRENT_THREAD_NOTIFY.with(|notify| {
                        let notify = notify.borrow();
                        // We check the flag before parking to avoid a race condition
                        // where a wake-up happens between the poll and the park.
                        if !notify
                            .unparked()
                            .swap(false, std::sync::atomic::Ordering::Acquire)
                        {
                            thread::park();
                        }
                    });
                }
            }
        }
    }

    // An async function that borrows from its environment.
    async fn non_static_task(s: &str) -> String {
        format!("Hello, {}!", s)
    }

    #[test]
    fn test_scope_with_borrow() {
        let s = "tedio".to_string();
        let handle = scope(non_static_task(&s));

        let result = block_on(handle);
        assert_eq!(result, "Hello, tedio!");
    }

    #[test]
    fn test_nested_scopes() {
        let outer_str = "outer".to_string();

        let outer_task = scope(async {
            let inner_str = "inner".to_string();
            let inner_result = scope(async { format!("inner says: {inner_str}") }).await;
            assert_eq!(inner_result, "inner says: inner");

            format!("outer says: {outer_str}")
        });

        let _ref_outer = &outer_str;
        let result = block_on(outer_task);
        assert_eq!(result, "outer says: outer");
    }

    async fn create_task(name: &str) -> String {
        format!("task{}", name)
    }

    #[test]
    fn test_join_all() {
        let result = block_on(async {
            let futures = vec![
                scope(create_task("1")),
                scope(create_task("2")),
                scope(create_task("3")),
            ];

            join_all(futures).await
        });

        assert_eq!(result, vec!["task1", "task2", "task3"]);
    }
}
