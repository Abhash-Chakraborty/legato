//! The PC's side of phone sync without the app around it: who may connect,
//! pairing with a one-time secret, and handing sessions to the engine. The
//! app (sync/mod.rs) supplies the settings and hears what happened through
//! `Events`.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::Result;

use super::engine::{now_s, Accepted, Engine};
use super::identity::{same_secret, secret};
use super::ledger::{self, Peer};
use super::link::{Host, TranscribeJob, Via};
use super::protocol::{Change, Clip, Hello, Refusal, Welcome, PROTOCOL};

/// How long a pairing code works, and how many wrong tries cancel it.
pub const PAIRING_FOR: Duration = Duration::from_secs(10 * 60);
pub const PAIRING_TRIES: u32 = 5;

/// How a phone reaches an address: "netbird" for the 100.64.0.0/10 overlay
/// range, "lan" for the rest; None for addresses a phone can't use.
pub fn address_kind(ip: std::net::Ipv4Addr) -> Option<&'static str> {
    let [a, b, ..] = ip.octets();
    if ip.is_loopback() || ip.is_link_local() || ip.is_unspecified() || ip.is_multicast() {
        None
    } else if a == 100 && (64..128).contains(&b) {
        Some("netbird")
    } else {
        Some("lan")
    }
}

/// What the PC syncs right now.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub name: String,
    pub app_version: String,
    pub kinds: Vec<String>,
    pub clipboard: bool,
}

/// What the app hears about. All optional.
pub trait Events: Send + Sync {
    fn paired(&self, _peer: &Peer) {}
    fn online(&self, _device_id: &str, _via: Option<Via>) {}
    fn synced(&self, _device_id: &str, _accepted: &Accepted) {}
    fn clip(&self, _peer: &Peer, _clip: Clip) {}
    fn forgotten(&self, _device_id: &str) {}
    /// A pairing attempt failed or the code ran out of tries.
    fn pairing_failed(&self, _reason: Refusal, _closed: bool) {}
    /// How to transcribe a phone's recording with this PC's model, for an
    /// app of the kind `app`.
    fn transcribe(
        &self,
        _device_id: &str,
        _samples: Vec<f32>,
        _app: Option<String>,
    ) -> Option<TranscribeJob> {
        None
    }
}

struct Pairing {
    secret: String,
    until: Instant,
    wrong: u32,
}

pub struct Hub {
    pub engine: Engine,
    config: Mutex<Config>,
    /// Where phones can reach this PC now, for `welcome`.
    addrs: Mutex<Vec<String>>,
    pairing: Mutex<Option<Pairing>>,
    events: Box<dyn Events>,
}

impl Hub {
    pub fn new(engine: Engine, config: Config, events: Box<dyn Events>) -> Self {
        Self {
            engine,
            config: Mutex::new(config),
            addrs: Mutex::new(Vec::new()),
            pairing: Mutex::new(None),
            events,
        }
    }

    pub fn config(&self) -> Config {
        self.config
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn set_config(&self, config: Config) {
        *self.config.lock().unwrap_or_else(|e| e.into_inner()) = config;
    }

    /// Where phones can reach this PC now (network addresses and
    /// Bluetooth), sent in every `welcome`.
    pub fn set_addrs(&self, addrs: Vec<String>) {
        *self.addrs.lock().unwrap_or_else(|e| e.into_inner()) = addrs;
    }

    fn pairing(&self) -> std::sync::MutexGuard<'_, Option<Pairing>> {
        self.pairing.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Opens pairing with a fresh one-time secret, replacing any open one.
    pub fn open_pairing(&self) -> String {
        let secret = secret();
        *self.pairing() = Some(Pairing {
            secret: secret.clone(),
            until: Instant::now() + PAIRING_FOR,
            wrong: 0,
        });
        secret
    }

    pub fn close_pairing(&self) {
        *self.pairing() = None;
    }

    pub fn pairing_open(&self) -> bool {
        self.pairing()
            .as_ref()
            .is_some_and(|p| p.until > Instant::now())
    }

    pub fn peers(&self) -> Result<Vec<Peer>> {
        ledger::peers(&self.engine.conn()?)
    }

    fn peer(&self, device_id: &str) -> Result<Option<Peer>> {
        ledger::peer(&self.engine.conn()?, device_id)
    }

    /// Checks a pairing secret, counting wrong ones.
    fn check_secret(&self, given: &str) -> std::result::Result<(), Refusal> {
        let mut pairing = self.pairing();
        let Some(open) = pairing.as_mut().filter(|p| p.until > Instant::now()) else {
            *pairing = None;
            return Err(Refusal::Expired);
        };
        if same_secret(&open.secret, given) {
            *pairing = None;
            return Ok(());
        }
        open.wrong += 1;
        let closed = open.wrong >= PAIRING_TRIES;
        if closed {
            *pairing = None;
        }
        drop(pairing);
        self.events.pairing_failed(Refusal::BadSecret, closed);
        Err(Refusal::BadSecret)
    }
}

impl Host for Hub {
    fn describe(&self) -> Welcome {
        let config = self.config();
        Welcome {
            protocol: PROTOCOL,
            device_id: self.engine.me.clone(),
            name: config.name,
            platform: "windows".into(),
            app_version: config.app_version,
            kinds: config.kinds,
            clipboard: config.clipboard,
            addrs: self.addrs.lock().unwrap_or_else(|e| e.into_inner()).clone(),
            transcribe: true,
        }
    }

