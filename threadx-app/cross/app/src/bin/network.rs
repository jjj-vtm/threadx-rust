#![no_main]
#![no_std]

use core::cell::RefCell;
use core::net::{Ipv4Addr, SocketAddr};
use core::sync::atomic::AtomicU32;
use core::time::Duration;

use alloc::boxed::Box;
use alloc::vec::Vec;
use board::{hts221, BoardMxAz3166, DisplayType, I2CBus, LowLevelInit};

use cortex_m::interrupt;
use cortex_m::itm::Aligned;
use defmt::println;
use embedded_graphics::mono_font::ascii::FONT_9X18;
use heapless::String;
use minimq::broker::IpBroker;
use minimq::embedded_time::rate::Fraction;
use minimq::embedded_time::{self, Clock, Instant};
use minimq::{ConfigBuilder, Minimq};
use netx_sys::ULONG;
use static_cell::StaticCell;
use threadx_app::minimqtransport::MiniMqBasedTransport;
use threadx_app::network::network::ThreadxTcpWifiNetwork;

use threadx_app::uprotocol_v1::UMessage;
use threadx_app::utransport::LocalUTransport;
use threadx_rs::allocator::ThreadXAllocator;
use threadx_rs::event_flags::GetOption::*;
use threadx_rs::event_flags::{EventFlagsGroup, EventFlagsGroupHandle};

use threadx_rs::executor::Executor;
use threadx_rs::mutex::Mutex;
use threadx_rs::queue::{Queue, QueueReceiver, QueueSender};
use threadx_rs::thread::{self, sleep};
use threadx_rs::WaitOption::*;

use threadx_rs::thread::Thread;
use threadx_rs::timer::Timer;

use core::fmt::Write;

use embedded_graphics::{
    mono_font::MonoTextStyleBuilder,
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Baseline, Text},
};

extern crate alloc;

pub type UINT = ::core::ffi::c_uint;
#[derive(Copy, Clone)]
pub enum Event {
    TemperatureMeasurement(i32),
}
impl Into<Vec<u8>> for Event {
    fn into(self) -> Vec<u8> {
        let mut str = String::<4>::new();

        let measure = match self {
            Event::TemperatureMeasurement(m) => m,
        };
        let _ = write!(str, "{measure}");
        str.as_bytes().to_vec()
    }
}

pub enum FlagEvents {
    WifiConnected = 1,
    WifiDisconnected = 2,
}

#[global_allocator]
static GLOBAL: ThreadXAllocator = ThreadXAllocator::new();

// Used for Rust heap allocation via global allocator
static HEAP: StaticCell<[u8; 1024]> = StaticCell::new();

// Wifi thread globals
static WIFI_THREAD_STACK: StaticCell<[u8; 4096]> = StaticCell::new();
static WIFI_THREAD: StaticCell<Thread> = StaticCell::new();

static MEASURE_THREAD_STACK: StaticCell<[u8; 1024]> = StaticCell::new();
static MEASURE_THREAD: StaticCell<Thread> = StaticCell::new();

static BOARD: cortex_m::interrupt::Mutex<RefCell<Option<BoardMxAz3166<I2CBus>>>> =
    cortex_m::interrupt::Mutex::new(RefCell::new(None));
static QUEUE: StaticCell<Queue<Event>> = StaticCell::new();
static QUEUE_MEM: StaticCell<[u8; 128]> = StaticCell::new();

static EVENT_GROUP: StaticCell<EventFlagsGroup> = StaticCell::new();
static DISPLAY: StaticCell<Mutex<Option<DisplayType<I2CBus>>>> = StaticCell::new();

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

            // Initialize global heap
            let heap = Aligned([0; 1024]);
            let heap_mem = HEAP.init_with(|| heap.0);

            GLOBAL.initialize(heap_mem).unwrap();

            // Get the peripherals
            let display_ref = DISPLAY.init(Mutex::new(None));
            let _ = display_ref.initialize(c"display_mtx", false).unwrap();
            let display = interrupt::free(|cs| {
                let mut board = BOARD.borrow(cs).borrow_mut();
                board.as_mut().unwrap().display.take().unwrap()
            });
            {
                // Temporary scope to hold the lock
                let mut display_guard = display_ref.lock(WaitForever).unwrap();
                display_guard.replace(display);
            }
            let (hts211, i2c) = interrupt::free(|cs| {
                let mut board = BOARD.borrow(cs).borrow_mut();
                let board = board.as_mut().unwrap();
                (
                    board.temp_sensor.take().unwrap(),
                    board.i2c_bus.take().unwrap(),
                )
            });

            // Create communication queue
            let qm = QUEUE_MEM.init_with(|| [0u8; 128]);
            let queue = QUEUE.init(Queue::new());
            let (sender, receiver) = queue.initialize(c"m_queue", qm).unwrap();

            // create events flag group
            let event_group = EVENT_GROUP.init(EventFlagsGroup::new());
            let evt_handle = event_group.initialize(c"event_flag").unwrap();

            // Static Cell since we need an allocated but uninitialized block of memory
            let wifi_thread_stack = WIFI_THREAD_STACK.init_with(|| [0u8; 4096]);
            let wifi_thread: &'static mut Thread = WIFI_THREAD.init(Thread::new());
            let _ = wifi_thread
                .initialize_with_autostart_box(
                    "wifi_thread",
                    Box::new(move || do_network(receiver, evt_handle, display_ref)),
                    wifi_thread_stack,
                    4,
                    4,
                    0,
                )
                .unwrap();
            println!("WLAN thread started");

            let measure_thread_stack = MEASURE_THREAD_STACK.init_with(|| [0u8; 1024]);
            let measure_thread: &'static mut Thread = MEASURE_THREAD.init(Thread::new());

            let _ = measure_thread
                .initialize_with_autostart_box(
                    "measurement_thread",
                    Box::new(move || do_measurement(sender, evt_handle, hts211, i2c)),
                    measure_thread_stack,
                    4,
                    4,
                    0,
                )
                .unwrap();

            println!("Measure thread started");
        },
    );

    tx.initialize();
    println!("Exit");
    threadx_app::exit()
}

