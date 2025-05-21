#![no_main]
#![no_std]

use core::cell::RefCell;
use core::sync::atomic::{AtomicI16, AtomicU8};
use threadx_rs::select::select;

use core::time::Duration;

use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::string::ToString;
use board::{BoardMxAz3166, I2CBus, LowLevelInit};

use cortex_m::interrupt::Mutex;
use embedded_graphics::Drawable;
use embedded_graphics::mono_font::ascii::FONT_9X18;
use embedded_graphics::prelude::Point;
use embedded_graphics::text::{Baseline, Text};
use embedded_graphics::{mono_font::MonoTextStyleBuilder, pixelcolor::BinaryColor};
use static_cell::StaticCell;
use threadx_rs::allocator::ThreadXAllocator;
use threadx_rs::executor::Executor;

use threadx_rs::thread::{Thread, sleep};

extern crate alloc;

#[global_allocator]
static GLOBAL: ThreadXAllocator = ThreadXAllocator::new();

static DISPLAY_THREAD: StaticCell<Thread> = StaticCell::new();
static DISPLAY_THREAD_STACK: StaticCell<[u8; 2048]> = StaticCell::new();

static SWITCHER_THREAD: StaticCell<Thread> = StaticCell::new();
static SWITCHER_THREAD_STACK: StaticCell<[u8; 512]> = StaticCell::new();

static MEASURE_THREAD: StaticCell<Thread> = StaticCell::new();
static MEASURE_THREAD_STACK: StaticCell<[u8; 512]> = StaticCell::new();

static HEAP: StaticCell<[u8; 1024]> = StaticCell::new();

enum DisplayState {
    Welcome,
    Temperature,
}

impl From<u8> for DisplayState {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Welcome,
            1 => Self::Temperature,
            _ => Self::Welcome,
        }
    }
}
static DISPLAY_STATE: AtomicU8 = AtomicU8::new(DisplayState::Welcome as u8);
static TEMP_MEASURE: AtomicI16 = AtomicI16::new(0);

