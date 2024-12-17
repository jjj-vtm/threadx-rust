#![no_main]
#![no_std]

use core::net::{Ipv4Addr, SocketAddr};

use board::{BoardMxAz3166, LowLevelInit};

use defmt::println;
use embedded_nal::TcpClientStack;
use static_cell::StaticCell;
use threadx_app::network::network::ThreadxTcpNetwork;

use threadx_rs::thread;

use threadx_rs::thread::Thread;

pub type UINT = ::core::ffi::c_uint;

#[cortex_m_rt::entry]
fn main() -> ! {
    let tx = threadx_rs::Builder::new(
        |ticks_per_second| {
            BoardMxAz3166::low_level_init(ticks_per_second).unwrap();
            static mut HEAP: [u8; 4096 * 1] = [0u8; 4096 * 1];
            unsafe { HEAP.as_mut_slice() }
        },
        // Start of Application definition
        //mem_start == Heap, use it with care (_tx_initialize_unused_memory)
        |user_heap_mem_start| {
            defmt::println!(
                "Define application. User Heap Memory starts at: {} with length:{}",
                user_heap_mem_start.as_ptr(),
                user_heap_mem_start.len()
            );

            // Static Cell since we need an allocated but uninitialized block of memory
            static WIFI_THREAD_STACK: StaticCell<[u8; 4096]> = StaticCell::new();
            let wifi_thread_stack: *mut [u8; 4096] = WIFI_THREAD_STACK.uninit().as_mut_ptr();

            static WIFI_THREAD: StaticCell<Thread<thread::UnInitialized>> = StaticCell::new();
            let wifi_thread: &mut Thread<thread::UnInitialized> =
                WIFI_THREAD.init(Thread::<thread::UnInitialized>::new());

            let _ =
                wifi_thread.initialize("wifi_thread", do_network, wifi_thread_stack, 4, 4, 0, true);
        },
    );

    tx.initialize();
    println!("Exit");
    threadx_app::exit()
}

pub fn do_network() {
    let mut network = ThreadxTcpNetwork::initialize("SSID", "PW").unwrap();
    let mut socket = network.socket().unwrap();
    let remote_addr = SocketAddr::new(core::net::IpAddr::V4(Ipv4Addr::new(192, 168, 2, 105)), 1883);
    
    network.connect(&mut socket, remote_addr).unwrap();
    defmt::println!("Network initialized");

    let buffer: [u8; 4] = [48, 49, 50, 51];
    let _ = network.send(&mut socket, &buffer).unwrap();
    defmt::println!("Data send");
}
