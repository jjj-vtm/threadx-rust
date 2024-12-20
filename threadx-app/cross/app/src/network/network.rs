use core::{
    ffi::c_void,
    mem::MaybeUninit,
    net::Ipv4Addr,
    ptr::{self},
};

use minimq::embedded_nal::{TcpClientStack, TcpError};
use netx_sys::*;
use static_cell::StaticCell;
use threadx_sys::{UINT, ULONG};

use crate::nx_checked_call;

use super::NxError;

// Wiced constant
const MAX_BUS_HEADER_LENGTH: UINT = 12;
const MAX_SDPCM_HEADER_LENGTH: UINT = 18;
const WICED_LINK_OVERHEAD_BELOW_ETHERNET_FRAME_MAX: UINT =
    MAX_BUS_HEADER_LENGTH + MAX_SDPCM_HEADER_LENGTH;
const WICED_LINK_TAIL_AFTER_ETHERNET_FRAME: UINT = 0;
const WICED_ETHERNET_SIZE: UINT = 14;
const WICED_PAYLOAD_MTU: UINT = 1500;
const WICED_PHYSICAL_HEADER: UINT =
    WICED_LINK_OVERHEAD_BELOW_ETHERNET_FRAME_MAX + WICED_ETHERNET_SIZE;
const WICED_PHYSICAL_TRAILER: UINT = WICED_LINK_TAIL_AFTER_ETHERNET_FRAME;
const WICED_LINK_MTU: UINT = WICED_PAYLOAD_MTU + WICED_PHYSICAL_HEADER + WICED_PHYSICAL_TRAILER;

// Application specifc constants

const NETX_TX_PACKET_COUNT: UINT = 16;
const NETX_RX_PACKET_COUNT: UINT = 12;
const NETX_PACKET_SIZE: UINT = size_of::<NX_PACKET>() as UINT;
const NETX_RX_POOL_SIZE: UINT = (WICED_LINK_MTU + NETX_PACKET_SIZE) * NETX_RX_PACKET_COUNT;
const NETX_TX_POOL_SIZE: UINT = (WICED_LINK_MTU + NETX_PACKET_SIZE) * NETX_TX_PACKET_COUNT;

const NETX_IP_STACK_SIZE: u32 = 4096;
const NETX_ARP_CACHE_SIZE: UINT = 512;

static TX_PACKET_POOL_MEM: StaticCell<[u8; NETX_TX_POOL_SIZE as usize]> = StaticCell::new();
static RX_PACKET_POOL_MEM: StaticCell<[u8; NETX_RX_POOL_SIZE as usize]> = StaticCell::new();
static NETX_IP_STACK: StaticCell<[u8; NETX_IP_STACK_SIZE as usize]> = StaticCell::new();
static NETX_ARP_CACHE_AREA: StaticCell<[u8; NETX_ARP_CACHE_SIZE as usize]> = StaticCell::new();

static DHCP_CLIENT: StaticCell<NX_DHCP> = StaticCell::new();
static SOCKET_PTR: StaticCell<NX_TCP_SOCKET> = StaticCell::new();
static IP_PTR: StaticCell<NX_IP> = StaticCell::new();

// TODO: Replace with StaticCells
const TX_IDX: usize = 0;
const RX_IDX: usize = 1;

static mut POOL: [MaybeUninit<NX_PACKET_POOL>; 2] = [
    core::mem::MaybeUninit::uninit(),
    core::mem::MaybeUninit::uninit(),
];

pub struct ThreadxTcpWifiNetwork {
    socket: Option<NetxTcpSocket>,
}

pub struct NetxTcpSocket {
    socket_ptr: *mut NX_TCP_SOCKET,
}

static INITIALIZED: StaticCell<bool> = StaticCell::new();

#[derive(Debug)]
pub enum NetxTcpError {
    SocketClosed,
    UnsupportedProtocol,
    BufferTooSmall,
    NoSocketsAvailable,
    Unknown,
}

