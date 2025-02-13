use core::cell::UnsafeCell;
use core::ffi::CStr;
use core::mem::MaybeUninit;
use core::ops::Deref;
use core::ops::DerefMut;

/*
UINT        _tx_mutex_create(TX_MUTEX *mutex_ptr, CHAR *name_ptr, UINT inherit);
UINT        _tx_mutex_delete(TX_MUTEX *mutex_ptr);
UINT        _tx_mutex_get(TX_MUTEX *mutex_ptr, ULONG wait_option);
UINT        _tx_mutex_info_get(TX_MUTEX *mutex_ptr, CHAR **name, ULONG *count, TX_THREAD **owner,
                    TX_THREAD **first_suspended, ULONG *suspended_count,
                    TX_MUTEX **next_mutex);
UINT        _tx_mutex_performance_info_get(TX_MUTEX *mutex_ptr, ULONG *puts, ULONG *gets,
                    ULONG *suspensions, ULONG *timeouts, ULONG *inversions, ULONG *inheritances);
UINT        _tx_mutex_performance_system_info_get(ULONG *puts, ULONG *gets, ULONG *suspensions, ULONG *timeouts,
                    ULONG *inversions, ULONG *inheritances);
UINT        _tx_mutex_prioritize(TX_MUTEX *mutex_ptr);
UINT        _tx_mutex_put(TX_MUTEX *mutex_ptr);

*/
use crate::tx_checked_call;

use super::error::TxError;
use super::WaitOption;
use defmt::error;
use num_traits::FromPrimitive;
use thiserror_no_std::Error;
use threadx_sys::_tx_mutex_create;
use threadx_sys::_tx_mutex_delete;
use threadx_sys::_tx_mutex_get;
use threadx_sys::_tx_mutex_put;
use threadx_sys::TX_MUTEX;

/// Safety: TODO: Is it safe to copy the TX_MUTEX structure? Currently nothing forbids rust to just move it ie. memcopy it. 
/// Maybe initialization of the mutex should return a handle containing the pointer. This should be safe and the can live as long as the mutex. 
/// But that maybe not enough since the original mutex struct can still be moved around...
/// ThreadX documentation:
/// Mutex Control Block TX_MUTEX
/// The characteristics of each mutex are found in its control block. It contains information such as the current mutex ownership count along with the pointer 
/// of the thread that owns the mutex. This structure is defined in the tx_api.h file. Mutex control blocks can be located anywhere in memory, but 
/// it is most common to make the control block a global structure by defining it outside the scope of any function.
pub struct Mutex<T> {
    inner: UnsafeCell<T>,
    // TX_MUTEX control block
    mutex: UnsafeCell<MaybeUninit<TX_MUTEX>>,
    initialized: bool,
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
}

impl<T> Mutex<T> {
    pub const fn new(inner: T) -> Mutex<T> {
        Mutex {
            inner: UnsafeCell::new(inner),
            mutex: UnsafeCell::new(MaybeUninit::<TX_MUTEX>::uninit()),
            initialized: false,
        }
    }
}
impl<T> Mutex<T> {
    pub fn initialize(&mut self, name: &CStr, inherit: bool) -> Result<(), TxError> {
        if self.initialized {
            // If mutex was already initialized we just return Ok
            return Ok(());
        }
        let mutex_ptr = self.mutex.get_mut().as_mut_ptr();
        let res = tx_checked_call!(_tx_mutex_create(
            mutex_ptr,
            name.as_ptr() as *mut i8,
            inherit as u32
        ));
        if res.is_ok() {
            self.initialized = true;
        }
        res
    }
    pub fn lock(&self, wait_option: WaitOption) -> Result<MutexGuard<'_, T>, MutexError> {
        if !self.initialized {
            return Err(MutexError::PoisonError);
        }
        let mutex_ptr = self.mutex.get();

        if let Some(mutex_ptr) = unsafe { mutex_ptr.as_mut() } {
            let mutex_ptr = mutex_ptr.as_mut_ptr();
            let result = tx_checked_call!(_tx_mutex_get(mutex_ptr, wait_option as u32));
            match result {
                Ok(_) => Ok(MutexGuard { mutex: self }),
                Err(e) => Err(MutexError::MutexError(e)),
            }
        } else {
            return Err(MutexError::PoisonError);
        }
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
