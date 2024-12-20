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
use defmt::{debug, println, trace};
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

pub struct EventFlagsGroup<STATE> {
    flag_group: MaybeUninit<TX_EVENT_FLAGS_GROUP>,
    _state: PhantomData<STATE>,
}

struct Uninitialized;
struct Initialized;

impl<STATE> EventFlagsGroup<STATE> {
    pub const fn new() -> EventFlagsGroup<Uninitialized> {
        EventFlagsGroup {
            flag_group: core::mem::MaybeUninit::uninit(),
            _state: PhantomData::<Uninitialized>,
        }
    }
}

impl EventFlagsGroup<Uninitialized> {
    pub fn initialize(&'static mut self, name: &CStr) -> Result<(), TxError> {
        let group_ptr = self.flag_group.as_mut_ptr();

        trace!("EventFlagsGroup::initialize: ptr is: {}", group_ptr);
        tx_checked_call!(_tx_event_flags_create(group_ptr, name.as_ptr() as *mut i8))?;
        Ok(())
    }
}

impl EventFlagsGroup<Initialized> {
    pub fn publish(&mut self, flags_to_set: u32) -> Result<(), TxError> {
        let group_ptr = self.flag_group.as_mut_ptr();

        tx_checked_call!(_tx_event_flags_set(group_ptr, flags_to_set, 0))
    }

    pub fn get(
        & mut self,
        requested_flags: u32,
        get_option: GetOption,
        wait_option: WaitOption,
    ) -> Result<u32, TxError> {
        let group_ptr = self.flag_group.as_mut_ptr();

        let mut actual_flags = 0u32;
        tx_checked_call!(_tx_event_flags_get(
            group_ptr,
            requested_flags,
            get_option as ULONG,
            &mut actual_flags,
            wait_option as ULONG
        ))?;
        Ok(actual_flags)
    }
}