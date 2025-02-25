#![no_main]
#![no_std]

use core::ffi::CStr;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use core::time::Duration;

use alloc::boxed::Box;
use board::{BoardMxAz3166, LowLevelInit};

use cortex_m::itm::Aligned;
use defmt::println;
use static_cell::StaticCell;
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
            let bp_mem = BP_MEM.init_with(||[0u8; 2048]);
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

            let thread2_fn = Box::new(move ||  loop {
                //block_on(NeverFinished {}, event_handle);
                block_on(test_async(), event_handle);
                println!("Short after...");
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
