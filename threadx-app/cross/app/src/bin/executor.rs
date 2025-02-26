#![no_main]
#![no_std]

use core::cell::RefCell;
use core::ffi::CStr;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use core::time::Duration;

use alloc::boxed::Box;
use board::{BoardMxAz3166, LowLevelInit};

use cortex_m::interrupt::{self, Mutex};
use cortex_m::itm::Aligned;
use defmt::println;
use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Baseline, Text},
};
use static_cell::StaticCell;
use stm32f4xx_hal::i2c::I2c;
use stm32f4xx_hal::pac::I2C1;
use threadx_rs::allocator::ThreadXAllocator;
use threadx_rs::event_flags::EventFlagsGroup;
use threadx_rs::executor::block_on;
use threadx_rs::pool::{self, BytePool};

use threadx_rs::thread::{sleep, Thread};

extern crate alloc;

#[global_allocator]
static GLOBAL: ThreadXAllocator = ThreadXAllocator::new();

static BP: StaticCell<BytePool> = StaticCell::new();

static THREAD1: StaticCell<Thread> = StaticCell::new();
static THREAD2: StaticCell<Thread> = StaticCell::new();

static BP_MEM: StaticCell<[u8; 2048]> = StaticCell::new();
static HEAP: StaticCell<[u8; 1024]> = StaticCell::new();
static EXECUTOR_EVENT: StaticCell<EventFlagsGroup> = StaticCell::new();

static BOARD: Mutex<RefCell<Option<BoardMxAz3166<I2c<I2C1>>>>> = Mutex::new(RefCell::new(None));

#[cortex_m_rt::entry]
fn main() -> ! {
    let tx = threadx_rs::Builder::new(
        // low level initialization
        |ticks_per_second| {
            let board = BoardMxAz3166::low_level_init(ticks_per_second).unwrap();
            interrupt::free(|cs| BOARD.borrow(cs).borrow_mut().replace(board));
        },
        // Start of Application definition
        |mem_start| {
            defmt::println!("Define application. Memory starts at: {} ", mem_start);

            let bp = BP.init(BytePool::new());

            // Inefficient, creates array on the stack first.
            let bp_mem = BP_MEM.init_with(|| [0u8; 2048]);
            let bp = bp
                .initialize(CStr::from_bytes_until_nul(b"pool1\0").unwrap(), bp_mem)
                .unwrap();

            //allocate memory for the two tasks.
            let task2_mem = bp.allocate(1024, true).unwrap();

            let heap: Aligned<[u8; 1024]> = Aligned([0; 1024]);
            let heap_mem = HEAP.init_with(|| heap.0);
            GLOBAL.initialize(heap_mem).unwrap();

            let evt = EXECUTOR_EVENT.init(EventFlagsGroup::new());
            let event_handle = evt
                .initialize(CStr::from_bytes_with_nul(b"ExecutorGroup\0").unwrap())
                .unwrap();

            let thread2_fn = Box::new(move || loop {
                let text_style = MonoTextStyleBuilder::new()
                    .font(&FONT_6X10)
                    .text_color(BinaryColor::On)
                    .build();
                //block_on(NeverFinished {}, event_handle);
                block_on(test_async(), event_handle);
                println!("Short after...");
                interrupt::free(|cs| {
                    let mut tmp = BOARD.borrow(cs).borrow_mut();
                    let board = tmp.as_mut().unwrap();
                    let display = &mut board.display;
                    Text::with_baseline("Test", Point::zero(), text_style, Baseline::Top)
                        .draw(display)
                        .unwrap();

                    display.flush().unwrap();
                });
                let _ = sleep(Duration::from_secs(5));
            });

            let thread2 = THREAD2.init(Thread::new());

            let _ = thread2
                .initialize_with_autostart_box("thread2", thread2_fn, task2_mem.consume(), 1, 1, 0)
                .unwrap();

            defmt::println!("Done with app init.");
        },
    );

    tx.initialize();
    println!("Exit");
    threadx_app::exit()
}

async fn test_async() {
    println!("Hello from async runtime");
}
struct NeverFinished {}

impl Future for NeverFinished {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let w1 = cx.waker().clone();
        Poll::Pending
    }
}
