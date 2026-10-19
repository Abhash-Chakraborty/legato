//! The addresses a phone can reach this PC on: the home network first, then
//! Netbird.

use std::net::Ipv4Addr;

use super::hub::address_kind;

/// Every usable IPv4 address of an adapter that's up, with its kind
/// ("lan" or "netbird"), home network first.
pub fn addresses() -> Vec<(&'static str, Ipv4Addr)> {
    let mut found: Vec<(&'static str, Ipv4Addr)> = adapter_addresses()
        .into_iter()
        .filter_map(|ip| Some((address_kind(ip)?, ip)))
        .collect();
    found.sort_by_key(|(kind, ip)| (*kind != "lan", !ip.is_private(), *ip));
    found.dedup();
    found
}

#[cfg(windows)]
fn adapter_addresses() -> Vec<Ipv4Addr> {
    use windows::Win32::NetworkManagement::IpHelper::{
        GetAdaptersAddresses, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER,
        GAA_FLAG_SKIP_MULTICAST, IP_ADAPTER_ADDRESSES_LH,
    };
    use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    use windows::Win32::Networking::WinSock::{AF_INET, SOCKADDR_IN};

    let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
    let mut size: u32 = 16 * 1024;
    let mut out = Vec::new();
    for _ in 0..3 {
        // u64s keep the buffer aligned for the structs inside it.
        let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
        let first = buffer.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH;
        // SAFETY: the buffer is `size` bytes; the call writes a linked list
        // inside it, read below while the buffer lives.
        let result =
            unsafe { GetAdaptersAddresses(AF_INET.0 as u32, flags, None, Some(first), &mut size) };
        const ERROR_BUFFER_OVERFLOW: u32 = 111;
        if result == ERROR_BUFFER_OVERFLOW {
            continue;
        }
        if result != 0 {
            log::warn!("Couldn't list network adapters: error {result}");
            return out;
        }
        let mut adapter = first as *const IP_ADAPTER_ADDRESSES_LH;
        unsafe {
            while let Some(a) = adapter.as_ref() {
                if a.OperStatus == IfOperStatusUp {
                    let mut unicast = a.FirstUnicastAddress;
                    while let Some(u) = unicast.as_ref() {
                        let sockaddr = u.Address.lpSockaddr;
                        if !sockaddr.is_null() && (*sockaddr).sa_family == AF_INET {
                            let v4 = &*(sockaddr as *const SOCKADDR_IN);
                            out.push(Ipv4Addr::from(u32::from_be(v4.sin_addr.S_un.S_addr)));
                        }
                        unicast = u.Next;
                    }
                }
                adapter = a.Next;
            }
        }
        return out;
    }
    out
}

#[cfg(not(windows))]
fn adapter_addresses() -> Vec<Ipv4Addr> {
    Vec::new()
}
