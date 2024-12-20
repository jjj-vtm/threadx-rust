#![no_std]
use core::arch::asm;
use core::ffi::c_void;

use cortex_m::peripheral::syst::SystClkSource;
use stm32f4xx_hal::pac;
use stm32f4xx_hal::rcc::RccExt;
use stm32f4xx_hal::time::MegaHertz;

use stm32f4xx_hal::prelude::*;
/// Low level initialization. The low level initialization function will
/// perform basic low level initialization of the hardware.
pub trait LowLevelInit {
    /// The input is the number of ticks per second that ThreadX will be
    /// expecting. The output is a pointer to a slice that is used as the
    /// heap memory. This function is also exptected to set
    /// the system stack pointer. The variable is exposed by threadx_sys
    /// as threadx_sys::_tx_thread_system_stack_ptr
    fn low_level_init(ticks_per_second: u32) -> Result<(), ()>;
}

// cortexm-rt crate defines the _stack_start function. Due to the action of flip-link, the stack pointer
// is moved lower down in memory after leaving space for the bss and data sections.
extern "C" {
    static _stack_start: u32;
}

pub struct BoardMxAz3166;

impl LowLevelInit for BoardMxAz3166 {
    fn low_level_init(ticks_per_second: u32) -> Result<(), ()> {
        unsafe {
            let stack_start = &_stack_start as *const u32 as u32;
            threadx_sys::_tx_thread_system_stack_ptr = stack_start as *mut c_void;
            defmt::println!(
                "Low level init.  Stack at: {=u32:#x} Ticks per second:{}",
                stack_start,
                ticks_per_second
            );

            defmt::println!("Stack size {}", stack_start - 0x2000_0000);
        }
        let p = pac::Peripherals::take().unwrap();
        
        let rcc = p.RCC.constrain();
        // Setup clocks. Reference (https://github.com/Eclipse-SDV-Hackathon-Chapter-Two/challenge-threadx-and-beyond/tree/main)
        let _clocks = rcc
            .cfgr
            .sysclk(96.MHz())
            .hclk(96.MHz())
            .pclk1(36.MHz())
            .pclk2(64.MHz())
            .use_hse(26.MHz())
            .freeze();

        let cp = cortex_m::Peripherals::take().unwrap();
        
        let mut syst = cp.SYST;
        let mut dcb = cp.DCB;
        dcb.enable_trace();
        let mut dbg = cp.DWT;
        // configures the system timer to trigger a SysTick exception every second
        dbg.enable_cycle_counter();

        syst.set_clock_source(SystClkSource::Core);
        syst.set_reload((96_000_000 / ticks_per_second) - 1);
        syst.enable_counter();
        syst.enable_interrupt();


        defmt::println!("Low level init");

        //Set up the priorities for SysTick and PendSV and SVC
        unsafe {
            asm!(
                "MOV     r0, #0xE000E000",
                "LDR     r1, =0x00000000",
                "STR     r1, [r0, #0xD18]",
                "LDR     r1, =0xFF000000",
                "STR     r1, [r0, #0xD1C]",
                "LDR     r1, =0x40FF0000",
                "STR     r1, [r0, #0xD20]",
            );
        }
        defmt::println!("Int prio set");
        Ok(())
    }
}
