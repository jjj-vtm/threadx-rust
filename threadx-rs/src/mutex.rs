use super::WaitOption;
use super::error::TxError;
use crate::tx_checked_call;
use core::cell::UnsafeCell;
use core::ffi::CStr;
use core::marker::PhantomPinned;
use core::mem::MaybeUninit;
use core::ops::Deref;
use core::ops::DerefMut;
use core::pin::Pin;
use defmt::error;
use num_traits::FromPrimitive;
use thiserror_no_std::Error;
use threadx_sys::_tx_mutex_create;
use threadx_sys::_tx_mutex_delete;
use threadx_sys::_tx_mutex_get;
use threadx_sys::_tx_mutex_put;
use threadx_sys::TX_MUTEX;

pub struct Mutex<T> {
    inner: UnsafeCell<T>,
    mutex: UnsafeCell<MaybeUninit<TX_MUTEX>>,
    initialized: bool,
    _phantom: PhantomPinned,
}

/// Safety: Initialization is done via a &mut reference hence thread safe
unsafe impl<T: Send> Send for Mutex<T> {}
unsafe impl<T: Send> Sync for Mutex<T> {}
pub struct MutexGuard<'a, T> {
    mutex: &'a Mutex<T>,
}

impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.mutex.inner.get() }
    }
}

impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.mutex.inner.get() }
    }
}

impl<T> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        let mutex_ptr = self.mutex.mutex.get();
        if let Some(mutex_ptr) = unsafe { mutex_ptr.as_mut() } {
            if tx_checked_call!(_tx_mutex_put(mutex_ptr.as_mut_ptr())).is_err() {
                error!("MutexGuard::drop failed to put mutex");
            }
        } else {
            panic!("Mutex ptr is null");
        }
    }
}
#[derive(Error, Debug)]
pub enum MutexError {
    MutexError(TxError),
    PoisonError,
    Uninitialized,
}

impl From<TxError> for MutexError {
    fn from(value: TxError) -> Self {
       MutexError::MutexError(value) 
    }
}

impl<T> Mutex<T> {
    pub const fn new(inner: T) -> Mutex<T> {
        Mutex {
            inner: UnsafeCell::new(inner),
            mutex: UnsafeCell::new(MaybeUninit::<TX_MUTEX>::uninit()),
            initialized: false,
            _phantom: PhantomPinned,
        }
    }
}
impl<T> Mutex<T> {
    // Initialize a pinned mutex. Pin is necessary since the mutex control block is not allowed to move after initialisation.
    pub fn initialize(mut self: Pin<&mut Self>, name: &CStr, inherit: bool) -> Result<(), TxError> {
        if self.initialized {
            // If mutex was already initialized we just return Ok
            return Ok(());
        }
        let mutex_ptr = unsafe {
            self.as_mut()
                .get_unchecked_mut()
                .mutex
                .get_mut()
                .as_mut_ptr()
        };
        tx_checked_call!(_tx_mutex_create(
            mutex_ptr,
            name.as_ptr() as *mut u8,
            inherit as u32
        ))?;
        // Safety: MutexGuard will only dereference to a T. The structure which must not move (TX_MUTEX) will not be moved.
        unsafe { self.as_mut().get_unchecked_mut().initialized = true };
        Ok(())
    }

    // Safety: Since we use only immutable references we do not need to use pin
    pub fn lock(&self, wait_option: WaitOption) -> Result<MutexGuard<'_, T>, MutexError> {
        if !self.initialized {
            return Err(MutexError::Uninitialized);
        }
        // Safety: Since the structure is initialized we know that mutex_ptr is valid.
        let mutex_ptr = unsafe {
            self.mutex
                .get()
                .as_mut()
                .expect("Invalid mutex pointer")
                .assume_init_mut()
        };
        tx_checked_call!(_tx_mutex_get(mutex_ptr, wait_option as u32))?;
        Ok(MutexGuard { mutex: self })
    }
}
impl<T> Drop for Mutex<T> {
    fn drop(&mut self) {
        if !self.initialized {
            // Nothing to drop, we rely on rusts recursive drop
            return;
        }
        let mutex_ptr = self.mutex.get_mut().as_mut_ptr();
        let _ = tx_checked_call!(_tx_mutex_delete(mutex_ptr));
    }
}
