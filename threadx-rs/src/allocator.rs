use crate::error::TxError;
use crate::tx_checked_call;
use core::panic;
use core::sync::atomic::AtomicBool;
use core::{
    alloc::{GlobalAlloc, Layout},
    ffi::c_void,
    mem::MaybeUninit,
};
use num_traits::FromPrimitive;
use threadx_sys::{
    _tx_byte_allocate, _tx_byte_pool_create, _tx_byte_release, CHAR, TX_BYTE_POOL, TX_WAIT_FOREVER,
    ULONG,
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
static INITIALIZED: AtomicBool = AtomicBool::new(false);
pub struct ThreadXAllocator {
    pool_ptr: *mut TX_BYTE_POOL,
}

unsafe impl Sync for ThreadXAllocator {}

impl ThreadXAllocator {
    pub const fn new() -> Self {
        // TODO: Make this return None if already initialized
        let allocator = ThreadXAllocator {
            pool_ptr: &raw mut POOL_STRUCT,
        };

        allocator
    }

    pub fn initialize(&'static self, pool_memory: &'static mut [u8]) -> Result<(), TxError> {
        if INITIALIZED.load(core::sync::atomic::Ordering::Relaxed) {
            panic!("ThreadXAllocator already initialized");
        }
        let pool_name = c"global";

        let res = tx_checked_call!(_tx_byte_pool_create(
            self.pool_ptr,
            pool_name.as_ptr() as *mut CHAR,
            pool_memory.as_mut_ptr() as *mut core::ffi::c_void,
            pool_memory.len() as ULONG
        ));
        // Set the allocator to initialized
        INITIALIZED.store(true, core::sync::atomic::Ordering::Release);
        res
    }
}

unsafe impl GlobalAlloc for ThreadXAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // We do not support 0 sized types currently
        assert!(layout.size() != 0);

        if !INITIALIZED.load(core::sync::atomic::Ordering::Acquire) {
            // .:was
            panic!("Use of ThreadX allocator before it was initialized");
        }
        let mut ptr: *mut c_void = core::ptr::null_mut() as *mut c_void;
        let layout = layout.pad_to_align();

        defmt::debug!("Allocation of size: {}", layout.size());
        // Safety: _tx_byte_allocate is thread safe so it is ok to use the pool_ptr ie. a pointer into the static mut struct
        let res = tx_checked_call!(_tx_byte_allocate(
            self.pool_ptr,
            &mut ptr,
            layout.size() as ULONG,
            TX_WAIT_FOREVER
        ));
        let Ok(_) = res else {
            defmt::error!("Allocation failed returning null");
            return core::ptr::null_mut();
        };
        unsafe { ptr.add(ptr.align_offset(layout.align())).cast() }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        tx_checked_call!(_tx_byte_release(ptr as *mut c_void)).expect("Deallocation failed")
    }
}
