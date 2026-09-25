use crate::error::TxError;
use crate::tx_checked_call;
use core::panic;
use core::sync::atomic::AtomicBool;
use core::{
    alloc::{GlobalAlloc, Layout},
    ffi::c_void,
};
use num_traits::FromPrimitive;
use threadx_sys::{
    _tx_byte_allocate, _tx_byte_pool_create, _tx_byte_release, TX_BYTE_POOL, TX_WAIT_FOREVER,
    ULONG,
};

// We use a static mut and initialize it to zero. After this we only work with raw pointers to this static mut to avoid UB by accidentally creating aliasing mut references
static mut POOL_STRUCT: TX_BYTE_POOL = unsafe { core::mem::zeroed() };
/// Alignment of every block handed out by a ThreadX byte pool, provided the pool memory itself is
/// aligned to it. ThreadX rounds all block sizes and headers to `ALIGN_TYPE`, which defaults to `ULONG`.
const POOL_ALIGN: usize = align_of::<ULONG>();
// The over-aligned path stores a pointer in the (at least POOL_ALIGN sized) gap in front of the block.
const _: () = assert!(size_of::<*mut u8>() <= POOL_ALIGN);
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// ThreadX allocator for Rust. Instantiate this struct and use it as the global allocator.
///
/// ```ignore
/// #[global_allocator]
/// static GLOBAL: ThreadXAllocator = ThreadXAllocator::new();
/// GLOBAL.initialize(heap_mem).unwrap();
/// ```
pub struct ThreadXAllocator {
    pool_ptr: *mut TX_BYTE_POOL,
}

unsafe impl Sync for ThreadXAllocator {}

impl ThreadXAllocator {
    pub const fn new() -> Self {
        // TODO: Make this return None if already initialized
        ThreadXAllocator {
            pool_ptr: &raw mut POOL_STRUCT,
        }
    }

    pub fn initialize(&'static self, pool_memory: &'static mut [u8]) -> Result<(), TxError> {
        if INITIALIZED.load(core::sync::atomic::Ordering::Relaxed) {
            panic!("ThreadXAllocator already initialized");
        }
        let pool_name = c"global";
        // ThreadX does not align the pool start itself. Skip leading bytes so all blocks are POOL_ALIGN aligned.
        let offset = pool_memory.as_ptr().align_offset(POOL_ALIGN);
        let pool_memory = pool_memory.get_mut(offset..).ok_or(TxError::SizeError)?;

        tx_checked_call!(_tx_byte_pool_create(
            self.pool_ptr,
            pool_name.as_ptr().cast_mut(),
            pool_memory.as_mut_ptr().cast(),
            pool_memory.len() as ULONG
        ))?;
        // Set the allocator to initialized
        INITIALIZED.store(true, core::sync::atomic::Ordering::Release);
        Ok(())
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
        // Blocks are POOL_ALIGN aligned. For larger alignments over-allocate and store the pointer returned
        // by ThreadX in the word right in front of the aligned block so dealloc can release it.
        let over_aligned = layout.align() > POOL_ALIGN;
        let size = if over_aligned {
            match layout.size().checked_add(layout.align()) {
                Some(size) => size,
                None => return core::ptr::null_mut(),
            }
        } else {
            layout.size()
        };
        let mut ptr: *mut c_void = core::ptr::null_mut();

        defmt::debug!("Allocation of size: {}", size);
        // Safety: _tx_byte_allocate is thread safe so it is ok to use the pool_ptr ie. a pointer into the static mut struct
        let res = tx_checked_call!(_tx_byte_allocate(
            self.pool_ptr,
            &raw mut ptr,
            size as ULONG,
            TX_WAIT_FOREVER
        ));
        let Ok(_) = res else {
            defmt::error!("Allocation failed returning null");
            return core::ptr::null_mut();
        };
        let raw = ptr.cast::<u8>();
        if !over_aligned {
            return raw;
        }
        // raw is POOL_ALIGN aligned, so the offset is in POOL_ALIGN..=align and leaves room for the header word.
        let offset = layout.align() - (raw.addr() & (layout.align() - 1));
        // Safety: offset + layout.size() <= size, so both the header and the block are inside the allocation.
        unsafe {
            let aligned = raw.add(offset);
            aligned.cast::<*mut u8>().sub(1).write(raw);
            aligned
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let raw = if layout.align() > POOL_ALIGN {
            // Safety: alloc stored the pointer returned by ThreadX right in front of the aligned block.
            unsafe { ptr.cast::<*mut u8>().sub(1).read() }
        } else {
            ptr
        };
        tx_checked_call!(_tx_byte_release(raw.cast())).expect("Deallocation failed")
    }
}
