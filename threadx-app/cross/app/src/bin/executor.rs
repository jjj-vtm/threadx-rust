#![no_main]
#![no_std]

use core::ffi::CStr;
use core::time::Duration;

use alloc::boxed::Box;
use board::{BoardMxAz3166, LowLevelInit};

use defmt::println;
use static_cell::StaticCell;
use threadx_rs::allocator::ThreadXAllocator;
use threadx_rs::executor::new_executor_and_spawner;
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

#[cortex_m_rt::entry]
fn main() -> ! {
    let tx = threadx_rs::Builder::new(
        // low level initialization
        |ticks_per_second| {
            BoardMxAz3166::low_level_init(ticks_per_second).unwrap();
        },
        // Start of Application definition
        |mem_start| {
            defmt::println!("Define application. Memory starts at: {} ", mem_start);

            let bp = BP.init(BytePool::new());

            // Inefficient, creates array on the stack first.
            let bp_mem = BP_MEM.init([0u8; 2048]);
            let bp = bp
                .initialize(CStr::from_bytes_until_nul(b"pool1\0").unwrap(), bp_mem)
                .unwrap();

            //allocate memory for the two tasks.
            let task1_mem = bp.allocate(512, true).unwrap();
            let task2_mem = bp.allocate(512, true).unwrap();

            let heap = HEAP.init([0u8; 1024]);
            GLOBAL.initialize(heap).unwrap();

            let (executor, spawner) = new_executor_and_spawner();
            let executor_thread = THREAD1.init(Thread::new());

            let thread_func = Box::new(move || loop {
                executor.run();
            });

            let _ = executor_thread
                .initialize_with_autostart_box(
                    "executor_thread",
                    thread_func,
                    task1_mem.consume(),
                    1,
                    1,
                    0,
                )
                .unwrap();

            let thread2_fn = Box::new(move ||  { loop {
                spawner.spawn(async {
                    println!("Hello from the async runtime");
                });
                sleep(Duration::from_secs(1));}
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
