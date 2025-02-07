/*
UINT        _tx_timer_activate(TX_TIMER *timer_ptr);
UINT        _tx_timer_change(TX_TIMER *timer_ptr, ULONG initial_ticks, ULONG reschedule_ticks);
UINT        _tx_timer_create(TX_TIMER *timer_ptr, CHAR *name_ptr,
                VOID (*expiration_function)(ULONG input), ULONG expiration_input,
                ULONG initial_ticks, ULONG reschedule_ticks, UINT auto_activate);
UINT        _tx_timer_deactivate(TX_TIMER *timer_ptr);
UINT        _tx_timer_delete(TX_TIMER *timer_ptr);
UINT        _tx_timer_info_get(TX_TIMER *timer_ptr, CHAR **name, UINT *active, ULONG *remaining_ticks,
                ULONG *reschedule_ticks, TX_TIMER **next_timer);
UINT        _tx_timer_performance_info_get(TX_TIMER *timer_ptr, ULONG *activates, ULONG *reactivates,
                ULONG *deactivates, ULONG *expirations, ULONG *expiration_adjusts);
UINT        _tx_timer_performance_system_info_get(ULONG *activates, ULONG *reactivates,
                ULONG *deactivates, ULONG *expirations, ULONG *expiration_adjusts);

ULONG       _tx_time_get(VOID);
VOID        _tx_time_set(ULONG new_time);
*/

use crate::time::TxTicks;
use core::ffi::c_void;
use core::ffi::CStr;

use super::error::TxError;
use defmt::println;
use num_traits::FromPrimitive;
use threadx_sys::_tx_timer_create;
use threadx_sys::TX_SUCCESS;
use threadx_sys::ULONG;

use core::mem::MaybeUninit;
use threadx_sys::TX_TIMER;

extern crate alloc;

// arg will point to the wide pointer of a dyn Fn()
unsafe extern "C" fn timer_callback_trampoline(arg: ULONG) {
    let argc: *mut alloc::boxed::Box<dyn Fn()> = core::ptr::with_exposed_provenance_mut(arg as usize);
    (*argc)();
}

pub struct Timer(MaybeUninit<TX_TIMER>);

impl Timer {
    pub const fn new() -> Self {
        Timer(MaybeUninit::uninit())
    }
    /// Using a closure we need the ULONG arg t_expiration_inpu to trampoline so you cannot use it directly
    pub fn initialize_with_closure(
        &'static mut self,
        name: &CStr,
        expiration_function: alloc::boxed::Box<dyn Fn()>,
        initial_ticks: core::time::Duration,
        reschedule_ticks: core::time::Duration,
        auto_activate: bool,
    ) -> Result<(), TxError> {
        let timer = self.0.as_mut_ptr();
        println!("Initialized stuff");

        // Calling into_raw on Box<dyn Fn()> gets a *mut dyn Fn() which is a wide pointer (https://doc.rust-lang.org/nomicon/exotic-sizes.html) ie. cannot directly be interpreted as a ULONG.
        // Therefore we box the pointer and call into_raw so expiration_function_ptr points to the wide pointer on the heap. This leaks two boxes ie. the closure and the pointer to the closure but those have to be valid for the whole time.
        let expiration_function_ptr =
            alloc::boxed::Box::into_raw(alloc::boxed::Box::new(expiration_function));

        //convert to a ULONG
        let expiration_fn_addr = expiration_function_ptr.expose_provenance() as ULONG;

        let initial_ticks = TxTicks::from(initial_ticks).into();
        let reschedule_ticks = TxTicks::from(reschedule_ticks).into();
        let auto_activate = if auto_activate { 1 } else { 0 };


        let res = unsafe {
            _tx_timer_create(
                timer,
                name.as_ptr() as *mut i8,
                Some(timer_callback_trampoline),
                expiration_fn_addr,
                initial_ticks,
                reschedule_ticks,
                auto_activate,
            )
        };
        // Manual error handling because the macro caused miscompilation
        if res != TX_SUCCESS {
            return Err(TxError::from_u32(res).unwrap());
        }

        Ok(())
    }

    pub fn initialize_with_fn(
        &'static mut self,
        name: &CStr,
        expiration_function: Option<unsafe extern "C" fn(ULONG)>,
        expiration_arg: u32,
        initial_ticks: core::time::Duration,
        reschedule_ticks: core::time::Duration,
        auto_activate: bool,
    ) -> Result<(), TxError> {
        let timer = self.0.as_mut_ptr();

        let initial_ticks = TxTicks::from(initial_ticks).into();
        let reschedule_ticks = TxTicks::from(reschedule_ticks).into();
        let auto_activate = if auto_activate { 1 } else { 0 };

        let res = unsafe {
            _tx_timer_create(
                timer,
                name.as_ptr() as *mut i8,
                expiration_function,
                expiration_arg,
                initial_ticks,
                reschedule_ticks,
                auto_activate,
            )
        };
        // Manual error handling because the macro caused miscompilation
        if res != TX_SUCCESS {
            return Err(TxError::from_u32(res).unwrap());
        }

        Ok(())
    }
}
