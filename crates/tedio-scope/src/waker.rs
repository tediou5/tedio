//! Local wake implementation for the executor.
use std::mem::{self, ManuallyDrop};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{LocalWake, RawWaker, RawWakerVTable, Waker};
use std::thread::{self, Thread};

pub struct ThreadNotify {
    /// The (single) executor thread.
    thread: Thread,
    /// A flag to ensure a wakeup (i.e. `unpark()`) is not "forgotten"
    /// before the next `park()`, which may otherwise happen if the code
    /// being executed as part of the future(s) being polled makes use of
    /// park / unpark calls of its own, i.e. we cannot assume that no other
    /// code uses park / unpark on the executing `thread`.
    pub unparked: AtomicBool,
}

impl ThreadNotify {
    pub fn new() -> ThreadNotify {
        ThreadNotify {
            thread: thread::current(),
            unparked: AtomicBool::new(false),
        }
    }

    pub fn unparked(&self) -> &AtomicBool {
        &self.unparked
    }
}

impl Default for ThreadNotify {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalWake for ThreadNotify {
    fn wake(self: Rc<Self>) {
        let unparked = self.unparked.swap(true, Ordering::Release);
        if !unparked {
            // If the thread has not been unparked yet, it must be done
            // now. If it was actually parked, it will run again,
            // otherwise the token made available by `unpark`
            // may be consumed before reaching `park()`, but `unparked`
            // ensures it is not forgotten.
            self.thread.unpark();
        }
    }

    fn wake_by_ref(self: &Rc<Self>) {
        self.clone().wake();
    }
}

fn waker_vtable<W: LocalWake + 'static>() -> &'static RawWakerVTable {
    &RawWakerVTable::new(
        clone_rc_raw::<W>,
        wake_rc_raw::<W>,
        wake_by_ref_rc_raw::<W>,
        drop_rc_raw::<W>,
    )
}

#[allow(clippy::redundant_clone)] // The clone here isn't actually redundant.
unsafe fn increase_refcount<T: LocalWake + 'static>(data: *const ()) {
    // Retain Arc, but don't touch refcount by wrapping in ManuallyDrop
    let rc = mem::ManuallyDrop::new(unsafe { Rc::<T>::from_raw(data.cast::<T>()) });
    // Now increase refcount, but don't drop new refcount either
    let _rc_clone: mem::ManuallyDrop<_> = rc.clone();
}

// used by `waker_ref`
#[inline(always)]
unsafe fn clone_rc_raw<T: LocalWake + 'static>(data: *const ()) -> RawWaker {
    unsafe { increase_refcount::<T>(data) }
    RawWaker::new(data, waker_vtable::<T>())
}

unsafe fn wake_rc_raw<T: LocalWake + 'static>(data: *const ()) {
    let rc: Rc<T> = unsafe { Rc::from_raw(data.cast::<T>()) };
    LocalWake::wake(rc);
}

// used by `waker_ref`
unsafe fn wake_by_ref_rc_raw<T: LocalWake + 'static>(data: *const ()) {
    // Retain Rc, but don't touch refcount by wrapping in ManuallyDrop
    let rc = mem::ManuallyDrop::new(unsafe { Rc::<T>::from_raw(data.cast::<T>()) });
    LocalWake::wake_by_ref(&rc);
}

unsafe fn drop_rc_raw<T: LocalWake + 'static>(data: *const ()) {
    drop(unsafe { Rc::<T>::from_raw(data.cast::<T>()) })
}

pub fn waker_ref<W>(wake: &Rc<W>) -> ManuallyDrop<Waker>
where
    W: LocalWake + 'static,
{
    let ptr = Rc::as_ptr(wake).cast::<()>();
    ManuallyDrop::new(unsafe { Waker::from_raw(RawWaker::new(ptr, waker_vtable::<W>())) })
}