impl TcpError for NetxTcpError {
    fn kind(&self) -> minimq::embedded_nal::TcpErrorKind {
        match self {
            Self::SocketClosed => embedded_nal::TcpErrorKind::PipeClosed,
            _ => minimq::embedded_nal::TcpErrorKind::Other,
        }
    }
}

impl From<NxError> for NetxTcpError {
    fn from(value: NxError) -> Self {
        match value {
            NxError::SocketClosed => Self::SocketClosed,
            _ => Self::Unknown,
        }
    }
}

impl From<NxError> for stm32f4xx_hal::nb::Error<NetxTcpError> {
    fn from(value: NxError) -> Self {
        match value {
            NxError::SocketClosed => embedded_nal::nb::Error::Other(NetxTcpError::SocketClosed),
            _ => embedded_nal::nb::Error::Other(value.into()),
        }
    }
}

impl ThreadxTcpWifiNetwork {
    pub fn initialize(ssid_str: &str, pw: &str) -> Result<ThreadxTcpWifiNetwork, NxError> {
        let initialized = INITIALIZED.try_init(true);
        if initialized.is_none() {
            return Err(NxError::AlreadyInitialized);
        }

        unsafe { _nx_system_initialize() };

        let mut name = "TX 0\0".as_ptr() as *mut core::ffi::c_char;

        let pool_mem_ptr = TX_PACKET_POOL_MEM.uninit().as_mut_ptr();

        nx_checked_call!(_nx_packet_pool_create(
            POOL[TX_IDX].as_mut_ptr(),
            name,
            WICED_LINK_MTU,
            pool_mem_ptr as *mut core::ffi::c_void,
            NETX_TX_POOL_SIZE as u32
        ))?;

        name = "RX 0\0".as_ptr() as *mut core::ffi::c_char;

        let pool_mem_ptr = RX_PACKET_POOL_MEM.uninit().as_mut_ptr();

        nx_checked_call!(_nx_packet_pool_create(
            POOL[RX_IDX].as_mut_ptr(),
            name,
            WICED_LINK_MTU,
            pool_mem_ptr as *mut core::ffi::c_void,
            NETX_RX_POOL_SIZE as u32
        ))?;

        nx_checked_call!(wwd_buffer_init(POOL.as_mut_ptr() as *mut core::ffi::c_void))?;

        nx_checked_call!(wwd_management_wifi_on(
            wiced_country_code_t_WICED_COUNTRY_WORLD_WIDE_XX
        ))?;

        let mut mac = wiced_mac_t { octet: [0; 6] };

        nx_checked_call!(wwd_wifi_get_mac_address(
            &mut mac as *mut wiced_mac_t,
            wwd_interface_t_WWD_STA_INTERFACE
        ))?;

        name = "NetX IP Instance 0\0".as_ptr() as *mut core::ffi::c_char;

        let netx_ip_mem_ptr = NETX_IP_STACK.uninit().as_mut_ptr();
        let ip_ptr = IP_PTR.uninit();
        nx_checked_call!(_nx_ip_create(
            ip_ptr.as_mut_ptr(),
            name,
            Ipv4Addr::new(0, 0, 0, 0).to_bits(),
            Ipv4Addr::new(255, 255, 255, 0).to_bits(),
            POOL[TX_IDX].as_mut_ptr(),
            Some(wiced_sta_netx_duo_driver_entry),
            netx_ip_mem_ptr as *mut core::ffi::c_void,
            NETX_IP_STACK_SIZE,
            1
        ))?;

        /*
         * ARP Cache area needs some realignment to 4bytes
         */
        let arp_mem_ptr = NETX_ARP_CACHE_AREA.uninit().as_mut_ptr();
        let offset = arp_mem_ptr.align_offset(align_of::<u32>());
        let aligned_ptr = unsafe { arp_mem_ptr.add(offset) };

        nx_checked_call!(_nx_arp_enable(
            ip_ptr.as_mut_ptr(),
            aligned_ptr as *mut c_void,
            NETX_ARP_CACHE_SIZE - offset as UINT
        ))?;

        nx_checked_call!(_nx_tcp_enable(ip_ptr.as_mut_ptr()))?;

        nx_checked_call!(_nx_udp_enable(ip_ptr.as_mut_ptr()))?;

        nx_checked_call!(_nx_icmp_enable(ip_ptr.as_mut_ptr()))?;

        let hostname = "myBoard0".as_ptr() as *mut core::ffi::c_char;
        let dhcp_client_ptr = DHCP_CLIENT.uninit();

        nx_checked_call!(_nx_dhcp_create(
            dhcp_client_ptr.as_mut_ptr(),
            ip_ptr.as_mut_ptr(),
            hostname
        ))?;

        nx_checked_call!(_nx_dhcp_start(dhcp_client_ptr.as_mut_ptr()))?;
        let ssid_b = ssid_str.as_bytes();
        if ssid_b.len() > 32 {
            defmt::error!("SSID too long, must be 32bytes maximal");
            return Err(NxError::Unknown);
        }
        let mut ssid: [u8; 32] = [0u8; 32];
        ssid[..ssid_str.len()].copy_from_slice(ssid_b);

        let ssid = wiced_ssid_t {
            length: ssid_str.len() as u8,
            value: ssid,
        };

        nx_checked_call!(wwd_wifi_join(
            &ssid as *const wiced_ssid_t,
            wiced_security_t_WICED_SECURITY_WPA2_AES_PSK,
            pw.as_ptr(),
            pw.len() as u8,
            core::ptr::null_mut(),
            wwd_interface_t_WWD_STA_INTERFACE
        ))?;

        let mut actual_status: ULONG = 0;
        let mut ip_address: ULONG = 0;
        let mut network_mask: ULONG = 0;

        let mut gateway_address: ULONG = 0;

        nx_checked_call!(_nx_ip_status_check(
            ip_ptr.as_mut_ptr(),
            NX_IP_ADDRESS_RESOLVED,
            &mut actual_status as *mut ULONG,
            3000
        ))?;

        nx_checked_call!(_nx_ip_address_get(
            ip_ptr.as_mut_ptr(),
            &mut ip_address as *mut ULONG,
            &mut network_mask as *mut ULONG
        ))?;

        nx_checked_call!(_nx_ip_gateway_address_get(
            ip_ptr.as_mut_ptr(),
            &mut gateway_address as *mut ULONG
        ))?;

        let network = ThreadxTcpWifiNetwork {
            socket: Some(NetxTcpSocket {
                socket_ptr: Self::create_socket(ip_ptr.as_mut_ptr()).unwrap(),
            }),
        };

        Ok(network)
    }

