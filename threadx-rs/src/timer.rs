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
use core::mem;
use core::mem::transmute;

use super::error::TxError;
use defmt::println;
use num_traits::FromPrimitive;
use threadx_sys::_tx_timer_create;
use threadx_sys::TX_SUCCESS;
use threadx_sys::ULONG;

use core::mem::MaybeUninit;
use threadx_sys::TX_TIMER;

extern crate alloc;

type TimerCallbackType = unsafe extern "C" fn(ULONG);


unsafe extern "C" fn timer_callback_trampoline(arg: ULONG)
{
    let argc = arg as *mut alloc::boxed::Box<dyn Fn()>;
    
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
        _expiration_input: ULONG,
        initial_ticks: core::time::Duration,
        reschedule_ticks: core::time::Duration,
        auto_activate: bool,
    ) -> Result<(), TxError> {
        let timer = self.0.as_mut_ptr();

        //convert to a ULONG
        // Clarify?
        let expiration_function_ptr =  alloc::boxed::Box::into_raw(alloc::boxed::Box::new(expiration_function)) as *mut c_void;
        let expiration_function_arg = expiration_function_ptr as ULONG;


        let initial_ticks = TxTicks::from(initial_ticks).into();
        let reschedule_ticks = TxTicks::from(reschedule_ticks).into();
        let auto_activate = if auto_activate { 1 } else { 0 };

        let res = unsafe {
            _tx_timer_create(
                timer,
                name.as_ptr() as *mut i8,
                Some(timer_callback_trampoline),
                expiration_function_arg,
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
