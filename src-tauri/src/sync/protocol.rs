//! The phone sync wire format: messages, frames and the pairing link. See
//! docs/phone-sync.md, which the Android app implements.

use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const PROTOCOL: u32 = 1;
/// The port the PC listens on, and how many after it to try when it's taken.
pub const PORT: u16 = 47613;
pub const PORT_TRIES: u16 = 10;
pub const MAX_FRAME: usize = 16 * 1024 * 1024;
/// The largest first frame from a peer not yet admitted: a hello is a few
/// hundred bytes, and a stranger mustn't make the PC set aside 16 MiB.
pub const MAX_HELLO: usize = 8 * 1024;
pub const PING_EVERY: Duration = Duration::from_secs(30);
pub const SILENCE_LIMIT: Duration = Duration::from_secs(90);
/// How many items one `changes` answer carries at most.
pub const PULL_LIMIT: usize = 500;
/// A pairing link address that is the PC's Bluetooth radio, as
/// "bt:AA:BB:CC:DD:EE:FF", rather than an IP address and port.
pub const BT_PREFIX: &str = "bt:";
/// The Bluetooth service (RFCOMM, found through SDP) the PC offers sync on.
/// Legato for Android connects to the same UUID.
#[allow(dead_code)] // The PC registers it; the phone has it in Kotlin.
pub const BT_SERVICE: &str = "d33e3e4c-9cb7-4638-b68d-813565eee1a7";

/// Everything that syncs, in the order Settings lists it.
pub const KINDS: [&str; 7] = [
    "dictionary",
    "snippets",
    "history",
    "notes",
    "scratchpad",
    "transforms",
    "style",
];