    fn create_socket(ip_ptr: *mut NX_IP) -> Result<*mut NX_TCP_SOCKET, NxError> {
        let name = "TCP_SOCKET\0".as_ptr() as *mut core::ffi::c_char;
        let socket_ptr = SOCKET_PTR.uninit();

        nx_checked_call!(_nx_tcp_socket_create(
            ip_ptr,
            socket_ptr.as_mut_ptr(),
            name,
            NX_IP_NORMAL,
            NX_DONT_FRAGMENT,
            0x80,
            8192,
            None,
            None
        ))?;

        Ok(socket_ptr.as_mut_ptr())
    }
}

impl TcpClientStack for ThreadxTcpWifiNetwork {
    type TcpSocket = NetxTcpSocket;

    type Error = NetxTcpError;

    fn socket(&mut self) -> Result<Self::TcpSocket, Self::Error> {
        let socket = self.socket.take();
        if let Some(sock) = socket {
            Ok(sock)
        } else {
            Err(NetxTcpError::NoSocketsAvailable)
        }
    }

    fn connect(
        &mut self,
        socket: &mut Self::TcpSocket,
        remote: core::net::SocketAddr,
    ) -> embedded_nal::nb::Result<(), Self::Error> {
        match remote {
            core::net::SocketAddr::V4(socket_addr_v4) => {
                nx_checked_call!(_nx_tcp_client_socket_bind(
                    socket.socket_ptr,
                    0,
                    NX_WAIT_FOREVER
                ))?;

                let res = unsafe {
                    _nx_tcp_client_socket_connect(
                        socket.socket_ptr,
                        socket_addr_v4.ip().to_bits(),
                        socket_addr_v4.port() as u32,
                        NX_WAIT_FOREVER,
                    )
                };
                if res != NX_SUCCESS {
                    // Unbind and return the error
                    nx_checked_call!(_nx_tcp_client_socket_unbind(socket.socket_ptr))?;
                    return Err(stm32f4xx_hal::nb::Error::from(NxError::from_u32(res)));
                }

                Ok(())
            }
            core::net::SocketAddr::V6(_socket_addr_v6) => {
                return Err(embedded_nal::nb::Error::Other(
                    NetxTcpError::UnsupportedProtocol,
                ));
            }
        }
    }

