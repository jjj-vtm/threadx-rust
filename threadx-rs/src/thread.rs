use core::ffi::{c_void, CStr};
use core::mem::MaybeUninit;
use core::time::Duration;

use threadx_sys::{_tx_thread_create, _tx_thread_resume, TX_THREAD, ULONG};
use threadx_sys::{_tx_thread_delete, _tx_thread_sleep, _tx_thread_suspend};

use crate::time::TxTicks;
use crate::tx_checked_call;

use super::error::TxError;
use defmt::error;
use num_traits::FromPrimitive;

extern crate alloc;

pub struct Thread {
    tx_struct: MaybeUninit<TX_THREAD>,
}

pub struct ThreadHandle {
    tx_ptr: *mut TX_THREAD,
}

pub struct UnInitialized;
pub struct Running;
pub struct Suspended;
pub struct Stopped;

impl Thread {
    pub const fn new() -> Thread {
        Thread {
            tx_struct: core::mem::MaybeUninit::uninit(),
        }
    }
}

fn __threadx_fn(_val: u32) {
    todo!()
}
unsafe extern "C" fn thread_trampoline<F>(arg: ULONG)
where
    F: Fn(),
{
    let closure = &mut *(arg as *mut F);
    closure();
}

unsafe extern "C" fn thread_box_callback_trampoline(arg: ULONG) {
    let argc: *mut alloc::boxed::Box<dyn FnOnce()> =
        core::ptr::with_exposed_provenance_mut(arg as usize);
    (argc.read())();
}

impl Thread {
    pub fn initialize_with_autostart_box(
        &'static mut self,
        name: &str,
        entry_function: alloc::boxed::Box<dyn FnOnce()>,
        stack: &'static mut [u8],
        priority: u32,
        preempt_threshold: u32,
        time_slice: u32,
    ) -> Result<ThreadHandle, TxError> {
        let expiration_function_ptr =
            alloc::boxed::Box::into_raw(alloc::boxed::Box::new(entry_function));
        //convert to a ULONG
        let entry_function_addr = expiration_function_ptr.expose_provenance() as ULONG;
        // Check that strlen < 31
        let mut local_name = [0u8; 32];
        local_name[..name.len()].copy_from_slice(name.as_bytes());

        tx_checked_call!(_tx_thread_create(
            // TODO: Ensure that threadx api does not modify this
            self.tx_struct.as_mut_ptr(),
            local_name.as_mut_ptr() as *mut i8,
            Some(thread_box_callback_trampoline),
            entry_function_addr,
            stack.as_mut_ptr() as *mut core::ffi::c_void,
            stack.len() as ULONG,
            priority as ULONG,
            preempt_threshold as ULONG,
            time_slice as ULONG,
            1
        ))
        .map(|_| ThreadHandle {
            tx_ptr: self.tx_struct.as_mut_ptr(),
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
    ) -> Result<Thread, TxError> {
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
        })
    }
}

impl ThreadHandle {
    pub fn start(&mut self) -> Result<ThreadHandle, TxError> {
        tx_checked_call!(_tx_thread_resume(self.tx_ptr))?;
        Ok(ThreadHandle {
            tx_ptr: self.tx_ptr,
        })
    }
    pub fn suspend(&mut self) -> Result<ThreadHandle, TxError> {
        tx_checked_call!(_tx_thread_suspend(self.tx_ptr))?;
        Ok(ThreadHandle {
            tx_ptr: self.tx_ptr,
        })
    }
    /// Deletes the thread. You need to pass ownership
    /// of the thread handle to this function.
    pub fn delete(self) -> Result<(), TxError> {
        tx_checked_call!(_tx_thread_delete(self.tx_ptr))
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