fn do_measurement(
    snd: QueueSender<Event>,
    evt_handle: EventFlagsGroupHandle,
    mut hts221: hts221::HTS221<I2CBus, stm32f4xx_hal::i2c::Error>,
    mut i2c: I2CBus,
) {
    let _res = evt_handle
        .get(
            FlagEvents::WifiConnected as u32,
            WaitAllAndClear,
            WaitForever,
        )
        .unwrap();
    println!("WLAN connected, beginning to measure");
    loop {
        let deg = hts221.temperature_x8(&mut i2c).unwrap() as i32;
        let _ = snd.send(Event::TemperatureMeasurement(deg), WaitForever);
        println!("Current temperature: {}", deg);
        let _ = sleep(Duration::from_secs(5));
    }
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

    let _ = clock_timer
        .initialize_with_fn(
            c"clock_timer_mqtt",
            Some(clock_tick),
            0,
            Duration::from_secs(1),
            Duration::from_secs(1),
            true,
        )
        .unwrap();
    ThreadXSecondClock {}
}

fn print_text(text: &str, display: &mut DisplayType<I2CBus>) {
    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_9X18)
        .text_color(BinaryColor::On)
        .build();
    display.clear_buffer();
    Text::with_baseline(text, Point::zero(), text_style, Baseline::Top)
        .draw(display)
        .unwrap();

    display.flush().unwrap();
}

pub fn do_network(
    recv: QueueReceiver<Event>,
    evt_handle: EventFlagsGroupHandle,
    display: &Mutex<Option<DisplayType<I2CBus>>>,
) -> ! {
    defmt::println!("Initializing Network");
    // Initialize the globlal async executor
    let executor = Executor::new();
    let mut display = display.lock(WaitForever).unwrap().take().unwrap();
    print_text("WLAN()\nMQTT()", &mut display);
    let network = ThreadxTcpWifiNetwork::initialize("", "");
    if network.is_err() {
        print_text("Failure :(", &mut display);
        panic!();
    }
    let network = network.unwrap();
    defmt::println!("Network initialized");
    let remote_addr = SocketAddr::new(core::net::IpAddr::V4(Ipv4Addr::new(192, 168, 2, 100)), 1883);
    let mut buffer = [0u8; 512];
    let mqtt_cfg = ConfigBuilder::new(IpBroker::new(remote_addr.ip()), &mut buffer)
        .keepalive_interval(60)
        .client_id("mytest")
        .unwrap();

    print_text("WLAN(x)\nMQTT()", &mut display);
    let clock = start_clock();
    let mut transport = MiniMqBasedTransport::new(Minimq::new(network, clock, mqtt_cfg));
    // Signal that measurements can begin
    let _res = evt_handle
        .publish(FlagEvents::WifiConnected as u32)
        .unwrap();

    loop {
        // Need to poll the transport in order to keep it connected
        transport.poll();
        if transport.is_connected() {
            print_text("WLAN(x)\nMQTT(x)", &mut display);
            if let Ok(evt) = recv.receive(NoWait) {
                // TODO: Use upRust to do it all properly. This creates a very simple (valid) uMessage MQTT payload.
                // Create a umessage and then call send and block_on from the
                let mut umessage = UMessage::default();
                umessage.payload.replace(evt.into());

                let _res = executor.block_on(transport.send(umessage));
            }
        } else {
            print_text("WLAN(x)\nMQTT()", &mut display);
        }
        // Poll every 1000ms
        let _ = thread::sleep(Duration::from_millis(1000)).unwrap();
    }
}
