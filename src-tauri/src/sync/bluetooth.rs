//! Bluetooth for phone sync, for a phone that shares no network with this
//! PC: an RFCOMM listener registered in SDP under `protocol::BT_SERVICE`.
//! A phone that comes in on it gets the same TLS session as one over TCP;
//! Bluetooth is only the wire.

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{bail, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};

/// `protocol::BT_SERVICE` as a number.
#[cfg_attr(not(windows), allow(dead_code))]
const SERVICE: u128 = 0xd33e3e4c_9cb7_4638_b68d_813565eee1a7;

/// A Bluetooth address as the phone wants it: "AA:BB:CC:DD:EE:FF".
pub fn mac(address: u64) -> String {
    (0..6)
        .rev()
        .map(|i| format!("{:02X}", (address >> (i * 8)) & 0xff))
        .collect::<Vec<_>>()
        .join(":")
}

/// Joins a Bluetooth connection to the async world: what arrives is read
/// from the returned stream, and what's written to it is sent. Two threads
/// carry the bytes; both end, and the socket closes, when either side does.
pub fn bridge(conn: Conn) -> DuplexStream {
    let (ours, theirs) = tokio::io::duplex(64 * 1024);
    let (mut from_session, mut to_session) = tokio::io::split(theirs);
    let conn = Arc::new(conn);
    let incoming = conn.clone();
    std::thread::spawn(move || {
        let mut buf = vec![0u8; 16 * 1024];
        loop {
            match incoming.recv(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let wrote = tauri::async_runtime::block_on(to_session.write_all(&buf[..n]));
                    if wrote.is_err() {
                        break;
                    }
                }
            }
        }
        let _ = tauri::async_runtime::block_on(to_session.shutdown());
        incoming.shutdown();
    });
    std::thread::spawn(move || {
        let mut buf = vec![0u8; 16 * 1024];
        loop {
            match tauri::async_runtime::block_on(from_session.read(&mut buf)) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if conn.send_all(&buf[..n]).is_err() {
                        break;
                    }
                }
            }
        }
        conn.shutdown();
    });
    ours
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::mem::size_of;
    use windows::core::{GUID, PWSTR};
    use windows::Win32::Devices::Bluetooth::{
        BluetoothFindFirstRadio, BluetoothFindRadioClose, BluetoothGetRadioInfo, AF_BTH,
        BLUETOOTH_FIND_RADIO_PARAMS, BLUETOOTH_RADIO_INFO, BTHPROTO_RFCOMM, NS_BTH, SOCKADDR_BTH,
    };
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Networking::WinSock::{
        accept, bind, closesocket, getsockname, listen, recv, send, shutdown, socket,
        WSAGetLastError, WSASetServiceW, WSAStartup, CSADDR_INFO, RNRSERVICE_DELETE,
        RNRSERVICE_REGISTER, SD_BOTH, SEND_RECV_FLAGS, SOCKADDR, SOCKET, SOCKET_ADDRESS,
        SOCK_STREAM, WSADATA, WSAQUERYSETW,
    };

    /// Any free RFCOMM channel.
    const BT_PORT_ANY: u32 = u32::MAX;

    fn last_error() -> io::Error {
        io::Error::from_raw_os_error(unsafe { WSAGetLastError() }.0)
    }

    /// This PC's Bluetooth address, when it has a radio.
    pub fn radio() -> Option<String> {
        let params = BLUETOOTH_FIND_RADIO_PARAMS {
            dwSize: size_of::<BLUETOOTH_FIND_RADIO_PARAMS>() as u32,
        };
        let mut handle = HANDLE::default();
        // SAFETY: plain calls with locals that outlive them; the handles are
        // closed below.
        unsafe {
            let find = BluetoothFindFirstRadio(&params, &mut handle).ok()?;
            let _ = BluetoothFindRadioClose(find);
            let mut info = BLUETOOTH_RADIO_INFO {
                dwSize: size_of::<BLUETOOTH_RADIO_INFO>() as u32,
                ..Default::default()
            };
            let result = BluetoothGetRadioInfo(handle, &mut info);
            let _ = CloseHandle(handle);
            if result != 0 {
                return None;
            }
            Some(mac(info.address.Anonymous.ullLong))
        }
    }

    /// One phone's RFCOMM connection.
    pub struct Conn(SOCKET);

    impl Conn {
        pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
            // SAFETY: the socket is open until drop; recv writes into buf.
            let n = unsafe { recv(self.0, buf, SEND_RECV_FLAGS(0)) };
            if n < 0 {
                Err(last_error())
            } else {
                Ok(n as usize)
            }
        }

        pub fn send_all(&self, mut buf: &[u8]) -> io::Result<()> {
            while !buf.is_empty() {
                // SAFETY: as above.
                let n = unsafe { send(self.0, buf, SEND_RECV_FLAGS(0)) };
                if n <= 0 {
                    return Err(last_error());
                }
                buf = &buf[n as usize..];
            }
            Ok(())
        }

        /// Ends both directions, which wakes a thread blocked in recv.
        pub fn shutdown(&self) {
            unsafe {
                let _ = shutdown(self.0, SD_BOTH);
            }
        }
    }

    impl Drop for Conn {
        fn drop(&mut self) {
            unsafe {
                let _ = closesocket(self.0);
            }
        }
    }

    /// The listening socket and its SDP record, which phones find by
    /// `SERVICE`. Closing it removes the record and ends `accept`.
    pub struct Listener {
        socket: SOCKET,
        addr: SOCKADDR_BTH,
        closed: AtomicBool,
        /// This PC's Bluetooth address, for the pairing link.
        pub address: String,
    }

    impl Listener {
        pub fn open() -> Result<Self> {
            let Some(address) = radio() else {
                bail!("no Bluetooth radio");
            };
            // SAFETY: Winsock calls on a socket this function owns; on any
            // failure it's closed before returning.
            unsafe {
                let mut data = WSADATA::default();
                WSAStartup(0x0202, &mut data);
                let socket = socket(AF_BTH as i32, SOCK_STREAM, BTHPROTO_RFCOMM as i32)?;
                let fail = |what: &str| {
                    let e = last_error();
                    let _ = closesocket(socket);
                    anyhow::anyhow!("{what}: {e}")
                };
                let mut addr = SOCKADDR_BTH {
                    addressFamily: AF_BTH,
                    btAddr: 0,
                    serviceClassId: GUID::zeroed(),
                    port: BT_PORT_ANY,
                };
                let mut len = size_of::<SOCKADDR_BTH>() as i32;
                if bind(socket, &addr as *const _ as *const SOCKADDR, len) != 0 {
                    return Err(fail("bind"));
                }
                if getsockname(socket, &mut addr as *mut _ as *mut SOCKADDR, &mut len) != 0 {
                    return Err(fail("getsockname"));
                }
                if listen(socket, 2) != 0 {
                    return Err(fail("listen"));
                }
                let listener = Self {
                    socket,
                    addr,
                    closed: AtomicBool::new(false),
                    address,
                };
                if let Err(e) = listener.record(true) {
                    let _ = closesocket(socket);
                    return Err(e);
                }
                Ok(listener)
            }
        }

        /// Adds or removes the SDP record that points phones at this socket.
        fn record(&self, add: bool) -> Result<()> {
            let mut service = GUID::from_u128(SERVICE);
            let mut name: Vec<u16> = "Legato sync\0".encode_utf16().collect();
            let mut addr = self.addr;
            let mut cs = CSADDR_INFO {
                LocalAddr: SOCKET_ADDRESS {
                    lpSockaddr: &mut addr as *mut _ as *mut SOCKADDR,
                    iSockaddrLength: size_of::<SOCKADDR_BTH>() as i32,
                },
                RemoteAddr: SOCKET_ADDRESS::default(),
                iSocketType: SOCK_STREAM.0,
                iProtocol: BTHPROTO_RFCOMM as i32,
            };
            let set = WSAQUERYSETW {
                dwSize: size_of::<WSAQUERYSETW>() as u32,
                lpszServiceInstanceName: PWSTR(name.as_mut_ptr()),
                lpServiceClassId: &mut service,
                dwNameSpace: NS_BTH,
                dwNumberOfCsAddrs: 1,
                lpcsaBuffer: &mut cs,
                ..Default::default()
            };
            let op = if add {
                RNRSERVICE_REGISTER
            } else {
                RNRSERVICE_DELETE
            };
            // SAFETY: every pointer in `set` points at a local that outlives
            // the call.
            if unsafe { WSASetServiceW(&set, op, 0) } != 0 {
                bail!("registering the Bluetooth service: {}", last_error());
            }
            Ok(())
        }

        /// Waits for a phone. Fails once the listener is closed.
        pub fn accept(&self) -> Result<Conn> {
            // SAFETY: the listening socket is open until `close`.
            let conn = unsafe { accept(self.socket, None, None) };
            match conn {
                Ok(socket) => Ok(Conn(socket)),
                Err(e) => bail!("accept: {e}"),
            }
        }

        pub fn is_closed(&self) -> bool {
            self.closed.load(Ordering::SeqCst)
        }

        pub fn close(&self) {
            if self.closed.swap(true, Ordering::SeqCst) {
                return;
            }
            let _ = self.record(false);
            unsafe {
                let _ = closesocket(self.socket);
            }
        }
    }

    impl Drop for Listener {
        fn drop(&mut self) {
            self.close();
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    pub struct Conn;

    impl Conn {
        pub fn recv(&self, _buf: &mut [u8]) -> io::Result<usize> {
            Ok(0)
        }
        pub fn send_all(&self, _buf: &[u8]) -> io::Result<()> {
            Ok(())
        }
        pub fn shutdown(&self) {}
    }

    pub struct Listener {
        closed: AtomicBool,
        pub address: String,
    }

    impl Listener {
        pub fn open() -> Result<Self> {
            bail!("Bluetooth sync needs Windows")
        }
        pub fn accept(&self) -> Result<Conn> {
            bail!("closed")
        }
        pub fn is_closed(&self) -> bool {
            self.closed.load(Ordering::SeqCst)
        }
        pub fn close(&self) {
            self.closed.store(true, Ordering::SeqCst);
        }
    }
}

pub use imp::{Conn, Listener};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_read_as_the_phone_wants_them() {
        assert_eq!(mac(0x0011_22aa_bbcc), "00:11:22:AA:BB:CC");
        assert_eq!(mac(0), "00:00:00:00:00:00");
    }

    #[test]
    fn the_service_number_is_the_protocols_uuid() {
        let text = super::super::protocol::BT_SERVICE.replace('-', "");
        assert_eq!(u128::from_str_radix(&text, 16).unwrap(), SERVICE);
    }
}
