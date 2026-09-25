#![no_main]
#![no_std]


use cortex_m_semihosting::debug;

use defmt_rtt as _; // global logger

// TODO(5) adjust HAL import
use stm32f4xx_hal as _; // memory layout

use panic_probe as _;

pub mod threadx;

pub mod network;

pub mod uprotocol_v1;
pub mod utransport;
pub mod minimqtransport;

// same panicking *behavior* as `panic-probe` but doesn't print a panic message
// this prevents the panic message being printed *twice* when `defmt::panic` is invoked
#[defmt::panic_handler]
fn panic() -> ! {
    cortex_m::asm::udf()
}

/// Terminates the application and makes a semihosting-capable debug tool exit
/// with status code 0.
pub fn exit() -> ! {
    loop {
        debug::exit(debug::EXIT_SUCCESS);
    }
}

/// Hardfault handler.
///
/// Terminates the application and makes a semihosting-capable debug tool exit
/// with an error. This seems better than the default, which is to spin in a
/// loop.
///

#[cortex_m_rt::exception]
unsafe fn HardFault(frame: &cortex_m_rt::ExceptionFrame) -> ! {
    
    defmt::error!("Exception PC {}, r0 {}", frame.pc(), frame.r0());
    loop {
        debug::exit(debug::EXIT_FAILURE);
    }
}


#[repr(u32)]
#[derive(Debug, defmt::Format)]
pub enum NxError {
    PoolError = netx_sys::NX_POOL_ERROR,
    SocketClosed = netx_sys::NX_NOT_CONNECTED,
    AlreadyInitialized = 0xFE,
    Unknown = 0xFF,
}

impl NxError {
    pub const fn from_u32(val: u32) -> Self {
        match val {
            netx_sys::NX_POOL_ERROR => Self::PoolError,
            netx_sys::NX_NOT_CONNECTED => Self::SocketClosed,
            _ => Self::Unknown,
        }
    } 
}

// defmt-test 0.3.0 has the limitation that this `#[tests]` attribute can only be used
// once within a crate. the module can be in any file but there can only be at most
// one `#[tests]` module in this library crate
#[cfg(test)]
#[defmt_test::tests]
mod unit_tests {
    use defmt::assert;

    #[test]
    fn it_works() {
        assert!(true)
    }
}

