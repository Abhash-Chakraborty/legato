//! The addresses a phone can reach this PC on: the home network first, then
//! Netbird.

use std::net::Ipv4Addr;

use super::hub::address_kind;

/// Every usable private IPv4 address of an adapter that's up, with its
/// kind ("lan" or "netbird"), home network first.
pub fn addresses() -> Vec<(&'static str, Ipv4Addr)> {
    let mut found: Vec<(&'static str, Ipv4Addr)> = adapter_addresses()
        .into_iter()
        .filter_map(|ip| Some((address_kind(ip)?, ip)))
        // A public address isn't offered: the listener turns away anyone
        // outside the local network and VPNs, and the phone refuses a link
        // that names one.
        .filter(|(kind, ip)| *kind == "netbird" || ip.is_private())
        .collect();
    found.sort_by_key(|(kind, ip)| (*kind != "lan", *ip));
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
                let name = a.FriendlyName.to_string().unwrap_or_default();
                let description = a.Description.to_string().unwrap_or_default();
                if a.OperStatus == IfOperStatusUp && !virtual_adapter(&name, &description) {
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

/// Adapters for virtual machines and containers on this PC (Hyper-V and
/// WSL's vEthernet, VMware, VirtualBox, Docker): private addresses no phone
/// can reach, so they stay out of the pairing link.
#[cfg_attr(not(windows), allow(dead_code))]
fn virtual_adapter(name: &str, description: &str) -> bool {
    const MARKS: [&str; 7] = [
        "vethernet",
        "hyper-v",
        "wsl",
        "vmware",
        "virtualbox",
        "docker",
        "loopback",
    ];
    let both = format!("{name} {description}").to_lowercase();
    MARKS.iter().any(|m| both.contains(m))
}

#[cfg(test)]
mod tests {
    use super::virtual_adapter;

    #[test]
    fn virtual_machine_adapters_are_left_out() {
        assert!(virtual_adapter(
            "vEthernet (WSL (Hyper-V firewall))",
            "Hyper-V Virtual Ethernet Adapter"
        ));
        assert!(virtual_adapter(
            "VMware Network Adapter VMnet8",
            "VMware Virtual Ethernet Adapter for VMnet8"
        ));
        assert!(virtual_adapter(
            "Ethernet 3",
            "VirtualBox Host-Only Ethernet Adapter"
        ));
        assert!(!virtual_adapter("Wi-Fi", "Intel(R) Wi-Fi 6E AX211 160MHz"));
        assert!(!virtual_adapter("wt0", "WireGuard Tunnel"));
        assert!(!virtual_adapter(
            "Ethernet",
            "Realtek PCIe GbE Family Controller"
        ));
    }
}
