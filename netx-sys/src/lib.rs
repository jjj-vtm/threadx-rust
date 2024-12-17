#![no_std]

use core::ffi::c_void;

include!("generated.rs");

// Functions that are implemented in assembly that are missed by bindgen
// TODO: the SVCall and PendSV call are probably specific to Arm Cortex
// and should be enabled based on the selected target.
extern "C" {
    pub fn SDIO_IRQHandler() -> ();
    pub fn DMA2_Stream3_IRQHandler() -> ();
}

#[no_mangle]
pub unsafe extern "C" fn printf(format: *const core::ffi::c_char) -> core::ffi::c_int {
    1
}

extern "C" {
    pub fn nx_rand16() -> UINT;
}

#[no_mangle]
pub unsafe extern "C" fn rand() -> UINT {
    nx_rand16()
}
// Constants that are not parsed by bindgen

pub const NX_IPV4_PACKET: ULONG = (14 + 12 + 18) + 20; /* Phyisical Header (Wiced) + 20 bytes of IP header          */
pub const NX_IPV4_TCP_PACKET: ULONG = (NX_IPV4_PACKET + 20); /* IP header plus 20 bytes        */
pub const NX_DONT_FRAGMENT: ULONG = 0x00004000 as ULONG;
pub const NX_IP_NORMAL: ULONG = 0x00000000 as ULONG;
pub const NX_NO_WAIT: u32 = 0;
pub const NX_WAIT_FOREVER: u32 = 0xFFFFFFFF;
pub const NX_IP_ADDRESS_RESOLVED: ULONG = 0x0002 as ULONG;

pub const NX_SUCCESS: UINT = 0;
pub const NX_NO_PACKET: UINT = 0x01;
pub const NX_UNDERFLOW: UINT = 0x02;
pub const NX_OVERFLOW: UINT = 0x03;
pub const NX_NO_MAPPING: UINT = 0x04;
pub const NX_DELETED: UINT = 0x05;
pub const NX_POOL_ERROR: UINT = 0x06;
pub const NX_PTR_ERROR: UINT = 0x07;
pub const NX_WAIT_ERROR: UINT = 0x08;
pub const NX_SIZE_ERROR: UINT = 0x09;
pub const NX_OPTION_ERROR: UINT = 0x0a;
pub const NX_DELETE_ERROR: UINT = 0x10;
pub const NX_CALLER_ERROR: UINT = 0x11;
pub const NX_INVALID_PACKET: UINT = 0x12;
pub const NX_INVALID_SOCKET: UINT = 0x13;
pub const NX_NOT_ENABLED: UINT = 0x14;
pub const NX_ALREADY_ENABLED: UINT = 0x15;
pub const NX_ENTRY_NOT_FOUND: UINT = 0x16;
pub const NX_NO_MORE_ENTRIES: UINT = 0x17;
pub const NX_ARP_TIMER_ERROR: UINT = 0x18;
pub const NX_RESERVED_CODE0: UINT = 0x19;
pub const NX_WAIT_ABORTED: UINT = 0x1A;
pub const NX_IP_INTERNAL_ERROR: UINT = 0x20;
pub const NX_IP_ADDRESS_ERROR: UINT = 0x21;
pub const NX_ALREADY_BOUND: UINT = 0x22;
pub const NX_PORT_UNAVAILABLE: UINT = 0x23;
pub const NX_NOT_BOUND: UINT = 0x24;
pub const NX_RESERVED_CODE1: UINT = 0x25;
pub const NX_SOCKET_UNBOUND: UINT = 0x26;
pub const NX_NOT_CREATED: UINT = 0x27;
pub const NX_SOCKETS_BOUND: UINT = 0x28;
pub const NX_NO_RESPONSE: UINT = 0x29;
pub const NX_POOL_DELETED: UINT = 0x30;
pub const NX_ALREADY_RELEASED: UINT = 0x31;
pub const NX_RESERVED_CODE2: UINT = 0x32;
pub const NX_MAX_LISTEN: UINT = 0x33;
pub const NX_DUPLICATE_LISTEN: UINT = 0x34;
pub const NX_NOT_CLOSED: UINT = 0x35;
pub const NX_NOT_LISTEN_STATE: UINT = 0x36;
pub const NX_IN_PROGRESS: UINT = 0x37;
pub const NX_NOT_CONNECTED: UINT = 0x38;
pub const NX_WINDOW_OVERFLOW: UINT = 0x39;
pub const NX_ALREADY_SUSPENDED: UINT = 0x40;
pub const NX_DISCONNECT_FAILED: UINT = 0x41;
pub const NX_STILL_BOUND: UINT = 0x42;
pub const NX_NOT_SUCCESSFUL: UINT = 0x43;
pub const NX_UNHANDLED_COMMAND: UINT = 0x44;
pub const NX_NO_FREE_PORTS: UINT = 0x45;
pub const NX_INVALID_PORT: UINT = 0x46;
pub const NX_INVALID_RELISTEN: UINT = 0x47;
pub const NX_CONNECTION_PENDING: UINT = 0x48;
pub const NX_TX_QUEUE_DEPTH: UINT = 0x49;
pub const NX_NOT_IMPLEMENTED: UINT = 0x4A;
pub const NX_NOT_SUPPORTED: UINT = 0x4B;
pub const NX_INVALID_INTERFACE: UINT = 0x4C;
pub const NX_INVALID_PARAMETERS: UINT = 0x4D;
pub const NX_NOT_FOUND: UINT = 0x4E;
pub const NX_CANNOT_START: UINT = 0x4F;
pub const NX_NO_INTERFACE_ADDRESS: UINT = 0x50;
pub const NX_INVALID_MTU_DATA: UINT = 0x51;
pub const NX_DUPLICATED_ENTRY: UINT = 0x52;
pub const NX_PACKET_OFFSET_ERROR: UINT = 0x53;
