//! Scoped async execution implementation.
//!
//! This module provides the core functionality for scoped task execution
//! with support for non-'static lifetimes.

use crate::waker::{NestedThreadNotify, waker_ref};
use futures::FutureExt;
use futures::channel::oneshot::{self, Receiver as OneshotReceiver};
use futures::future::Fuse;
use pin_project::{pin_project, pinned_drop};
use std::future::{Future, poll_fn};
use std::marker::PhantomData;
use std::pin::{Pin, pin};
use std::rc::Rc;
use std::task::{Context, Poll};

#[pin_project(PinnedDrop)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct ScopeExecutor<'env, 'scope: 'env> {
    depth: usize,
    #[pin]
    leafs: Vec<ScopeTask<'scope>>,
    is_leafs_terminated: Vec<bool>,
    remaining: usize,
    scope: Option<Rc<NestedThreadNotify<'env, 'scope>>>,

    _env: PhantomData<&'env ()>,
}

impl<'env, 'scope: 'env> ScopeExecutor<'env, 'scope> {
    pub fn new(depth: usize) -> Self {
        println!("Creating ScopeExecutor: depth={depth}");

        Self {
            depth,
            leafs: Vec::new(),
            is_leafs_terminated: Vec::new(),
            remaining: 0,
            scope: None,
            _env: PhantomData,
        }
    }

    pub fn spawn(&'scope mut self, fut: ScopeTask<'scope>) -> usize {
        let depth = self.depth;
        self.leafs.push(fut);
        self.is_leafs_terminated.push(false);
        self.remaining += 1;

        println!(
            "[ScopeExecutor]Spawning future at depth {depth}, leafs: {}",
            self.leafs.len()
        );

        depth
    }
}

impl<'env, 'scope: 'env> Future for ScopeExecutor<'env, 'scope> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        if *this.remaining == 0 {
            return Poll::Ready(());
        }

        let scope = this
            .scope
            .get_or_insert_with(|| Rc::new(NestedThreadNotify::new(*this.depth + 1)));
        let waker = waker_ref(scope);
        let cx = &mut Context::from_waker(&waker);

        println!(
            "[ScopeExecutor]Polling ScopeExecutor, depth: {}, leafs: {}, remaining: {}",
            *this.depth,
            this.leafs.len(),
            *this.remaining
        );

        let mut made_progress = false;

        for i in 0..this.leafs.len() {
            if !this.is_leafs_terminated[i] && this.leafs[i].poll_unpin(cx).is_ready() {
                this.is_leafs_terminated[i] = true;
                *this.remaining -= 1;
                made_progress = true;
                continue;
            }
        }

        println!(
            "[ScopeExecutor]Executor Polled: remaining {}, depth: {}, made_progress: {made_progress}",
            *this.remaining, *this.depth
        );

        if *this.remaining == 0 {
            Poll::Ready(())
        } else if made_progress {
            cx.waker().wake_by_ref();
            Poll::Pending
        } else {
            Poll::Pending
        }
    }
}

#[pinned_drop]
impl PinnedDrop for ScopeExecutor<'_, '_> {
    fn drop(self: Pin<&mut Self>) {
        // let this = self.project();
        // TODO: wait for finished.
        println!(
            "Drop ScopeExecutor: {}, remaining: {}",
            self.depth, self.remaining
        );
    }
}

#[pin_project]
pub struct ScopeTask<'scope> {
    fut: Pin<Box<dyn Future<Output = ()> + 'scope>>,
    label: &'scope str,
    is_terminated: bool,
    _scope: PhantomData<&'scope ()>,
}

impl<'scope> ScopeTask<'scope> {
    pub fn new<T, F>(label: &'scope str, fut: F) -> (Self, OneshotReceiver<F::Output>)
    where
        T: std::fmt::Debug + 'scope,
        F: Future<Output = T> + 'scope,
    {
        let (optput_sender, receiver) = oneshot::channel();

        let fut = Box::pin(async move {
            let output = fut.await;
            if let Err(_error) = optput_sender.send(output) {
                // TODO: Handle error
            }
        });

        let task = Self {
            fut,
            label,
            is_terminated: false,
            _scope: PhantomData,
        };
        (task, receiver)
    }
}

impl<'scope> Future for ScopeTask<'scope> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        if *this.is_terminated {
            return Poll::Ready(());
        };

        this.fut.poll_unpin(cx)
    }
}

/// A handle to a scoped task.
///
/// The future is stored inside this handle, ensuring its lifetime is tied
/// to the scope where it was created.
/// If the handle is dropped, the task is cancelled.
#[pin_project(PinnedDrop)]
// #[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct ScopeGuard<F: Future> {
    receiver: Fuse<oneshot::Receiver<F::Output>>,
    is_terminated: bool,
}

