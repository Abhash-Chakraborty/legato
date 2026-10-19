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
pub const PING_EVERY: Duration = Duration::from_secs(30);
pub const SILENCE_LIMIT: Duration = Duration::from_secs(90);
/// How many items one `changes` answer carries at most.
pub const PULL_LIMIT: usize = 500;

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
    let mut len = [0u8; 4];
    r.read_exact(&mut len).await?;
    let len = u32::from_be_bytes(len) as usize;
    if len > MAX_FRAME {
        bail!("frame too large ({len} bytes)");
    }
    let mut bytes = vec![0u8; len];
    r.read_exact(&mut bytes).await?;
    Ok(serde_json::from_slice(&bytes)?)
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

    #[allow(dead_code)] // The phone reads the link; see tools/stand-in-phone.
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
}
