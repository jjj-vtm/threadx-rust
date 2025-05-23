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
    sync::atomic::AtomicBool,
    task::{Context, Poll, Waker},
};

use crate::{
    WaitOption::WaitForever,
    error::TxError,
    event_flags::EventFlagsGroup,
    mutex::{Mutex, MutexError},
};
use static_cell::StaticCell;

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

#[derive(Debug)]
pub enum ExecutorError {
    NoFreeSlots,
    IndexOutOfBounds,
    MutexError,
    ThreadxError,
}

impl From<TxError> for ExecutorError {
    fn from(_value: TxError) -> Self {
        ExecutorError::ThreadxError
    }
}
impl From<MutexError> for ExecutorError {
    fn from(_value: MutexError) -> Self {
        ExecutorError::MutexError
    }
}

static EXECUTOR_EVENT: StaticCell<EventFlagsGroup> = StaticCell::new();

static SIGNALS: StaticCell<Mutex<[SignalState; 31]>> = StaticCell::new();

struct Signal {
    state_index: usize,
    event_flag_handle: EventFlagsGroupHandle,
    signal_mtx: &'static Mutex<[SignalState; 31]>,
}
static EXECUTOR_INITIALIZED: AtomicBool = AtomicBool::new(false);
impl Signal {
    fn new(
        event_flag_handle: EventFlagsGroupHandle,
        index: usize,
        signal_mtx: &'static Mutex<[SignalState; 31]>,
    ) -> Self {
        Self {
            state_index: index,
            event_flag_handle,
            signal_mtx,
        }
    }

    fn wait(&self) -> Result<(), ExecutorError> {
        let mut signals = self.signal_mtx.lock(WaitForever)?;
        let Some(state) = signals.get_mut(self.state_index) else {
            return Err(ExecutorError::IndexOutOfBounds);
        };
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
                drop(signals);
                self.event_flag_handle.get(
                    requested_flag,
                    crate::event_flags::GetOption::WaitAllAndClear,
                    WaitForever,
                )?;
            }
            SignalState::Unused => todo!(),
        }
        Ok(())
    }

    fn notify(&self) -> Result<(), ExecutorError> {
        let mut signals = self.signal_mtx.lock(WaitForever)?;
        let Some(state) = signals.get_mut(self.state_index) else {
            return Err(ExecutorError::IndexOutOfBounds);
        };
        let requested_flag = 0b1 << self.state_index;
        match *state {
            SignalState::Notified => {}
            SignalState::Empty => *state = SignalState::Notified,
            SignalState::Waiting => {
                *state = SignalState::Empty;
                self.event_flag_handle.publish(requested_flag)?
            }
            SignalState::Unused => {
                defmt::warn!(
                    "Ignoring notification, waker is connected to interrupt but nobody is listening"
                )
            }
        }
        Ok(())
    }
}

impl alloc::task::Wake for Signal {
    fn wake(self: alloc::sync::Arc<Self>) {
        // Possible errors are IndexOutOfBounds, corrupted event_flag pointer and wrong set_data
        // see
        // https://github.com/eclipse-threadx/rtos-docs/blob/main/rtos-docs/threadx/chapter4.md#tx_event_flags_set)
        // All of them are unrecoverable so we will panic.
        self.notify().expect("Invalid state of the excutor.");
    }

    fn wake_by_ref(self: &alloc::sync::Arc<Self>) {
        self.notify().expect("Invalid state of the excutor.");
    }
}
#[derive(Clone, Copy)]
pub struct Executor {
    event_handle: EventFlagsGroupHandle,
    signal_mtx: &'static Mutex<[SignalState; 31]>,
}

impl Executor {
    pub fn new() -> Result<Self, ExecutorError> {
        // Initialize the mutex on first call
        if EXECUTOR_INITIALIZED.load(core::sync::atomic::Ordering::Acquire) {
            panic!("Executor initialized twice");
        };
        let signal_ref = SIGNALS.init(Mutex::new([SignalState::Unused; 31]));
        let mut signal_mtx = core::pin::Pin::static_mut(signal_ref);
        //TODO: Correct error mapping.
        signal_mtx.as_mut().initialize(c"signal_mtx", false)?;
        let evt = EXECUTOR_EVENT.init(EventFlagsGroup::new());
        let executor_event_handle = evt.initialize(c"ExecutorGroup")?;

        EXECUTOR_INITIALIZED.store(true, core::sync::atomic::Ordering::Release);

        Ok(Executor {
            event_handle: executor_event_handle,
            //Safety: We give out a shared reference so the mutex struct cannot be moved
            signal_mtx: unsafe { signal_mtx.get_unchecked_mut() },
        })
    }
    /// Block the thread until the future is ready.
    ///
    /// # Example
    ///
    /// ```
    /// let my_fut = async {};
    /// let result = threadx-rs::block_on(my_fut);
    /// ```
    pub fn block_on<F: IntoFuture>(&self, fut: F) -> Result<F::Output, ExecutorError> {
        let mut fut = core::pin::pin!(fut.into_future());

        let unused_index = {
            let mut signals = self.signal_mtx.lock(WaitForever)?;
            let Some(idx) = signals.into_iter().position(|p| SignalState::Unused == p) else {
                return Err(ExecutorError::NoFreeSlots);
            };
            let Some(state) = signals.get_mut(idx) else {
                return Err(ExecutorError::IndexOutOfBounds);
            };
            *state = SignalState::Empty;

            idx
        };
        // Signal used to wake up the thread for polling as the future moves to completion. We need to use an `Arc`
        // because, although the lifetime of `fut` is limited to this function, the underlying IO abstraction might keep
        // the signal alive for far longer. `Arc` is a thread-safe way to allow this to happen.
        // TODO: Investigate ways to reuse this `Arc<Signal>`... perhaps via a `static`?
        let signal = alloc::sync::Arc::new(Signal::new(
            self.event_handle,
            unused_index,
            self.signal_mtx,
        ));

        // Create a context that will be passed to the future.
        let waker = Waker::from(alloc::sync::Arc::clone(&signal));
        let mut context = Context::from_waker(&waker);

        // Poll the future to completion
        let item = loop {
            match fut.as_mut().poll(&mut context) {
                Poll::Pending => {
                    signal.wait()?;
                }
                Poll::Ready(item) => break item,
            }
        };

        // Reset the signal
        let mut signals = self.signal_mtx.lock(WaitForever)?;
        let Some(state) = signals.get_mut(unused_index) else {
            // Should not happen since unused_index is obtained from signals
            return Err(ExecutorError::IndexOutOfBounds);
        };

        *state = SignalState::Unused;
        Ok(item)
    }
}
