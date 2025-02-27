#![no_std]
use core::ffi::c_void;
use core::{arch::asm, cell::RefCell};

use cortex_m::interrupt::{self, Mutex};
use cortex_m::peripheral::syst::SystClkSource;
use ssd1306::prelude::I2CInterface;
use stm32f4xx_hal::time::Hertz;
use stm32f4xx_hal::{
    gpio::GpioExt,
    i2c::{I2c, Mode},
    pac::{self, I2C1},
    rcc::RccExt,
};

pub use hts221;
pub use embedded_hal::i2c;

use ssd1306::{
    mode::DisplayConfig, prelude::DisplayRotation, size::DisplaySize128x64, I2CDisplayInterface,
    Ssd1306,
};
/// Low level initialization. The low level initialization function will
/// perform basic low level initialization of the hardware.
pub trait LowLevelInit {
    /// The input is the number of ticks per second that ThreadX will be
    /// expecting. The output is an initialized Board struct
    fn low_level_init(ticks_per_second: u32) -> Result<BoardMxAz3166<I2CBus>, ()>;
}

// cortexm-rt crate defines the _stack_start function. Due to the action of flip-link, the stack pointer
// is moved lower down in memory after leaving space for the bss and data sections.
extern "C" {
    static _stack_start: u32;
}

type DisplayType<I2C> = Ssd1306<
    ssd1306::prelude::I2CInterface<I2C>,
    DisplaySize128x64,
    ssd1306::mode::BufferedGraphicsMode<DisplaySize128x64>,
>;
type TempSensorType<I2C> = hts221::HTS221<I2C, stm32f4xx_hal::i2c::Error>;

pub struct BoardMxAz3166<I2C>
where
    I2C: embedded_hal::i2c::I2c,
{
    pub display: Option<DisplayType<I2C>>,
    pub temp_sensor: Option<TempSensorType<I2CBus>>,
    pub i2c_bus: Option<I2CBus>,
}

#[derive(Clone, Copy)]
pub struct I2CBus {
    pub i2c: &'static Mutex<RefCell<Option<I2c<I2C1>>>>,
}
impl embedded_hal::i2c::ErrorType for I2CBus
{
    type Error = stm32f4xx_hal::i2c::Error;
}

impl embedded_hal::i2c::I2c for I2CBus
{
    fn transaction(
        &mut self,
        address: u8,
        operations: &mut [embedded_hal::i2c::Operation<'_>],
    ) -> Result<(), Self::Error> {
        interrupt::free(|cs| {
            let mut binding = self.i2c.borrow(cs).borrow_mut();
            let bus = binding.as_mut().unwrap();
            bus.transaction_slice(address, operations)
        })
    }
}

static SHARED_BUS: Mutex<RefCell<Option<I2c<I2C1>>>> = Mutex::new(RefCell::new(None));

impl LowLevelInit for BoardMxAz3166<I2CBus> {
    fn low_level_init(ticks_per_second: u32) -> Result<BoardMxAz3166<I2CBus>, ()> {
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
        let clocks = rcc
            .cfgr
            .sysclk(Hertz::MHz(96))
            .hclk(Hertz::MHz(96))
            .pclk1(Hertz::MHz(36))
            .pclk2(Hertz::MHz(64))
            .use_hse(Hertz::MHz(26))
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

        let gpiob = p.GPIOB.split();
        // Configure I2C1
        let scl = gpiob.pb8;
        let sda = gpiob.pb9;

        let i2c = I2c::new(p.I2C1, (scl, sda), Mode::standard(Hertz::kHz(400)), &clocks);
        interrupt::free(|cs| SHARED_BUS.borrow(cs).replace(Some(i2c)));
        let mut bus = I2CBus {
            i2c: &SHARED_BUS,
        };
        defmt::println!("Low level init");
        
        let hts221 =
            hts221::Builder::new()
                .with_data_rate(hts221::DataRate::Continuous1Hz)
                .build(&mut bus)
                .unwrap();


        let interface: I2CInterface<I2CBus> = I2CDisplayInterface::new(bus);

        let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
            .into_buffered_graphics_mode();
        display.init().unwrap();

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
        Ok(BoardMxAz3166 {
            display: Some(display),
            temp_sensor: Some(hts221),
            i2c_bus: Some(bus),
        })
    }
}
