/*
UINT        _tx_queue_create(TX_QUEUE *queue_ptr, CHAR *name_ptr, UINT message_size,
                        VOID *queue_start, ULONG queue_size);
UINT        _tx_queue_delete(TX_QUEUE *queue_ptr);
UINT        _tx_queue_flush(TX_QUEUE *queue_ptr);
UINT        _tx_queue_info_get(TX_QUEUE *queue_ptr, CHAR **name, ULONG *enqueued, ULONG *available_storage,
                    TX_THREAD **first_suspended, ULONG *suspended_count, TX_QUEUE **next_queue);
UINT        _tx_queue_performance_info_get(TX_QUEUE *queue_ptr, ULONG *messages_sent, ULONG *messages_received,
                    ULONG *empty_suspensions, ULONG *full_suspensions, ULONG *full_errors, ULONG *timeouts);
UINT        _tx_queue_performance_system_info_get(ULONG *messages_sent, ULONG *messages_received,
                    ULONG *empty_suspensions, ULONG *full_suspensions, ULONG *full_errors, ULONG *timeouts);
UINT        _tx_queue_prioritize(TX_QUEUE *queue_ptr);
UINT        _tx_queue_receive(TX_QUEUE *queue_ptr, VOID *destination_ptr, ULONG wait_option);
UINT        _tx_queue_send(TX_QUEUE *queue_ptr, VOID *source_ptr, ULONG wait_option);
UINT        _tx_queue_send_notify(TX_QUEUE *queue_ptr, VOID (*queue_send_notify)(TX_QUEUE *notify_queue_ptr));
UINT        _tx_queue_front_send(TX_QUEUE *queue_ptr, VOID *source_ptr, ULONG wait_option);

*/

use super::{error::TxError, WaitOption};
use crate::pool::MemoryBlock;
use crate::tx_checked_call;
use core::mem::size_of;
use core::{ffi::CStr, mem::MaybeUninit};
use defmt::error;
use num_traits::FromPrimitive;
use threadx_sys::{_tx_queue_create, _tx_queue_receive, _tx_queue_send, TX_QUEUE, ULONG};

pub struct Queue<T>(MaybeUninit<TX_QUEUE>, core::marker::PhantomData<T>);

impl<T> Queue<T> {
    // according to the threadx docs, the supported messages sizes are 1 to 16 32 bit words
    const SIZE_OK: () =
        assert!(size_of::<T>() >= size_of::<u32>() && size_of::<T>() <= (size_of::<u32>() * 16));

    pub const fn new() -> Self {
        let _ = Self::SIZE_OK;
        Queue(core::mem::MaybeUninit::uninit(), core::marker::PhantomData)
    }
    //TODO: Queue must not necessary live for 'static but can live as long as the memory block does
    pub fn initialize(
        &'static mut self,
        name: &CStr,
        queue_memory: &'static mut [u8],
    ) -> Result<(QueueSender<T>, QueueReceiver<T>), TxError> {
        let queue_ptr = self.0.as_mut_ptr();

        tx_checked_call!(_tx_queue_create(
            queue_ptr,
            name.as_ptr() as *mut i8,
            size_of::<T>() as ULONG,
            queue_memory.as_mut_ptr() as *mut core::ffi::c_void,
            queue_memory.len() as ULONG
        ))
        .map(|_| {
            (
                QueueSender(queue_ptr, core::marker::PhantomData),
                QueueReceiver(queue_ptr, core::marker::PhantomData),
            )
        })
    }
}

#[derive(Clone)]
pub struct QueueSender<T>(*mut TX_QUEUE, core::marker::PhantomData<T>);
unsafe impl<T> Send for QueueSender<T>{}

unsafe impl<T> Sync for QueueSender<T>{}
    
pub struct QueueReceiver<T>(*mut TX_QUEUE, core::marker::PhantomData<T>);
unsafe impl<T> Send for QueueReceiver<T>{}

impl<T> QueueSender<T> {
    pub fn send(&self, message: T, wait: WaitOption) -> Result<(), TxError> {
        tx_checked_call!(_tx_queue_send(
            self.0,
            &message as *const T as *mut core::ffi::c_void,
            wait as ULONG
        ))
    }
}

impl<T> QueueReceiver<T> {
    pub fn receive(&self, wait: WaitOption) -> Result<T, TxError> {
        let mut message = core::mem::MaybeUninit::uninit();
        tx_checked_call!(_tx_queue_receive(
            self.0,
            message.as_mut_ptr() as *mut core::ffi::c_void,
            wait as ULONG
        ))
        .map(|_| unsafe { message.assume_init() })
    }
}