static BOARD: Mutex<RefCell<Option<BoardMxAz3166<I2CBus>>>> = Mutex::new(RefCell::new(None));
#[cortex_m_rt::entry]
fn main() -> ! {
    let tx = threadx_rs::Builder::new(
        // low level initialization
        |ticks_per_second| {
            let board = BoardMxAz3166::low_level_init(ticks_per_second);
            // ThreadX mutexes cannot be used here.
            cortex_m::interrupt::free(|cs| BOARD.borrow(cs).borrow_mut().replace(board));
        },
        // Start of Application definition
        |mem_start| {
            defmt::info!("Define application. Memory starts at: {} ", mem_start);
            // Inefficient, creates array on the stack first.
            let display_thread_stack = DISPLAY_THREAD_STACK.init_with(|| [0u8; 2048]);
            let measure_thread_stack = MEASURE_THREAD_STACK.init_with(|| [0u8; 512]);
            let switcher_thread_stack = SWITCHER_THREAD_STACK.init_with(|| [0u8; 512]);

            let heap_mem = HEAP.init_with(|| [0u8; 1024]);
            GLOBAL.initialize(heap_mem).unwrap();
            let executor = Executor::new();

            let measure_task = Box::new(move || {
                let (mut hts221, mut i2c) = extract_temperature_peripherals();

                loop {
                    TEMP_MEASURE.store(
                        hts221.temperature_x8(&mut i2c).unwrap(),
                        core::sync::atomic::Ordering::Relaxed,
                    );
                    let _ = sleep(Duration::from_secs(1));
                }
            });

            let switcher_task = Box::new(move || {
                let btn_b = cortex_m::interrupt::free(|cs| {
                    let mut board = BOARD.borrow(cs).borrow_mut();
                    board.as_mut().unwrap().btn_b.take().unwrap()
                });

                let btn_a = cortex_m::interrupt::free(|cs| {
                    let mut board = BOARD.borrow(cs).borrow_mut();
                    board.as_mut().unwrap().btn_a.take().unwrap()
                });
                loop {
                    executor.block_on(async {
                        // Await full button press for either button
                        let button_pressed = select(
                            btn_a.wait_for_button_pressed(),
                            btn_b.wait_for_button_pressed(),
                        );
                        let button_pressed = match button_pressed.await {
                            threadx_rs::either::Either::Left(_) => board::BUTTONS::ButtonA,
                            threadx_rs::either::Either::Right(_) => board::BUTTONS::ButtonB,
                        };

                        match button_pressed {
                            board::BUTTONS::ButtonA => btn_a.wait_for_button_released().await,
                            board::BUTTONS::ButtonB => btn_b.wait_for_button_released().await,
                        }

                        let state = DisplayState::from(
                            DISPLAY_STATE.load(core::sync::atomic::Ordering::Relaxed),
                        );
                        let new_state = match state {
                            DisplayState::Welcome => DisplayState::Temperature,
                            DisplayState::Temperature => DisplayState::Welcome,
                        };
                        DISPLAY_STATE.store(new_state as u8, core::sync::atomic::Ordering::Relaxed);
                    });
                }
            });

            let display_task = Box::new(move || {
                // Get the peripherals
                let mut display = cortex_m::interrupt::free(|cs| {
                    let mut board = BOARD.borrow(cs).borrow_mut();
                    board.as_mut().unwrap().display.take().unwrap()
                });
                loop {
                    let text_style = MonoTextStyleBuilder::new()
                        .font(&FONT_9X18)
                        .text_color(BinaryColor::On)
                        .build();

                    display.clear_buffer();

                    let state = DisplayState::from(
                        DISPLAY_STATE.load(core::sync::atomic::Ordering::Relaxed),
                    );

                    match state {
                        DisplayState::Welcome => {
                            Text::with_baseline(
                                "Welcome!",
                                Point::zero(),
                                text_style,
                                Baseline::Top,
                            )
                            .draw(&mut display)
                            .unwrap();
                        }
                        DisplayState::Temperature => {
                            let mut text = "temperature: \n".to_owned();
                            let temp = (f32::from(
                                TEMP_MEASURE.load(core::sync::atomic::Ordering::Relaxed),
                            ) / 8.0)
                                .to_string();
                            text.push_str(&temp);
                            text.push('C');

                            Text::with_baseline(&text, Point::zero(), text_style, Baseline::Top)
                                .draw(&mut display)
                                .unwrap();
                        }
                    }

                    display.flush().unwrap();
                    let _ = sleep(Duration::from_millis(200));
                }
            });

            let display_thread = DISPLAY_THREAD.init(Thread::new());
            let measure_thread = MEASURE_THREAD.init(Thread::new());
            let switcher_thread = SWITCHER_THREAD.init(Thread::new());

            let _ = display_thread
                .initialize_with_autostart_box(
                    c"display_thread",
                    display_task,
                    display_thread_stack,
                    1,
                    1,
                    0,
                )
                .unwrap();

            let _ = measure_thread
                .initialize_with_autostart_box(
                    c"measure_thread",
                    measure_task,
                    measure_thread_stack,
                    1,
                    1,
                    0,
                )
                .unwrap();

            let _ = switcher_thread
                .initialize_with_autostart_box(
                    c"switcher_thread",
                    switcher_task,
                    switcher_thread_stack,
                    1,
                    1,
                    0,
                )
                .unwrap();

            defmt::info!("Done with app init.");
        },
    );

    tx.initialize();
    defmt::info!("Exit");
    threadx_app::exit()
}

fn extract_temperature_peripherals() -> (
    board::hts221::HTS221<I2CBus, stm32f4xx_hal::i2c::Error>,
    I2CBus,
) {
    cortex_m::interrupt::free(|cs| {
        let mut binding = BOARD.borrow(cs).borrow_mut();
        let board = binding.as_mut().unwrap();
        let hts221 = board.temp_sensor.take().unwrap();
        let i2c = board.i2c_bus.take().unwrap();
        (hts221, i2c)
    })
}
