/*
Copyright (c) 2020-2021 Joshua Barretto

Permission is hereby granted, free of charge, to any
person obtaining a copy of this software and associated
documentation files (the "Software"), to deal in the
Software without restriction, including without
limitation the rights to use, copy, modify, merge,
publish, distribute, sublicense, and/or sell copies of
the Software, and to permit persons to whom the Software
is furnished to do so, subject to the following
conditions:

The above copyright notice and this permission notice
shall be included in all copies or substantial portions
of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
DEALINGS IN THE SOFTWARE.
*/

use core::{
    future::{Future, IntoFuture},
    mem::MaybeUninit,
    sync::atomic::AtomicBool,
    task::{Context, Poll, Waker},
};

use crate::{event_flags::EventFlagsGroup, mutex::StaticMutex, WaitOption::WaitForever};
use static_cell::StaticCell;
use threadx_sys::TX_MUTEX_STRUCT;

use crate::event_flags::EventFlagsGroupHandle;
extern crate alloc;

/*
 * A port of the main parts of the pollster library using ThreadX components.
 */

#[derive(Clone, Copy, PartialEq)]
enum SignalState {
    Unused,
    Empty,
    Waiting,
    Notified,
}

static EXECUTOR_EVENT: StaticCell<EventFlagsGroup> = StaticCell::new();
static mut MUTEX_S: TX_MUTEX_STRUCT = unsafe { MaybeUninit::zeroed().assume_init() };
static SIGNALS: StaticMutex<[SignalState; 31]> =
    StaticMutex::new([SignalState::Unused; 31], &raw mut MUTEX_S);

struct Signal {
    state_index: usize,
    event_flag_handle: EventFlagsGroupHandle,
}
static EXECUTOR_INITIALIZED: AtomicBool = AtomicBool::new(false);
// TODO: Does not work for more then one signal. Should work for 32 (number of event_flags))
impl Signal {
    fn new(event_flag_handle: EventFlagsGroupHandle, index: usize) -> Self {
        Self {
            state_index: index,
            event_flag_handle: event_flag_handle,
        }
    }

    fn wait(&self) {
        let mut binding = SIGNALS.lock(WaitForever).unwrap();
        let state = binding.get_mut(self.state_index).unwrap();
        match *state {
            SignalState::Notified => *state = SignalState::Empty,
            SignalState::Waiting => {
                unreachable!("Multiple threads waiting on the same signal: Open a bug report!");
            }
            SignalState::Empty => {
                // Nothing has happened yet, and we're the only thread waiting (as should be the case!). Set the state
                // accordingly and begin polling the condvar in a loop until it's no longer telling us to wait. The
                // loop prevents incorrect spurious wakeups.
                *state = SignalState::Waiting;
                // Release the mutex.
                // Wait for notification. TODO: What happens if we were preempted in between? Wait can only be called by
                // the executor routine when the future was PENDING. What might happen is that between the Mutex drop and the
                // waiting for the event flag a notification came in ie.
                // MUTEX_UNLOCK --> Preempt Executor Thread --> notify_called on different thread.
                // ThreadX seems to guarantee no spurious wakeups so we can assume that this thread is only woken up after
                // the request flag is SIGNALED_X
                let requested_flag = 0b1 << self.state_index;
                drop(binding);
                self.event_flag_handle
                    .get(
                        requested_flag,
                        crate::event_flags::GetOption::WaitAllAndClear,
                        WaitForever,
                    )
                    .unwrap();
            }
            SignalState::Unused => todo!(),
        }
    }

    fn notify(&self) {
        let mut binding = SIGNALS.lock(WaitForever).unwrap();
        let state = binding.get_mut(self.state_index).unwrap();

        match *state {
            SignalState::Notified => {}
            SignalState::Empty => *state = SignalState::Notified,
            SignalState::Waiting => {
                *state = SignalState::Empty;
                self.event_flag_handle.publish(0x1).unwrap()
            }
            SignalState::Unused => todo!(),
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
#[derive(Clone, Copy)]
pub struct Executor {
    event_handle: EventFlagsGroupHandle,
}

impl Executor {
    pub fn new() -> Self {
        // Initialize the mutex on first call
        if EXECUTOR_INITIALIZED.load(core::sync::atomic::Ordering::Acquire) {
            panic!("Executor initialized twice");
        };

        SIGNALS.initialize(c"signal_mtx", false).unwrap();
        let evt = EXECUTOR_EVENT.init(EventFlagsGroup::new());
        let executor_event_handle = evt.initialize(c"ExecutorGroup").unwrap();

        EXECUTOR_INITIALIZED.store(true, core::sync::atomic::Ordering::Release);

        Executor {
            event_handle: executor_event_handle,
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
    pub fn block_on<F: IntoFuture>(&self, fut: F) -> F::Output {
        let mut fut = core::pin::pin!(fut.into_future());

        let unused_index = {
            let mut signals = SIGNALS.lock(WaitForever).unwrap();
            let idx = signals
                .into_iter()
                .position(|p| SignalState::Unused == p)
                .expect("No free task slots");
            *signals.get_mut(idx).unwrap() = SignalState::Empty;
            idx
        };

        // Signal used to wake up the thread for polling as the future moves to completion. We need to use an `Arc`
        // because, although the lifetime of `fut` is limited to this function, the underlying IO abstraction might keep
        // the signal alive for far longer. `Arc` is a thread-safe way to allow this to happen.
        // TODO: Investigate ways to reuse this `Arc<Signal>`... perhaps via a `static`?
        let signal = alloc::sync::Arc::new(Signal::new(self.event_handle, unused_index));

        // Create a context that will be passed to the future.
        let waker = Waker::from(alloc::sync::Arc::clone(&signal));
        let mut context = Context::from_waker(&waker);

        // Poll the future to completion
        let item = loop {
            match fut.as_mut().poll(&mut context) {
                Poll::Pending => signal.wait(),
                Poll::Ready(item) => break item,
            }
        };

        // Reset the signal
        let mut signals = SIGNALS.lock(WaitForever).unwrap();
        *signals.get_mut(unused_index).unwrap() = SignalState::Unused;

        item
    }
}
