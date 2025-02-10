use core::ffi::CStr;
use core::marker::PhantomData;
use core::mem::MaybeUninit;

use threadx_sys::{_tx_event_flags_create, TX_EVENT_FLAGS_GROUP};
use threadx_sys::{
    _tx_event_flags_delete, _tx_event_flags_get, _tx_event_flags_set, _tx_event_flags_set_notify,
    ULONG,
};

use crate::tx_checked_call;

use super::error::TxError;
use super::WaitOption;
use defmt::error;
use defmt::trace;
use num_traits::FromPrimitive;

#[derive(Copy, Clone)]
#[repr(u32)]
pub enum GetOption {
    WaitAll = threadx_sys::TX_AND,
    WaitAllAndClear = threadx_sys::TX_AND_CLEAR,
    WaitAny = threadx_sys::TX_OR,
    WaitAnyAndClear = threadx_sys::TX_OR_CLEAR,
}

#[derive(Copy, Clone)]
#[repr(u32)]
pub enum SetOption {
    SetAndClear = threadx_sys::TX_AND,
    SetAny = threadx_sys::TX_OR,
}

pub struct EventFlagsGroup {
    flag_group: MaybeUninit<TX_EVENT_FLAGS_GROUP>,
}
#[derive(Copy, Clone)]

pub struct EventFlagsGroupHandle {
    pub flag_group_ptr: *mut TX_EVENT_FLAGS_GROUP,
}
/// Safety: Interaction with this pointer is only done via get/publish methods which is safe to do from different threads 
unsafe impl Send for EventFlagsGroupHandle {}
unsafe impl Sync for EventFlagsGroupHandle {}

pub struct UnInitialized;
pub struct Initialized;

impl EventFlagsGroup {
    pub const fn new() -> EventFlagsGroup {
        EventFlagsGroup {
            flag_group: core::mem::MaybeUninit::uninit(),
        }
    }
}

impl EventFlagsGroup {
    // Since this takes a mut borrow for 'static it cannot be initialized twice.
    pub fn initialize(&'static mut self, name: &CStr) -> Result<EventFlagsGroupHandle, TxError> {
        let group_ptr = self.flag_group.as_mut_ptr();

        tx_checked_call!(_tx_event_flags_create(group_ptr, name.as_ptr() as *mut i8))?;
        Ok(EventFlagsGroupHandle {
            flag_group_ptr: group_ptr,
        })
    }
}

impl EventFlagsGroupHandle {
    pub fn publish(&self, flags_to_set: u32) -> Result<(), TxError> {
        tx_checked_call!(_tx_event_flags_set(self.flag_group_ptr, flags_to_set, 0))
    }

    pub fn get(
        &self,
        requested_flags: u32,
        get_option: GetOption,
        wait_option: WaitOption,
    ) -> Result<u32, TxError> {
        let mut actual_flags = 0u32;
        tx_checked_call!(_tx_event_flags_get(
            self.flag_group_ptr,
            requested_flags,
            get_option as ULONG,
            &mut actual_flags,
            wait_option as ULONG
        ))?;
        Ok(actual_flags)
    }
}