    fn admit(&self, hello: &Hello, fingerprint: &str) -> std::result::Result<(), Refusal> {
        let known = self.peer(&hello.device_id).ok().flatten();
        let trusted = known
            .as_ref()
            .is_some_and(|p| same_secret(&p.fingerprint, fingerprint));
        if !trusted {
            let Some(given) = hello.secret.as_deref() else {
                return Err(Refusal::UnknownDevice);
            };
            self.check_secret(given)?;
            let peer = Peer {
                device_id: hello.device_id.clone(),
                name: hello.name.chars().take(40).collect(),
                platform: hello.platform.clone(),
                app_version: hello.app_version.clone(),
                fingerprint: fingerprint.to_string(),
                paired_at: now_s(),
                last_seen: Some(now_s()),
                last_sync: None,
                sent_upto: 0,
                recv_upto: 0,
            };
            let saved = self
                .engine
                .conn()
                .and_then(|conn| ledger::add_peer(&conn, &peer));
            if let Err(e) = saved {
                log::error!("Couldn't save the paired phone: {e:#}");
                return Err(Refusal::Expired);
            }
            self.events.paired(&peer);
        } else if let Ok(conn) = self.engine.conn() {
            let _ = ledger::update_peer(&conn, &hello.device_id, &hello.app_version, now_s());
        }
        // The first pull gets what changed while it was away.
        if let Err(e) = self.engine.refresh(&self.config().kinds) {
            log::warn!("Couldn't look for changes before syncing: {e:#}");
        }
        Ok(())
    }

    fn changes(
        &self,
        peer: &str,
        after: u64,
        kinds: &[String],
        limit: usize,
    ) -> Result<(Vec<Change>, u64, bool)> {
        // A device unpaired mid-session gets nothing more; the error ends
        // its session.
        if self.peer(peer)?.is_none() {
            return Err(anyhow::anyhow!("{peer} isn't paired"));
        }
        self.engine
            .changes(peer, after, kinds, &self.config().kinds, limit)
    }

    fn accept(&self, peer: &str, items: Vec<Change>, upto: u64) -> Result<()> {
        let Some(known) = self.peer(peer)? else {
            return Ok(());
        };
        let accepted = self
            .engine
            .accept(&known, items, upto, &self.config().kinds)?;
        self.events.synced(peer, &accepted);
        Ok(())
    }

    fn recv_upto(&self, peer: &str) -> u64 {
        self.peer(peer).ok().flatten().map_or(0, |p| p.recv_upto)
    }

    fn wanted(&self, _peer: &str) -> Vec<String> {
        self.config().kinds
    }

    fn clip(&self, peer: &str, clip: Clip) {
        if !self.config().clipboard {
            return;
        }
        if let Ok(Some(known)) = self.peer(peer) {
            self.events.clip(&known, clip);
        }
    }

    fn online(&self, peer: &str, via: Option<Via>) {
        self.events.online(peer, via);
    }

    fn transcribe(
        &self,
        peer: &str,
        samples: Vec<f32>,
        app: Option<String>,
    ) -> Option<TranscribeJob> {
        // Only for a paired phone.
        self.peer(peer).ok().flatten()?;
        self.events.transcribe(peer, samples, app)
    }

    fn forgotten(&self, peer: &str) {
        if let Ok(conn) = self.engine.conn() {
            let _ = ledger::remove_peer(&conn, peer);
        }
        self.events.forgotten(peer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_are_home_network_or_netbird() {
        let kind = |s: &str| address_kind(s.parse().unwrap());
        assert_eq!(kind("192.168.1.20"), Some("lan"));
        assert_eq!(kind("10.0.0.4"), Some("lan"));
        assert_eq!(kind("100.97.188.40"), Some("netbird"));
        assert_eq!(kind("100.63.0.1"), Some("lan"));
        assert_eq!(kind("100.128.0.1"), Some("lan"));
        assert_eq!(kind("127.0.0.1"), None);
        assert_eq!(kind("169.254.3.3"), None);
    }
}
