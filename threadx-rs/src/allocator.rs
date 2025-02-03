use crate::error::TxError;
use crate::{tx_checked_call, tx_str};
use core::sync::atomic::AtomicBool;
use core::{
    alloc::{GlobalAlloc, Layout},
    ffi::{c_void, CStr},
    mem::MaybeUninit,
};
use defmt::{error, println};
use num_traits::FromPrimitive;
use threadx_sys::{
    _tx_byte_allocate, _tx_byte_pool_create, _tx_byte_release, TX_BYTE_POOL, TX_WAIT_FOREVER, ULONG,
};

/// ThreadX allocator for Rust. Instantiate this struct and use it as the global allocator.
///
///  `
///  #[global_allocator]
///  static mut GLOBAL: ThreadXAllocator = ThreadXAllocator::new();
///  unsafe{GLOBAL.initialize(bp1_mem).unwrap()};
///  `

// We use a static mut and initialize it to zero. After this we only work with raw pointers to this static mut to avoid UB by accidentally creating aliasing mut references
static mut POOL_STRUCT: TX_BYTE_POOL = unsafe { MaybeUninit::zeroed().assume_init() };

pub struct ThreadXAllocator {
    pool_ptr: *mut TX_BYTE_POOL,
    initialized: AtomicBool,
}

unsafe impl Sync for ThreadXAllocator {}

impl ThreadXAllocator {
    pub const fn new() -> Self {

        // TODO: Make this return None if already initialized
        let allocator = ThreadXAllocator {
            pool_ptr: &raw mut POOL_STRUCT,
            initialized: AtomicBool::new(false),
        };
        
        allocator
    }

    pub fn initialize(&'static self, pool_memory: &mut [u8]) -> Result<(), TxError> {
        // Panic if initialized twice. Check if name is not global (and not zero)
        let res = tx_checked_call!(_tx_byte_pool_create(
            self.pool_ptr,
            tx_str!("global").as_ptr() as *mut i8,
            pool_memory.as_mut_ptr() as *mut core::ffi::c_void,
            pool_memory.len() as ULONG
        ));
        // Set the allocator to initialized
        self.initialized
            .fetch_or(true, core::sync::atomic::Ordering::Relaxed);
        res
    }
}

unsafe impl GlobalAlloc for ThreadXAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !self.initialized.load(core::sync::atomic::Ordering::Relaxed) {
            panic!("Use of ThreadX allocator before it was initialized");
        }
        // TODO: Handle alignment
        let mut ptr: *mut c_void = core::ptr::null_mut() as *mut c_void;
        // Safety: _tx_byte_allocate is thread safe so it is ok to use the pool_ptr ie. a pointer into the static mut struct
        tx_checked_call!(_tx_byte_allocate(
            self.pool_ptr,
            &mut ptr,
            layout.size() as ULONG,
            TX_WAIT_FOREVER
        ))
        .map(|_| ptr as *mut u8)
        .unwrap()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        // Safety: _tx_byte_allocate is thread safe so it is ok to use the pool_ptr ie. a pointer into the static mut struct

        tx_checked_call!(_tx_byte_release(ptr as *mut c_void)).unwrap()
    }
}
