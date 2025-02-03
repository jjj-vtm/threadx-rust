use core::{
    ffi::{c_void, CStr},
    marker::PhantomData,
    mem::MaybeUninit,

};

use threadx_sys::{
    _tx_block_allocate, _tx_block_pool_create, _tx_block_pool_delete, _tx_block_pool_info_get,
    _tx_block_pool_prioritize, _tx_block_release, _tx_byte_allocate, _tx_byte_pool_create,
    _tx_byte_pool_delete, _tx_byte_release, TX_BLOCK_POOL, TX_BYTE_POOL, TX_NO_WAIT,
    TX_WAIT_FOREVER, ULONG,
};

use crate::tx_checked_call;

use super::error::TxError;
use defmt::error;
use num_traits::FromPrimitive;

pub struct BytePool(MaybeUninit<TX_BYTE_POOL>);
impl BytePool {
    /// Create a new BytePool. This is a const function because we want to create static instances
    /// of the byte pool. Rust code will never access the inner structure directly, so we leave
    /// it as uninitialized, even though we know that it will be initialized by the threadx call.
    /// This will also prevent rust from trying to drop the inner structure.
    pub const fn new() -> Self {
        BytePool(MaybeUninit::<TX_BYTE_POOL>::uninit())
    }

    // From safe rust this cannot be called more than once for a given self since it is mutable borrowed for 'static
    pub fn initialize<'pool_memory>(
        &'static mut self,
        name: &CStr,
        pool_memory: &'pool_memory mut [u8],
    ) -> Result<BytePoolHandle<'pool_memory>, TxError> {
        let pool_ptr = self.0.as_mut_ptr();

        defmt::println!(
            "Pool ptr: {} name:{} memory:{}",
            pool_ptr,
            name.as_ptr(),
            pool_memory.as_mut_ptr()
        );

        tx_checked_call!(_tx_byte_pool_create(
            pool_ptr,
            name.as_ptr() as *mut i8,
            pool_memory.as_mut_ptr() as *mut core::ffi::c_void,
            pool_memory.len() as ULONG
        ))
        .map(|_| BytePoolHandle::new(pool_ptr, PhantomData))
    }
}

pub struct MemoryBlock<'block>(&'block mut [u8]);

impl<'block> MemoryBlock<'block> {
    // Not public as it is constructued by the BytePoolHandle
    pub fn new(mem: &'block mut [u8]) -> MemoryBlock<'block> {
        MemoryBlock(mem)
    }

    pub fn consume(self: MemoryBlock<'block>) -> &'block mut [u8] {
        self.0
    }
}

pub struct BytePoolHandle<'a> {
    pool_ptr: *mut TX_BYTE_POOL,
    phantom: PhantomData<&'a [u8]>,
}


impl<'a> BytePoolHandle<'a> {
    fn new(
        ptr: *mut TX_BYTE_POOL,
        memory_lifetime_phantom: PhantomData<&'a [u8]>,
    ) -> BytePoolHandle<'a> {
        assert!(!ptr.is_null(), "Pool ptr is null");
        BytePoolHandle {
            pool_ptr: ptr,
            phantom: memory_lifetime_phantom,
        }
    }

    pub fn allocate(
        &self,
        size: usize,
        wait: bool,
    ) -> Result<MemoryBlock<'a>, TxError> {
        let mut ptr: *mut c_void = core::ptr::null_mut() as *mut c_void;
    
        tx_checked_call!(_tx_byte_allocate(
            self.pool_ptr,
            &mut ptr,
            size as ULONG,
            if wait { TX_WAIT_FOREVER } else { TX_NO_WAIT }
        ))
        .map(|_| MemoryBlock(unsafe { core::slice::from_raw_parts_mut(ptr as *mut u8, size) }))
    }    

    pub fn release(&self, mem: &mut [u8]) -> Result<(), TxError> {
        tx_checked_call!(_tx_byte_release(mem.as_mut_ptr() as *mut c_void))
    }

    pub fn delete(self) -> Result<(), TxError> {
        tx_checked_call!(_tx_byte_pool_delete(self.pool_ptr))
    }
}

pub struct BlockPool(MaybeUninit<TX_BLOCK_POOL>);

impl BlockPool {
    pub const fn new() -> Self {
        BlockPool(core::mem::MaybeUninit::uninit())
    }

    pub fn initialize<'pool_memory>(
        &'static mut self,
        name: &CStr,
        block_size: usize,
        pool_memory: &'pool_memory mut [u8],
    ) -> Result<BlockPoolHandle<'pool_memory>, TxError> {
        let pool_ptr = self.0.as_mut_ptr();

        tx_checked_call!(_tx_block_pool_create(
            pool_ptr,
            name.as_ptr() as *mut i8,
            block_size as ULONG,
            pool_memory.as_mut_ptr() as *mut core::ffi::c_void,
            pool_memory.len() as ULONG
        ))
        .map(|_| BlockPoolHandle(pool_ptr, PhantomData))
    }
}

pub struct BlockPoolHandle<'a> (*mut TX_BLOCK_POOL, PhantomData<&'a [u8]>,
);

impl<'memory> BlockPoolHandle<'memory> {
    pub fn allocate(&mut self, wait: bool) -> Result<&'memory mut [u8], TxError> {
        let mut ptr: *mut c_void = core::ptr::null_mut() as *mut c_void;
        tx_checked_call!(_tx_block_allocate(
            self.0,
            &mut ptr,
            if wait { TX_WAIT_FOREVER } else { TX_NO_WAIT }
        ))
        .map(|_| unsafe {
            core::slice::from_raw_parts_mut(
                ptr as *mut u8,
                (*self.0).tx_block_pool_block_size as usize,
            )
        })
    }

    pub fn release(&mut self, mem: &mut [u8]) -> Result<(), TxError> {
        tx_checked_call!(_tx_block_release(mem.as_mut_ptr() as *mut c_void))
    }

    /*
        #define tx_block_allocate                           _tx_block_allocate
    #define tx_block_pool_create                        _tx_block_pool_create
    #define tx_block_pool_delete                        _tx_block_pool_delete
    #define tx_block_pool_info_get                      _tx_block_pool_info_get
    #define tx_block_pool_performance_info_get          _tx_block_pool_performance_info_get
    #define tx_block_pool_performance_system_info_get   _tx_block_pool_performance_system_info_get
    #define tx_block_pool_prioritize                    _tx_block_pool_prioritize
    #define tx_block_release                            _tx_block_release
         */

    pub fn prioritize(&mut self, mem: &'static mut [u8]) -> Result<(), TxError> {
        tx_checked_call!(_tx_block_pool_prioritize(self.0))
    }

    // Free the block pool
    pub fn delete(self) -> Result<(), TxError> {
        tx_checked_call!(_tx_block_pool_delete(self.0))
    }
}
