use core::ffi::{c_void, CStr};
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::time::Duration;

use threadx_sys::{_tx_thread_create, _tx_thread_resume, TX_THREAD, ULONG};
use threadx_sys::{_tx_thread_delete, _tx_thread_sleep, _tx_thread_suspend};

use crate::time::TxTicks;
use crate::tx_checked_call;

use super::error::TxError;
use defmt::error;
use num_traits::FromPrimitive;

pub struct Thread<STATE> {
    tx_struct: MaybeUninit<TX_THREAD>,
    state: PhantomData<STATE>
}

pub struct UnInitialized;
pub struct Running;
pub struct Suspended;


type TxThreadEntry = unsafe extern "C" fn(ULONG);

impl<STATE> Thread<STATE> {
    pub const fn new() -> Thread<UnInitialized> {
        Thread {
            tx_struct: core::mem::MaybeUninit::zeroed(),
            state: PhantomData::<UnInitialized>,
        }
    }
}

unsafe extern "C" fn thread_trampoline<F>(arg: ULONG)
where
    F: Fn(),
{
    let closure = &mut *(arg as *mut F);
    closure();
}

fn get_trampoline<F>(closure: &F) -> TxThreadEntry
where
    F: Fn(),
{
    thread_trampoline::<F>
}

impl Thread<UnInitialized> {
    pub fn initialize<F: Fn()>(
        &'static mut self,
        name: &str,
        mut entry_function: F,
        stack: *mut [u8],
        priority: u32,
        preempt_threshold: u32,
        time_slice: u32,
        auto_start: bool,
    ) -> Result<Thread<Running>, TxError> {
        //convert entry function into a pointer
        let entry_function_ptr = &mut entry_function as *mut _ as *mut c_void;
        //convert to a ULONG
        let entry_function_arg = entry_function_ptr as ULONG;
        let trampoline = get_trampoline(&entry_function);

        // Check that strlen < 31
        let mut local_name= [0u8; 32];
        local_name[..name.len()].copy_from_slice(name.as_bytes());

        tx_checked_call!(_tx_thread_create(
            // TODO: Ensure that threadx api does not modify this
            self.tx_struct.as_mut_ptr(),
            local_name.as_mut_ptr() as *mut i8,
            Some(trampoline),
            entry_function_arg,
            stack as *mut core::ffi::c_void,
            stack.len() as ULONG,
            priority as ULONG,
            preempt_threshold as ULONG,
            time_slice as ULONG,
            if auto_start { 1 } else { 0 }
        ))
        .map(|_| Thread {
            tx_struct: self.tx_struct,
            state: PhantomData::<Running>,
        })
    }
    pub fn create_with_c_func(
        &mut self,
        name: &CStr,
        entry_function: Option<unsafe extern "C" fn(ULONG)>,
        arg: ULONG,
        stack: &mut [u8],
        priority: u32,
        preempt_threshold: u32,
        time_slice: u32,
        auto_start: bool,
    ) -> Result<Thread<Running>, TxError> {
        // check if already initialized.
        let s = unsafe { &*self.tx_struct.as_ptr() };
        if !s.tx_thread_name.is_null() {
            panic!("Thread must be initialized only once");
        }
        tx_checked_call!(_tx_thread_create(
            // TODO: Ensure that threadx api does not modify this
            self.tx_struct.as_mut_ptr(),
            name.as_ptr() as *mut i8,
            entry_function,
            arg,
            stack.as_mut_ptr() as *mut core::ffi::c_void,
            stack.len() as ULONG,
            priority as ULONG,
            preempt_threshold as ULONG,
            time_slice as ULONG,
            if auto_start { 1 } else { 0 }
        ))
        .map(|_| Thread {
            tx_struct: self.tx_struct,
            state: PhantomData::<Running>,
        })
    }
}

impl Thread<Running> {
    pub fn start(&mut self) -> Result<(), TxError> {
        tx_checked_call!(_tx_thread_resume(self.tx_struct.as_mut_ptr()))
    }

    pub fn suspend(&mut self) -> Result<(), TxError> {
        tx_checked_call!(_tx_thread_suspend(self.tx_struct.as_mut_ptr()))
    }

    /// Deletes the thread. You need to pass ownership
    /// of the thread handle to this function.
    pub fn delete(mut self) -> Result<(), TxError> {
        tx_checked_call!(_tx_thread_delete(self.tx_struct.as_mut_ptr()))
    }
}

/// Put the current task to sleep for the specified duration. Note that
/// the minimum sleep time is 1 os tick and the wall time that represents
/// will be rounded up to the nearest tick.  So if the os tick is 10ms,
/// which is the default, and you sleep for 1ms, you will actually sleep
/// for 10ms. The number of ticks per second is a compile time constant
/// available at `threadx-sys::TX_TICKS_PER_SECOND`
pub fn sleep(d: Duration) -> Result<(), TxError> {
    tx_checked_call!(_tx_thread_sleep(TxTicks::from(d).into()))
}