    fn send(
        &mut self,
        socket: &mut Self::TcpSocket,
        buffer: &[u8],
    ) -> embedded_nal::nb::Result<usize, Self::Error> {
        let mut packet_ptr: *mut NX_PACKET = ptr::null_mut();
        let packet_ptr_ptr = ptr::addr_of_mut!(packet_ptr);

        if buffer.len() > 255 {
            return Err(embedded_nal::nb::Error::Other(NetxTcpError::BufferTooSmall));
        }
        let mut send_buffer: [u8; 256] = [0u8; 256];
        send_buffer[0..buffer.len()].copy_from_slice(buffer);

        nx_checked_call!(_nx_packet_allocate(
            POOL[TX_IDX].as_mut_ptr(),
            packet_ptr_ptr,
            NX_IPV4_TCP_PACKET,
            NX_WAIT_FOREVER
        ))?;

        nx_checked_call!(_nx_packet_data_append(
            packet_ptr,
            send_buffer.as_mut_ptr() as *mut c_void,
            buffer.len() as u32,
            POOL[TX_IDX].as_mut_ptr(),
            NX_WAIT_FOREVER
        ))?;

        nx_checked_call!(_nx_tcp_socket_send(
            socket.socket_ptr,
            packet_ptr,
            NX_WAIT_FOREVER
        ))?;

        Ok(buffer.len())
    }

    fn receive(
        &mut self,
        socket: &mut Self::TcpSocket,
        buffer: &mut [u8],
    ) -> embedded_nal::nb::Result<usize, Self::Error> {
        let mut packet_ptr: *mut NX_PACKET = ptr::null_mut();
        let packet_ptr_ptr = ptr::addr_of_mut!(packet_ptr);

        let res = unsafe { _nx_tcp_socket_receive(socket.socket_ptr, packet_ptr_ptr, NX_NO_WAIT) };

        if res == NX_SUCCESS {
            let mut bytes_copied: ULONG = 0;
            let res = unsafe {
                _nx_packet_data_retrieve(
                    packet_ptr,
                    buffer.as_mut_ptr() as *mut c_void,
                    &mut bytes_copied as *mut ULONG,
                )
            };

            // NetXDuo wants us to release if NX_SUCCESS was returned upon receive
            nx_checked_call!(_nx_packet_release(packet_ptr))?;
            if res == NX_SUCCESS {
                return Ok(bytes_copied as usize);
            } else {
                return Err(embedded_nal::nb::Error::Other(NetxTcpError::from(
                    NxError::from_u32(res),
                )));
            }
        } else if res == NX_NO_PACKET {
            return Err(embedded_nal::nb::Error::WouldBlock);
        } else {
            defmt::println!("Receive error: {}", res);
            return Err(embedded_nal::nb::Error::Other(NetxTcpError::from(
                NxError::from_u32(res),
            )));
        }
    }

    fn close(&mut self, socket: Self::TcpSocket) -> Result<(), Self::Error> {
        nx_checked_call!(_nx_tcp_socket_disconnect(
            socket.socket_ptr,
            NX_WAIT_FOREVER
        ))?;

        nx_checked_call!(_nx_tcp_client_socket_unbind(socket.socket_ptr))?;
        // Put socket back for reuse
        self.socket = Some(NetxTcpSocket {
            socket_ptr: socket.socket_ptr,
        });
        Ok(())
    }
}
