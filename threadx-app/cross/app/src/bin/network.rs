#![no_main]
#![no_std]

use core::ffi::CStr;
use core::net::{Ipv4Addr, SocketAddr};
use core::sync::atomic::AtomicU32;
use core::time::Duration;

use alloc::boxed::Box;
use board::{BoardMxAz3166, LowLevelInit};

use defmt::println;
use minimq::broker::IpBroker;
use minimq::embedded_time::rate::Fraction;
use minimq::embedded_time::{self, Clock, Instant};
use minimq::{ConfigBuilder, Minimq, Publication};
use netx_sys::ULONG;
use static_cell::StaticCell;
use threadx_app::network::network::ThreadxTcpWifiNetwork;

use threadx_rs::allocator::ThreadXAllocator;
use threadx_rs::thread;

use threadx_rs::thread::Thread;
use threadx_rs::timer::Timer;

extern crate alloc;

pub type UINT = ::core::ffi::c_uint;

#[global_allocator]
static GLOBAL: ThreadXAllocator = ThreadXAllocator::new();

// Used for Rust heap allocation via global allocator
static HEAP: StaticCell<[u8; 1024]> = StaticCell::new();

// Wifi thread globals
static WIFI_THREAD_STACK: StaticCell<[u8; 8192]> = StaticCell::new();
static WIFI_THREAD: StaticCell<Thread> = StaticCell::new();

#[cortex_m_rt::entry]
fn main() -> ! {
    let tx = threadx_rs::Builder::new(
        |ticks_per_second| {
            BoardMxAz3166::low_level_init(ticks_per_second).unwrap();
        },
        |mem_start| {
            defmt::println!("Define application. Memory starts at: {} ", mem_start);

            let heap = HEAP.init_with(||[0u8; 1024]);
            GLOBAL.initialize(heap).unwrap();

            // Static Cell since we need an allocated but uninitialized block of memory
            let wifi_thread_stack = WIFI_THREAD_STACK.init_with(|| [0u8; 8192]);

            let wifi_thread: &'static mut Thread = WIFI_THREAD.init(Thread::new());

            let _ = wifi_thread
                .initialize_with_autostart_box(
                    "wifi_thread",
                    Box::new(do_network),
                    wifi_thread_stack,
                    4,
                    4,
                    0,
                )
                .unwrap();
        },
    );

    tx.initialize();
    println!("Exit");
    threadx_app::exit()
}

fn start_clock() -> impl Clock {
    static TICKS: AtomicU32 = AtomicU32::new(0);

    // TODO: Hardware Clock implementation
    struct ThreadXSecondClock {}

    impl embedded_time::Clock for ThreadXSecondClock {
        type T = u32;

        const SCALING_FACTOR: embedded_time::rate::Fraction = Fraction::new(1, 1);

        fn try_now(&self) -> Result<embedded_time::Instant<Self>, embedded_time::clock::Error> {
            Ok(Instant::new(
                TICKS.fetch_add(0, core::sync::atomic::Ordering::Relaxed),
            ))
        }
    }

    extern "C" fn clock_tick(_arg: ULONG) {
        TICKS.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    }

    // Start the clock timer --> Should be done in Hardware but we do it via ThreadX for the fun of it

    static CLOCK_TIMER: StaticCell<Timer> = StaticCell::new();
    let clock_timer = CLOCK_TIMER.init(Timer::new());

    let clock_name = CStr::from_bytes_until_nul(b"clock_timer_mqtt\0").unwrap();
    let _ = clock_timer
        .initialize_with_fn(
            clock_name,
            Some(clock_tick),
            0,
            Duration::from_secs(1),
            Duration::from_secs(1),
            true,
        )
        .unwrap();
    ThreadXSecondClock {}
}

pub fn do_network() {
    defmt::println!("Initializing Network");
    let network = ThreadxTcpWifiNetwork::initialize("SSID", "PW").unwrap();
    defmt::println!("Network initialized");

    let remote_addr = SocketAddr::new(core::net::IpAddr::V4(Ipv4Addr::new(192, 168, 2, 105)), 1883);
    let mut buffer = [0u8; 128];
    let mqtt_cfg = ConfigBuilder::new(IpBroker::new(remote_addr.ip()), &mut buffer)
        .keepalive_interval(60)
        .client_id("mytest")
        .unwrap();

    let clock = start_clock();
    let mut mqtt_client = Minimq::new(network, clock, mqtt_cfg);
    loop {
        match mqtt_client.poll(|_client, _topic, _payload, _properties| 1) {
            Ok(_) => (),
            Err(minimq::Error::Network(e)) => {
                defmt::println!("Network disconnect, trying to reconnect.")
            }
            Err(minimq::Error::SessionReset) => {
                defmt::println!("Session reset.")
            }
            _ => panic!("Error during poll, giving up."),
        }
        if mqtt_client.client().is_connected() {
            let _ = mqtt_client
                .client()
                .publish(Publication::new("/cellar/temperature", "1.25"));
        }

        // Poll every 500ms
        let _ = thread::sleep(Duration::from_millis(500)).unwrap();
    }
}
