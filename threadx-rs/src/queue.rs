use super::{error::TxError, WaitOption};
use crate::{tx_checked_call, tx_checked_call_no_log};
use core::mem::size_of;
use core::{ffi::CStr, mem::MaybeUninit};
use num_traits::FromPrimitive;
use threadx_sys::{_tx_queue_create, _tx_queue_receive, _tx_queue_send, TX_QUEUE, ULONG};

/// Wrapper around the ThreadX queue. ThreadX will copy the message so the best approximation is to restrict the type to be Copy.
/// Since messages might be received by a different thread any reference must be valid for 'static. Note that the message struct will be dropped
/// at the end of this function.
pub struct Queue<T: Copy + 'static>(MaybeUninit<TX_QUEUE>, core::marker::PhantomData<T>);

impl<T: core::marker::Copy + 'static> Queue<T> {
    // according to the threadx docs, the supported messages sizes are 1 to 16 32 bit words
    pub const fn new() -> Self {
        // ThreadX queue messages are 1 to 16 ULONGs
        const {
            assert!(size_of::<T>() >= size_of::<u32>() && size_of::<T>() <= (size_of::<u32>() * 16))
        };
        Queue(core::mem::MaybeUninit::uninit(), core::marker::PhantomData)
    }
    //TODO: Queue must not necessary live for 'static but can live as long as the memory block does
    //      The static mut borrow pins the queue control structure which is necessary due to the intrusive linked list the struct is part of.
    pub fn initialize(
        &'static mut self,
        name: &CStr,
        queue_memory: &'static mut [u8],
    ) -> Result<(QueueSender<T>, QueueReceiver<T>), TxError> {
        let queue_ptr = self.0.as_mut_ptr();
        tx_checked_call!(_tx_queue_create(
            queue_ptr,
            name.as_ptr().cast_mut(),
            size_of::<T>() as ULONG,
            queue_memory.as_mut_ptr().cast(),
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
/// Safety: QueueSender is Sync and Send since the internal pointer is not exposed and the calls to send/sync
/// can be done from any Thread as per ThreadX documentation.
unsafe impl<T> Send for QueueSender<T> {}
unsafe impl<T> Sync for QueueSender<T> {}

/// Safety: QueueReceiver is Sync and Send since the internal pointer is not exposed and the calls to send/sync
/// can be done from any Thread as per ThreadX documentation.
pub struct QueueReceiver<T>(*mut TX_QUEUE, core::marker::PhantomData<T>);
unsafe impl<T> Send for QueueReceiver<T> {}
unsafe impl<T> Sync for QueueReceiver<T> {}

impl<T> QueueSender<T> {
    pub fn send(&self, message: T, wait: WaitOption) -> Result<(), TxError> {
        tx_checked_call!(_tx_queue_send(
            self.0,
            (&raw const message).cast_mut().cast(),
            wait as ULONG
        ))
    }
}

impl<T> QueueReceiver<T> {
    pub fn receive(&self, wait: WaitOption) -> Result<T, TxError> {
        let mut message = core::mem::MaybeUninit::<T>::uninit();
        tx_checked_call_no_log!(_tx_queue_receive(
            self.0,
            message.as_mut_ptr().cast(),
            wait as ULONG
        ))
        .map(|_| unsafe {
            //Safety: Message was initialized by ThreadX since the call returned successful.
            message.assume_init()
        })
    }
}