impl<F: Future> ScopeGuard<F> {
    /// Creates a new `ScopeGuard` from a future.
    fn new(receiver: oneshot::Receiver<F::Output>) -> Self {
        let receiver = receiver.fuse();
        Self {
            receiver,
            is_terminated: false,
        }
    }
}

#[pinned_drop]
impl<F: Future> PinnedDrop for ScopeGuard<F> {
    fn drop(self: Pin<&mut Self>) {
        // TODO: wait for finished.
        println!("ScopeGuard Dropped");
    }
}

impl<F: Future> Future for ScopeGuard<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        if *this.is_terminated {
            return Poll::Pending;
        }

        if let Poll::Ready(Ok(output)) = this.receiver.poll_unpin(cx) {
            *this.is_terminated = true;
            return Poll::Ready(output);
        };

        Poll::Pending
    }
}

#[pin_project]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Scope<'scope, F: 'scope> {
    fut: Option<F>,
    label: &'scope str,
    _scope: PhantomData<&'scope ()>,
}

impl<'scope, F: Future + 'scope> Scope<'scope, F> {
    pub fn new(label: &'scope str, fut: F) -> Self {
        Scope {
            fut: Some(fut),
            label,
            _scope: PhantomData,
        }
    }
}

impl<'scope, T, F> Future for Scope<'scope, F>
where
    T: std::fmt::Debug + 'scope,
    F: Future<Output = T> + 'scope,
{
    type Output = ScopeGuard<F>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        // Move the inner future out exactly once and spawn it.
        let fut = this
            .fut
            .take()
            .expect("`scope` polled after completion: future already taken");
        let (task, receiver) = ScopeTask::new(this.label, fut);
        let waker = unsafe { &mut *cx.waker().data().cast::<NestedThreadNotify>().cast_mut() };
        waker.executor.spawn(task);
        Poll::Ready(ScopeGuard::new(receiver))
    }
}

#[must_use = "futures do nothing unless you `.await` or poll them"]
pub fn scope<'scope, T: std::fmt::Debug + 'scope, F: Future<Output = T> + 'scope>(
    label: &'scope str,
    fut: F,
) -> impl Future<Output = ScopeGuard<F>> + 'scope {
    Scope::new(label, fut)
}

pub fn wait() -> impl Future<Output = ()> {
    println!("Waiting...");
    poll_fn(move |cx| {
        let waker = unsafe { &mut *cx.waker().data().cast::<NestedThreadNotify>().cast_mut() };
        println!("[waitter]Executor polled, depth: {}", waker.depth);
        waker.executor.poll_unpin(cx)
    })
}

#[cfg(test)]
mod tests {
    use std::thread;

    use super::*;

    /// The core of our single-threaded runtime. It blocks the current thread
    /// until the given future completes.
    fn block_on<F: Future>(future: F) -> F::Output {
        let mut future = pin!(future);

        let executor = Rc::new(NestedThreadNotify::new(0));
        let waker = waker_ref(&executor);
        let mut cx = Context::from_waker(&waker);

        loop {
            match future.as_mut().poll(&mut cx) {
                Poll::Ready(output) => {
                    println!("Future completed, break");
                    return output;
                }
                Poll::Pending => {
                    println!("Future pending, poll again");
                    // only for test, poll again
                    if !executor
                        .unparked()
                        .swap(false, std::sync::atomic::Ordering::Acquire)
                    {
                        println!("Thread parked");
                        thread::park();
                    }
                }
            }
        }
    }

    #[test]
    fn test_nested_scope_2() {
        async fn print_depth_and_index(depth: usize, index: usize, message: &str) {
            println!("[{depth}:{index}]: {message}");
        }

        let fut = async {
            let str = "hello world".to_string();
            scope("s00", print_depth_and_index(0, 0, &str)).await;
            scope("s01", async {
                let str = "wellcome to level one".to_string();
                print_depth_and_index(0, 1, &str).await;
                scope("s10", print_depth_and_index(1, 0, &str)).await;
                scope("s11", print_depth_and_index(1, 1, &str)).await;
                scope("s12", print_depth_and_index(1, 2, &str)).await;
                wait().await
            })
            .await;
            scope("s02", print_depth_and_index(0, 2, &str)).await;

            wait().await
        };
        block_on(fut);
        panic!("");
    }

    #[test]
    fn test_scope() {
        async fn print_depth_and_index(depth: usize, index: usize) -> (usize, usize) {
            println!("Depth: {depth}, Index: {index}");
            (depth, index)
        }

        let fut = async {
            scope("s00", print_depth_and_index(0, 0)).await;
            scope("s01", print_depth_and_index(0, 1)).await;
            scope("s02", print_depth_and_index(0, 2)).await;

            wait().await
        };
        block_on(fut);
    }
}
