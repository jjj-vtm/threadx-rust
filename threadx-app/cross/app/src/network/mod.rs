pub mod network;

#[repr(u32)]
#[derive(Debug)]
pub enum NxError {
    PoolError = netx_sys::NX_POOL_ERROR,
    Unknown = 0xFF,
}

impl NxError {
    pub fn from_u32(val: u32) -> NxError {
        match val {
            netx_sys::NX_POOL_ERROR => Self::PoolError,
            _ => Self::Unknown,
        }
    } 
}

#[macro_export]
macro_rules! nx_checked_call {
    ($func:ident($($arg:expr),*)) => {
        {
            use defmt::error;
            use defmt::trace;
            let ret = unsafe { $func($($arg),*) };
            if ret != netx_sys::NX_SUCCESS {
                error!("NetXDuo call {} returned {}", stringify!($func), ret);
                Err(NxError::from_u32(ret))
            } else {
                trace!("NetXDuo call {} Success", stringify!($func));
                Ok(())
            }
        }
    }
}