#![no_main]
#![no_std]

use core::cell::RefCell;
use core::ffi::CStr;
use core::net::{Ipv4Addr, SocketAddr};
use core::sync::atomic::AtomicU32;
use core::time::Duration;

use alloc::boxed::Box;
use board::{BoardMxAz3166, I2CBus, LowLevelInit};

use cortex_m::interrupt;
use cortex_m::itm::Aligned;
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
static WIFI_THREAD_STACK: StaticCell<[u8; 4096]> = StaticCell::new();
static WIFI_THREAD: StaticCell<Thread> = StaticCell::new();

static MEASURE_THREAD_STACK: StaticCell<[u8; 1024]> = StaticCell::new();
static MEASURE_THREAD: StaticCell<Thread> = StaticCell::new();

static BOARD: cortex_m::interrupt::Mutex<RefCell<Option<BoardMxAz3166<I2CBus>>>> = cortex_m::interrupt::Mutex::new(RefCell::new(None));

#[cortex_m_rt::entry]
fn main() -> ! {
    let tx = threadx_rs::Builder::new(
        |ticks_per_second| {
            let board = BoardMxAz3166::low_level_init(ticks_per_second).unwrap();
            // ThreadX mutexes cannot be used here.
            interrupt::free(|cs| BOARD.borrow(cs).borrow_mut().replace(board));
        },
        |mem_start| {
            defmt::println!("Define application. Memory starts at: {} ", mem_start);

            let heap = Aligned([0; 1024]);
            let heap_mem = HEAP.init_with(|| heap.0);

            GLOBAL.initialize(heap_mem).unwrap();

            // Static Cell since we need an allocated but uninitialized block of memory
            let wifi_thread_stack = WIFI_THREAD_STACK.init_with(|| [0u8; 4096]);
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

            let measure_thread_stack = MEASURE_THREAD_STACK.init_with(|| [0u8; 1024]);
            let measure_thread: &'static mut Thread = MEASURE_THREAD.init(Thread::new());

            let _ = measure_thread
                .initialize_with_autostart(
                    "measurement_thread",
                    Box::new(do_measurement),
                    measure_thread_stack,
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

fn do_measurement() {
    /*
     * - Only start measurements after Wifi and MQTT is connected.
     * - Implement via event_handle
     * - Run measurement every 5 seconds
     * - Publish data via Queue to network thread
     */
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