/// One version of one item. `body` is None for a delete.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Change {
    pub kind: String,
    pub key: String,
    pub body: Option<serde_json::Value>,
    pub changed_at: i64,
    pub origin: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClipKind {
    Text,
    Image,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    pub id: String,
    pub kind: ClipKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Base64 PNG bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub png: Option<String>,
    /// How long ago it was copied, when it was sent.
    pub age_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_app: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hello {
    pub protocol: u32,
    pub device_id: String,
    pub name: String,
    pub platform: String,
    pub app_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Welcome {
    pub protocol: u32,
    pub device_id: String,
    pub name: String,
    pub platform: String,
    pub app_version: String,
    pub kinds: Vec<String>,
    pub clipboard: bool,
    /// Where the PC can be reached now, as in the pairing link; the phone
    /// keeps them for next time. Absent from older PCs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub addrs: Vec<String>,
    /// The PC transcribes recordings the phone sends (`transcribe`) with
    /// its own speech model.
    #[serde(default)]
    pub transcribe: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Refusal {
    UnknownDevice,
    BadSecret,
    Expired,
    Protocol,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    Hello(Hello),
    Welcome(Welcome),
    Refused {
        reason: Refusal,
    },
    Pull {
        after: u64,
        kinds: Vec<String>,
        #[serde(default)]
        limit: Option<usize>,
    },
    Changes {
        items: Vec<Change>,
        upto: u64,
        more: bool,
    },
    Nudge,
    Clip(Clip),
    Ping,
    Pong,
    Bye {
        reason: String,
    },
    /// A recording for the PC's speech model: 16 kHz mono, as 16-bit
    /// little-endian samples in base64 (`pcm_encode`).
    Transcribe {
        id: String,
        pcm: String,
        /// The kind of app the text is for ("personal", "email"…), so the PC
        /// styles it as it would there; "other" when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        app: Option<String>,
    },
    /// The PC's answer to `transcribe`: the text, or why there is none.
    Transcript {
        id: String,
        #[serde(default)]
        text: Option<String>,
        #[serde(default)]
        error: Option<String>,
    },
    /// A type this version doesn't know; ignored after the handshake.
    #[serde(other)]
    Unknown,
}

pub async fn write<W: AsyncWrite + Unpin>(w: &mut W, message: &Message) -> Result<()> {
    let bytes = serde_json::to_vec(message)?;
    if bytes.len() > MAX_FRAME {
        bail!("message too large ({} bytes)", bytes.len());
    }
    w.write_all(&(bytes.len() as u32).to_be_bytes()).await?;
    w.write_all(&bytes).await?;
    w.flush().await?;
    Ok(())
}

pub async fn read<R: AsyncRead + Unpin>(r: &mut R) -> Result<Message> {
    read_limited(r, MAX_FRAME).await
}

/// Reads one frame of at most `max` bytes. Memory grows with the bytes that
/// actually arrive, not with the length the peer claims.
pub async fn read_limited<R: AsyncRead + Unpin>(r: &mut R, max: usize) -> Result<Message> {
    let mut len = [0u8; 4];
    r.read_exact(&mut len).await?;
    let len = u32::from_be_bytes(len) as usize;
    if len > max {
        bail!("frame too large ({len} bytes)");
    }
    let mut bytes = Vec::with_capacity(len.min(64 * 1024));
    (&mut *r).take(len as u64).read_to_end(&mut bytes).await?;
    if bytes.len() != len {
        bail!("frame cut short");
    }
    Ok(serde_json::from_slice(&bytes)?)
}

/// Whether a phone may connect from this address: this PC, the LAN, a
/// link-local address, or a NetBird or Tailscale one. Anyone else is
/// dropped before TLS, so a stranger on the internet gets nothing.
pub fn local_peer(ip: std::net::IpAddr) -> bool {
    use std::net::IpAddr;
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || (o[0] == 100 && (o[1] & 0xc0) == 64)
        }
        IpAddr::V6(v6) => {
            let first = v6.segments()[0];
            v6.is_loopback()
                // Unique local (fc00::/7, Tailscale's fd7a:… among them) and
                // link-local (fe80::/10).
                || (first & 0xfe00) == 0xfc00
                || (first & 0xffc0) == 0xfe80
                || v6.to_ipv4_mapped().is_some_and(|v4| local_peer(IpAddr::V4(v4)))
        }
    }
}

/// Samples (-1..1, 16 kHz mono) as `transcribe` carries them.
#[allow(dead_code)] // The phone sends; the PC only reads.
pub fn pcm_encode(samples: &[f32]) -> String {
    use base64::Engine as _;
    let bytes: Vec<u8> = samples
        .iter()
        .flat_map(|s| ((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())
        .collect();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

pub fn pcm_decode(pcm: &str) -> Result<Vec<f32>> {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD.decode(pcm)?;
    Ok(bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| f32::from(i16::from_le_bytes(*b)) / 32767.0)
        .collect())
}

/// What the PC's QR code holds.
#[derive(Clone, Debug, PartialEq)]
pub struct PairLink {
    pub device_id: String,
    pub name: String,
    pub fingerprint: String,
    pub addrs: Vec<String>,
    pub secret: String,
}

impl PairLink {
    pub fn to_uri(&self) -> String {
        format!(
            "legato://pair?v=1&id={}&name={}&fp={}&addr={}&secret={}",
            encode(&self.device_id),
            encode(&self.name),
            encode(&self.fingerprint),
            encode(&self.addrs.join(",")),
            encode(&self.secret),
        )
    }

    #[allow(dead_code)] // The phone reads the link; see phone/core.
    pub fn parse(uri: &str) -> Result<Self> {
        let query = uri
            .trim()
            .strip_prefix("legato://pair?")
            .ok_or_else(|| anyhow!("not a Legato pairing link"))?;
        let get = |name: &str| -> Result<String> {
            query
                .split('&')
                .find_map(|pair| pair.strip_prefix(name)?.strip_prefix('='))
                .map(decode)
                .ok_or_else(|| anyhow!("the pairing link has no {name}"))
        };
        if get("v")? != "1" {
            bail!("this pairing link is from a newer Legato");
        }
        Ok(Self {
            device_id: get("id")?,
            name: get("name")?,
            fingerprint: get("fp")?,
            addrs: get("addr")?
                .split(',')
                .filter(|a| !a.is_empty())
                .map(str::to_string)
                .collect(),
            secret: get("secret")?,
        })
    }
}

fn encode(value: &str) -> String {
    let mut out = String::new();
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b':' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[allow(dead_code)]
fn decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = || std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok();
        match (bytes[i], hex().and_then(|h| u8::from_str_radix(h, 16).ok())) {
            (b'%', Some(b)) => {
                out.push(b);
                i += 3;
            }
            (b'+', _) => {
                out.push(b' ');
                i += 1;
            }
            (b, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pairing_link_reads_back_what_it_holds() {
        let link = PairLink {
            device_id: "6f1c".into(),
            name: "Abhash's desk PC & co".into(),
            fingerprint: "q-_x".into(),
            addrs: vec!["192.168.1.20:47613".into(), "100.97.188.40:47613".into()],
            secret: "s3cr3t".into(),
        };
        let uri = link.to_uri();
        assert!(uri.starts_with("legato://pair?v=1&"));
        assert!(!uri.contains(' '));
        assert_eq!(PairLink::parse(&uri).unwrap(), link);
        assert!(PairLink::parse("https://example.com").is_err());
        assert!(PairLink::parse(&uri.replace("v=1", "v=2")).is_err());
    }

    #[test]
    fn messages_carry_their_type_and_unknown_ones_still_parse() {
        let json = serde_json::to_string(&Message::Nudge).unwrap();
        assert_eq!(json, r#"{"type":"nudge"}"#);
        let pull: Message =
            serde_json::from_str(r#"{"type":"pull","after":3,"kinds":["notes"]}"#).unwrap();
        assert_eq!(
            pull,
            Message::Pull {
                after: 3,
                kinds: vec!["notes".into()],
                limit: None
            }
        );
        let later: Message = serde_json::from_str(r#"{"type":"presence","x":1}"#).unwrap();
        assert_eq!(later, Message::Unknown);
    }

    #[test]
    fn recordings_survive_the_trip() {
        let samples = [0.0, 0.5, -0.5, 1.0, -1.0, 2.0];
        let back = pcm_decode(&pcm_encode(&samples)).unwrap();
        assert_eq!(back.len(), samples.len());
        for (a, b) in samples.iter().zip(&back) {
            assert!((a.clamp(-1.0, 1.0) - b).abs() < 0.001);
        }
    }

    #[tokio::test]
    async fn frames_round_trip_and_oversized_ones_are_refused() {
        let (mut a, mut b) = tokio::io::duplex(1024);
        write(&mut a, &Message::Ping).await.unwrap();
        assert_eq!(read(&mut b).await.unwrap(), Message::Ping);
        a.write_all(&(MAX_FRAME as u32 + 1).to_be_bytes())
            .await
            .unwrap();
        assert!(read(&mut b).await.is_err());
    }

    #[tokio::test]
    async fn a_first_frame_over_the_hello_limit_is_refused() {
        let (mut a, mut b) = tokio::io::duplex(1024);
        a.write_all(&(MAX_HELLO as u32 + 1).to_be_bytes())
            .await
            .unwrap();
        assert!(read_limited(&mut b, MAX_HELLO).await.is_err());
        // A frame cut short is an error, not a hang or a short message.
        let (mut a, mut b) = tokio::io::duplex(1024);
        a.write_all(&100u32.to_be_bytes()).await.unwrap();
        a.write_all(b"{}").await.unwrap();
        drop(a);
        assert!(read_limited(&mut b, MAX_HELLO).await.is_err());
    }

    #[test]
    fn only_local_and_vpn_peers_may_connect() {
        let ok = |s: &str| local_peer(s.parse().unwrap());
        for addr in [
            "127.0.0.1",
            "192.168.1.20",
            "10.1.2.3",
            "172.16.0.9",
            "169.254.3.4",
            "100.97.188.40",
            "::1",
            "fd7a:115c:a1e0::9",
            "fe80::1",
            "::ffff:192.168.0.2",
        ] {
            assert!(ok(addr), "{addr}");
        }
        for addr in [
            "8.8.8.8",
            "100.128.0.1",
            "203.0.113.5",
            "2001:db8::1",
            "::ffff:1.1.1.1",
        ] {
            assert!(!ok(addr), "{addr}");
        }
    }
}
