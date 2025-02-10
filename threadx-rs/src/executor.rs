use core::{
    ffi::CStr,
    future::{Future, IntoFuture},
    mem,
    pin::{pin, Pin},
    task::{Context, Poll, Waker},
};

use crate::WaitOption::WaitForever;
use defmt::println;
use static_cell::StaticCell;
use threadx_sys::{TX_AND_CLEAR, TX_WAIT_FOREVER};

use crate::{
    event_flags::EventFlagsGroupHandle,
    mutex::{self, Mutex},
    queue::{Queue, QueueReceiver, QueueSender},
};
extern crate alloc;

#[derive(Clone, Copy)]
enum SignalState {
    Empty,
    Waiting,
    Notified,
}

struct Signal {
    state: Mutex<SignalState>,
    event_flag_handle: EventFlagsGroupHandle,
}

impl Signal {
    fn new(event_flag_handle: EventFlagsGroupHandle) -> Self {
        Self {
            state: Mutex::new(SignalState::Empty),
            event_flag_handle,
        }
    }

    fn wait(&self) {
        let mut state = self.state.lock(WaitForever).unwrap();
        match *state {
            // Notify() was called before we got here, consume it here without waiting and return immediately.
            SignalState::Notified => *state = SignalState::Empty,
            // This should not be possible because our signal is created within a function and never handed out to any
            // other threads. If this is the case, we have a serious problem so we panic immediately to avoid anything
            // more problematic happening.
            SignalState::Waiting => {
                unreachable!("Multiple threads waiting on the same signal: Open a bug report!");
            }
            SignalState::Empty => {
                // Nothing has happened yet, and we're the only thread waiting (as should be the case!). Set the state
                // accordingly and begin polling the condvar in a loop until it's no longer telling us to wait. The
                // loop prevents incorrect spurious wakeups.
                *state = SignalState::Waiting;
                // Release the mutex
                drop(state);
                // Wait for notification. TODO: What happens if we were preempted in between?
                self.event_flag_handle.get(
                    0x1,
                    crate::event_flags::GetOption::WaitAllAndClear,
                    WaitForever,
                ).unwrap();
            }
        }
    }

    fn notify(&self) {
        let mut state = self.state.lock(WaitForever).unwrap();
        match *state {
            // The signal was already notified, no need to do anything because the thread will be waking up anyway
            SignalState::Notified => {}
            // The signal wasn't notified but a thread isn't waiting on it, so we can avoid doing unnecessary work by
            // skipping the condvar and leaving behind a message telling the thread that a notification has already
            // occurred should it come along in the future.
            SignalState::Empty => *state = SignalState::Notified,
            // The signal wasn't notified and there's a waiting thread. Reset the signal so it can be wait()'ed on again
            // and wake up the thread. Because there should only be a single thread waiting, `notify_all` would also be
            // valid.
            SignalState::Waiting => {
                *state = SignalState::Empty;
                self.event_flag_handle.publish(0x1).unwrap()
            }
        }
    }
}

impl alloc::task::Wake for Signal {
    fn wake(self: alloc::sync::Arc<Self>) {
        self.notify();
    }

    fn wake_by_ref(self: &alloc::sync::Arc<Self>) {
        self.notify();
    }
}

/// Block the thread until the future is ready.
///
/// # Example
///
/// ```
/// let my_fut = async {};
/// let result = pollster::block_on(my_fut);
/// ```
pub fn block_on<F: IntoFuture>(fut: F, event_flag_handle: EventFlagsGroupHandle) -> F::Output {
    let mut fut = core::pin::pin!(fut.into_future());

    // Signal used to wake up the thread for polling as the future moves to completion. We need to use an `Arc`
    // because, although the lifetime of `fut` is limited to this function, the underlying IO abstraction might keep
    // the signal alive for far longer. `Arc` is a thread-safe way to allow this to happen.
    // TODO: Investigate ways to reuse this `Arc<Signal>`... perhaps via a `static`?
    let signal = alloc::sync::Arc::new(Signal::new(event_flag_handle));

    // Create a context that will be passed to the future.
    let waker = Waker::from(alloc::sync::Arc::clone(&signal));
    let mut context = Context::from_waker(&waker);

    // Poll the future to completion
    loop {
        match fut.as_mut().poll(&mut context) {
            Poll::Pending => signal.wait(),
            Poll::Ready(item) => break item,
        }
    }
}
