#![no_main]
#![no_std]

use core::cell::RefCell;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use core::time::Duration;

use alloc::boxed::Box;
use board::{BoardMxAz3166, I2CBus, LowLevelInit};

use cortex_m::interrupt::{self, Mutex};
use cortex_m::itm::Aligned;
use defmt::println;
use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
};
use prost::Message;
use static_cell::StaticCell;
use threadx_app::uprotocol_v1::{UAttributes, UMessage};
use threadx_rs::allocator::ThreadXAllocator;
use threadx_rs::event_flags::EventFlagsGroup;
use threadx_rs::executor::{self, Executor};
use threadx_rs::pool::BytePool;

use threadx_rs::thread::{sleep, Thread};

extern crate alloc;

#[global_allocator]
static GLOBAL: ThreadXAllocator = ThreadXAllocator::new();

static BP: StaticCell<BytePool> = StaticCell::new();

static THREAD2: StaticCell<Thread> = StaticCell::new();

static BP_MEM: StaticCell<[u8; 2048]> = StaticCell::new();
static HEAP: StaticCell<[u8; 1024]> = StaticCell::new();

static BOARD: Mutex<RefCell<Option<BoardMxAz3166<I2CBus>>>> = Mutex::new(RefCell::new(None));

#[cortex_m_rt::entry]
fn main() -> ! {
    let tx = threadx_rs::Builder::new(
        // low level initialization
        |ticks_per_second| {
            let board = BoardMxAz3166::low_level_init(ticks_per_second).unwrap();
            // ThreadX mutexes cannot be used here.
            interrupt::free(|cs| BOARD.borrow(cs).borrow_mut().replace(board));
        },
        // Start of Application definition
        |mem_start| {
            defmt::println!("Define application. Memory starts at: {} ", mem_start);

            let bp = BP.init(BytePool::new());

            // Inefficient, creates array on the stack first.
            let bp_mem = BP_MEM.init_with(|| [0u8; 2048]);
            let bp = bp.initialize(c"pool1", bp_mem).unwrap();

            //allocate memory for the two tasks.
            let task2_mem = bp.allocate(1024, true).unwrap();

            let heap: Aligned<[u8; 1024]> = Aligned([0; 1024]);
            let heap_mem = HEAP.init_with(|| heap.0);
            GLOBAL.initialize(heap_mem).unwrap();
            let executor = Executor::new();

            let thread2_fn = Box::new(move || {

                // Get the display out out the board structure
                let text_style = MonoTextStyleBuilder::new()
                    .font(&FONT_6X10)
                    .text_color(BinaryColor::On)
                    .build();
               // executor.block_on(NeverFinished {});
                let ua = UAttributes::default();
                let mut u_m = UMessage::default();

                u_m.attributes.replace(ua);
                let mut buf = [0u8; 24];
                let _ = UMessage::encode(&u_m, &mut buf.as_mut_slice()).unwrap();

                loop {
                    executor.block_on(test_async());
                    interrupt::free(|cs| {
                        let mut binding = BOARD.borrow(cs).borrow_mut();
                        let board = binding.as_mut().unwrap();
                        let hts221 = board.temp_sensor.as_mut().unwrap();
                        let deg = hts221.temperature_x8(&mut board.i2c_bus.unwrap()).unwrap()
                            as f32
                            / 8.0;
                        // println!("Current temperature: {}", deg);
                    });
                    let _ = sleep(Duration::from_secs(5));
                }
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
        let _w1 = cx.waker().clone();
        Poll::Pending
    }
}
