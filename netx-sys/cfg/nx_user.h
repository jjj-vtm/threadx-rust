/* 
 * Copyright (c) Microsoft
 * Copyright (c) 2024 Eclipse Foundation
 * 
 *  This program and the accompanying materials are made available 
 *  under the terms of the MIT license which is available at
 *  https://opensource.org/license/mit.
 * 
 *  SPDX-License-Identifier: MIT
 * 
 *  Contributors: 
 *     Microsoft         - Initial version
 *     Frédéric Desbiens - 2024 version.
 */

#ifndef NX_USER_H
#define NX_USER_H

//#define NX_DISABLE_FRAGMENTATION
//#define NX_DISABLE_PACKET_CHAIN
#define NX_LITTLE_ENDIAN
#define NX_SECURE_ENABLE
#define NX_ENABLE_EXTENDED_NOTIFY_SUPPORT
#define NX_ENABLE_IP_PACKET_FILTER
#define NX_DISABLE_IPV6
#define NX_DNS_CLIENT_USER_CREATE_PACKET_POOL
//#define NX_PACKET_ALIGNMENT 16
#define NX_SNTP_CLIENT_MIN_SERVER_STRATUM 3
#define NX_DISABLE_ERROR_CHECKING

#define NX_RAND                         nx_rand16

#define NX_ASSERT_FAIL for(;;){}

/* Symbols for Wiced.  */

/* This define specifies the size of the physical packet header. The default value is 16 (based on
   a typical 16-byte Ethernet header).  */
#define NX_PHYSICAL_HEADER              (14 + 12 + 18)

/* This define specifies the size of the physical packet trailer and is typically used to reserve storage
   for things like Ethernet CRCs, etc.  */
#define NX_PHYSICAL_TRAILER             (0)

#define NX_LINK_PTP_SEND                51      /* Precision Time Protocol */

#endif /* NX_USER_H */
