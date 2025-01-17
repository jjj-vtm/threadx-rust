#![no_main]
#![no_std]

use core::ffi::CStr;
use core::sync::atomic::{AtomicBool, AtomicU32};

use alloc::boxed::Box;
use board::{BoardMxAz3166, LowLevelInit};

use defmt::{debug, println};
use static_cell::StaticCell;
use threadx_rs::allocator::ThreadXAllocator;
use threadx_rs::event_flags::EventFlagsGroup;
use threadx_rs::pool::BytePool;
use threadx_rs::timer::Timer;
use threadx_rs::{tx_checked_call, WaitOption};

use threadx_rs::thread::Thread;
use threadx_rs::tx_str;

extern crate alloc;

#[global_allocator]
static GLOBAL: ThreadXAllocator = ThreadXAllocator::new();

#[cortex_m_rt::entry]
fn main() -> ! {
    defmt::println!("Hello, world!");

    let tx = threadx_rs::Builder::new(
        // low level initialization
        |ticks_per_second| {
            BoardMxAz3166::low_level_init(ticks_per_second).unwrap();
            static mut HEAP: [u8; 4096 * 3] = [0u8; 4096 * 3];
            unsafe { HEAP.as_mut_slice() }
        },
        // Start of Application definition
        |mem_start| {
            defmt::println!(
                "Define application. Memory starts at: {} with length:{}",
                mem_start.as_ptr(),
                mem_start.len()
            );

            static BP: StaticCell<BytePool> = StaticCell::new();
            let bp = BP.init(BytePool::new());

            let (bp_mem, next) = mem_start.split_at_mut(1024);

            let bp = bp
                .initialize(CStr::from_bytes_until_nul(b"pool1\0").unwrap(), bp_mem)
                .unwrap();

            //allocate memory for the two tasks.
            let task1_mem = bp.allocate(256, true).unwrap();
            let task2_mem = bp.allocate(256, true).unwrap();
            let task3_mem = bp.allocate(256, true).unwrap();

            let (global_alloc_mem, next) = next.split_at_mut(1024);
            //let  heap_bytepool : BytePoolHandle = unsafe{BP1.initialize(tx_str!("pool2"), bp1_mem).unwrap()};

            // Only after this call the global allocator is working, using Heap alloocations before should panic.
            GLOBAL.initialize(global_alloc_mem).unwrap();

            // create events flag group
            static EVENT_GROUP: StaticCell<EventFlagsGroup> = StaticCell::new();
            let event_group = EVENT_GROUP.init(EventFlagsGroup::new());

            let evt_handle = event_group
                .initialize(CStr::from_bytes_until_nul(b"event_flag\0").unwrap())
                .unwrap();

            // Create timer
            static TIMER: StaticCell<Timer> = StaticCell::new();
            let timer = TIMER.init(Timer::new());

            let timer_fn = Box::new(move || {
                evt_handle.publish(1).unwrap();
            });

            timer
                .initialize_with_closure(
                    CStr::from_bytes_until_nul(b"timer\0").unwrap(),
                    timer_fn,
                    core::time::Duration::from_secs(5), // initial timeout is 5 seconds
                    core::time::Duration::from_secs(1), // periodic timeout is 1 second
                    true,                               // start the timer immediately
                )
                .expect("Timer Init failed");

            static THREAD1: StaticCell<Thread> = StaticCell::new();
            let thread1 = THREAD1.init(Thread::new());

            let thread_func = Box::new(move || loop {
                let event = evt_handle
                    .get(
                        1,
                        threadx_rs::event_flags::GetOption::WaitAllAndClear,
                        WaitOption::WaitForever,
                    )
                    .expect("Thread1 failed");
                println!("Thread1: Got Event 1 : {}", event);

                threadx_rs::thread::sleep(core::time::Duration::from_millis(100)).unwrap();
            });

            let _ = thread1
                .initialize_with_autostart_box("thread1", thread_func, task1_mem.consume(), 1, 1, 0)
                .unwrap();

            let thread2_fn = Box::new(move || {
                let arg: u32 = 1;
                println!("Thread:{}", arg);

                loop {
                    let event = evt_handle
                        .get(
                            1,
                            threadx_rs::event_flags::GetOption::WaitAllAndClear,
                            WaitOption::WaitForever,
                        )
                        .expect("Thread2 failed");
                    println!("Thread2: Got Event 1 : {}", event);
                    threadx_rs::thread::sleep(core::time::Duration::from_millis(100)).unwrap();
                }
            });

            static THREAD2: StaticCell<Thread> = StaticCell::new();
            let thread2 = THREAD2.init(Thread::new());

            let _ = thread2
                .initialize_with_autostart_box("thread2", thread2_fn, task2_mem.consume(), 1, 1, 0)
                .unwrap();

            let thread3_fn = Box::new(move || {
                let arg: u32 = 2;
                println!("Thread:{}", arg);

                loop {
                    let event = evt_handle
                        .get(
                            1,
                            threadx_rs::event_flags::GetOption::WaitAllAndClear,
                            WaitOption::WaitForever,
                        )
                        .expect("Thread3 failed");
                    threadx_rs::thread::sleep(core::time::Duration::from_millis(100)).unwrap();

                    println!("Thread3: Got Event 1 : {}", event);
                }
            });

            static THREAD3: StaticCell<Thread> = StaticCell::new();
            let thread3 = THREAD3.init(Thread::new());

            let _ = thread3
                .initialize_with_autostart_box("thread3", thread3_fn, task3_mem.consume(), 1, 1, 0)
                .unwrap();

            defmt::println!("Done with app init.");
        },
    );

    tx.initialize();
    println!("Exit");
    threadx_app::exit()
}
