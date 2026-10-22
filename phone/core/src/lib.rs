//! A phone for testing Legato's phone sync before the Android app exists.
//! It keeps a database shaped like the PC's and runs the app's own sync
//! modules, so pairing, syncing and the shared clipboard can be tried
//! against a real Legato over Wi-Fi or Netbird.

pub mod audio_toolkit;
pub mod finish;
pub mod managers;
pub mod sync;

// The app's pure text rules, compiled here as they are in the app.
#[path = "../../../src-tauri/src/cleanup_rules.rs"]
pub mod cleanup_rules;
#[path = "../../../src-tauri/src/learn_rules.rs"]
pub mod learn_rules;
#[path = "../../../src-tauri/src/snippet_rules.rs"]
pub mod snippet_rules;
#[path = "../../../src-tauri/src/style_rules.rs"]
pub mod style_rules;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use rusqlite_migration::Migrations;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use sync::engine::{now_s, Engine, Extra};
use sync::identity::Identity;
use sync::ledger::{self, Item, Peer};
use sync::link::{self, Dialer, Host, Sessions, Via};
use sync::protocol::{Change, Clip, Hello, Refusal, Welcome, KINDS, PROTOCOL};
use tokio::sync::watch;

/// The PC this phone is paired with, as the pairing link gave it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PairedPc {
    pub device_id: String,
    pub name: String,
    pub fingerprint: String,
    pub addrs: Vec<String>,
}

pub struct Phone {
    pub engine: Engine,
    pub identity: Identity,
    pub name: String,
    dir: PathBuf,
    /// What the PC said it syncs.
    pc_kinds: Mutex<Vec<String>>,
    pub clips: Mutex<Vec<Clip>>,
    /// Hears each clip the PC sends, as it arrives.
    pub on_clip: Mutex<Option<Box<dyn Fn(&Clip) + Send + Sync>>>,
    /// Connected to the PC right now.
    online: AtomicBool,
    /// How: "lan", "netbird" or "bluetooth".
    via: Mutex<Option<&'static str>>,
    /// The app's way to open Bluetooth to the PC, when it has one.
    pub bluetooth: Mutex<Option<Dialer>>,
    /// The PC said it transcribes recordings for the phone.
    pc_transcribes: AtomicBool,
    /// How often a link over Bluetooth looks for the PC on the network again.
    pub network_check_ms: std::sync::atomic::AtomicU64,
    /// Recordings sent to the PC, waiting for their text, by request id.
    waiting: Mutex<HashMap<String, std::sync::mpsc::Sender<Result<String, String>>>>,
    say: Box<dyn Fn(String) + Send + Sync>,
    /// Cuts the wait before the next try short (the app came to the front,
    /// the network changed).
    wake: tokio::sync::Notify,
}

/// How long to wait before the next try after `failures` failed ones: from
/// 2 s, doubling, to 5 minutes, give or take a fifth so phones on one
/// network don't all knock at once. A PC that's off for a weekend costs a
/// few hundred tries, not fifty thousand.
pub fn retry_after(failures: u32, jitter: f64) -> Duration {
    let base = 2u64
        .saturating_mul(1 << failures.saturating_sub(1).min(16))
        .min(300) as f64;
    let jitter = jitter.clamp(0.0, 1.0) * 0.4 - 0.2;
    Duration::from_secs_f64(base * (1.0 + jitter))
}

fn jitter() -> f64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    f64::from(nanos % 1000) / 1000.0
}

/// How a connection attempt ended, for the reconnect loop.
#[derive(Debug, PartialEq, Eq)]
pub enum LinkEnd {
    /// The PC refused us (bad secret, expired link, unknown device); pairing is gone.
    Refused(String),
    /// Stopped on purpose, or the PC unpaired us.
    Stopped,
}

/// Transforms and Style, which a phone keeps in a table of its own.
struct ExtraTable(PathBuf);

impl Extra for ExtraTable {
    fn handles(&self, kind: &str) -> bool {
        kind == "transforms" || kind == "style"
    }

    fn export(&self, kind: &str) -> Result<Vec<Item>> {
        let conn = Connection::open(&self.0)?;
        let mut stmt = conn.prepare("SELECT key, body FROM phone_extra WHERE kind = ?1")?;
        let rows = stmt.query_map([kind], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        let mut items = Vec::new();
        for row in rows {
            let (key, body) = row?;
            items.push(Item {
                key,
                body: serde_json::from_str(&body)?,
                at: None,
            });
        }
        Ok(items)
    }

    fn apply(&self, kind: &str, key: &str, body: Option<&Value>) -> Result<()> {
        let conn = Connection::open(&self.0)?;
        match body {
            Some(body) => conn.execute(
                "INSERT OR REPLACE INTO phone_extra (kind, key, body) VALUES (?1, ?2, ?3)",
                params![kind, key, body.to_string()],
            )?,
            None => conn.execute(
                "DELETE FROM phone_extra WHERE kind = ?1 AND key = ?2",
                params![kind, key],
            )?,
        };
        Ok(())
    }
}

impl Phone {
    /// Opens (or makes) a phone in `dir`. `say` hears what happens.
    pub fn open(dir: &Path, name: &str, say: Box<dyn Fn(String) + Send + Sync>) -> Result<Self> {
        std::fs::create_dir_all(dir)?;
        let identity = Identity::load_or_create(&dir.join("identity.json"))?;
        let db = dir.join("phone.db");
        let mut conn = Connection::open(&db)?;
        Migrations::new(managers::history_store::MIGRATIONS.to_vec())
            .to_latest(&mut conn)
            .context("setting up the phone's database")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS phone_extra (kind TEXT NOT NULL, key TEXT NOT NULL,
                body TEXT NOT NULL, PRIMARY KEY (kind, key));",
        )?;
        Ok(Self {
            engine: Engine {
                db: db.clone(),
                me: identity.device_id.clone(),
                extra: Some(Box::new(ExtraTable(db))),
            },
            identity,
            name: name.to_string(),
            dir: dir.to_path_buf(),
            pc_kinds: Mutex::new(Vec::new()),
            clips: Mutex::new(Vec::new()),
            on_clip: Mutex::new(None),
            online: AtomicBool::new(false),
            via: Mutex::new(None),
            bluetooth: Mutex::new(None),
            pc_transcribes: AtomicBool::new(false),
            network_check_ms: std::sync::atomic::AtomicU64::new(30_000),
            waiting: Mutex::new(HashMap::new()),
            say,
            wake: tokio::sync::Notify::new(),
        })
    }

    pub fn is_online(&self) -> bool {
        self.online.load(Ordering::Relaxed)
    }

    /// Tries the PC again now rather than after the backoff.
    pub fn wake(&self) {
        self.wake.notify_waiters();
    }

    /// Whether the PC transcribes recordings for this phone (while connected).
    pub fn pc_transcribes(&self) -> bool {
        self.is_online() && self.pc_transcribes.load(Ordering::Relaxed)
    }

    /// Sends a recording (16 kHz mono) to the PC and waits for its text,
    /// styled for `app`, a kind of app as Style names it ("personal", "email").
    /// Blocks; Err when not connected, on the PC's error, or after `wait`.
    pub fn transcribe_on_pc(
        &self,
        sessions: &Sessions,
        samples: &[f32],
        wait: Duration,
        app: Option<&str>,
    ) -> Result<String> {
        let pc = self.paired_pc().context("not paired with a PC")?;
        let id = sync::identity::uuid();
        let (tx, rx) = std::sync::mpsc::channel();
        self.waiting.lock().unwrap().insert(id.clone(), tx);
        let message = sync::protocol::Message::Transcribe {
            id: id.clone(),
            pcm: sync::protocol::pcm_encode(samples),
            app: app.map(str::to_string),
        };
        let sent = sessions.send(&pc.device_id, link::Command::Send(message));
        let result = if sent {
            rx.recv_timeout(wait)
                .map_err(|_| anyhow::anyhow!("{} didn't answer in time", pc.name))
                .and_then(|r| r.map_err(|e| anyhow::anyhow!(e)))
        } else {
            Err(anyhow::anyhow!("not connected to {}", pc.name))
        };
        self.waiting.lock().unwrap().remove(&id);
        result
    }

    /// Asks the PC to load its speech model while the user is still talking,
    /// with an empty recording; its answer is ignored. False when not connected.
    pub fn warm_pc(&self, sessions: &Sessions) -> bool {
        let Some(pc) = self.paired_pc() else {
            return false;
        };
        let message = sync::protocol::Message::Transcribe {
            id: sync::identity::uuid(),
            pcm: String::new(),
            app: None,
        };
        sessions.send(&pc.device_id, link::Command::Send(message))
    }

    /// Text from the phone's own model, finished as the PC finishes a
    /// dictation for the app `package` (Dictionary, snippets, cleanup, Style).
    pub fn finish_dictation(&self, raw: &str, package: &str) -> Result<String> {
        let category = finish::category_of(package);
        finish::finish(&self.conn()?, raw, category, chrono::Local::now())
    }

    /// How the phone is connected: "lan", "netbird" or "bluetooth".
    pub fn via(&self) -> Option<&'static str> {
        *self.via.lock().unwrap()
    }

    /// Keeps a connection to the paired PC: connects, and when the link drops
    /// tries again, backing off (`retry_after`), until `stop` flips or the
    /// pairing is gone.
    /// `secret` is the one-time pairing secret for a first connection.
    pub async fn link_loop(
        self: &Arc<Self>,
        sessions: Sessions,
        mut secret: Option<String>,
        mut stop: watch::Receiver<bool>,
    ) -> LinkEnd {
        let mut failures = 0u32;
        loop {
            let Some(pc) = self.paired_pc() else {
                return LinkEnd::Stopped;
            };
            let host: Arc<dyn Host> = self.clone();
            let dial = self.bluetooth.lock().unwrap().clone();
            let upgraded = Arc::new(AtomicBool::new(false));
            let watcher = tokio::spawn(self.clone().back_to_network(
                sessions.clone(),
                pc.device_id.clone(),
                upgraded.clone(),
            ));
            let result = link::connect(
                &pc.addrs,
                &pc.fingerprint,
                &self.identity,
                secret.clone(),
                dial,
                host,
                sessions.clone(),
                stop.clone(),
            )
            .await;
            watcher.abort();
            self.online.store(false, Ordering::Relaxed);
            // The secret is spent once the PC has heard it, not when the PC was out of reach.
            if !matches!(&result, Err(e) if e.is::<link::Unreachable>()) {
                secret = None;
            }
            if *stop.borrow() || self.paired_pc().is_none() {
                return LinkEnd::Stopped;
            }
            match result {
                // Not "connection refused" from a closed port: only the PC's own no.
                Err(e) if e.chain().any(|c| c.is::<link::Refused>()) => {
                    let _ = self.forget_pc();
                    return LinkEnd::Refused(format!("{e:#}"));
                }
                Err(e) => {
                    failures += 1;
                    let wait = retry_after(failures, jitter());
                    self.say(format!(
                        "Can't reach {}: {e:#}. Trying again in {} s.",
                        pc.name,
                        wait.as_secs().max(1)
                    ));
                    tokio::select! {
                        _ = tokio::time::sleep(wait) => {}
                        _ = self.wake.notified() => {}
                        changed = stop.changed() => if changed.is_err() {
                            return LinkEnd::Stopped;
                        },
                    }
                    continue;
                }
                Ok(_) if upgraded.load(Ordering::Relaxed) => {
                    self.say(format!(
                        "{} answers on the network again; moving off Bluetooth.",
                        pc.name
                    ));
                    continue;
                }
                Ok(_) => {
                    failures = 0;
                    self.say("Connection closed. Reconnecting in 2 s.");
                }
            }
            let wait = retry_after(1, jitter());
            tokio::select! {
                _ = tokio::time::sleep(wait) => {}
                _ = self.wake.notified() => {}
                changed = stop.changed() => if changed.is_err() {
                    return LinkEnd::Stopped;
                },
            }
        }
    }

    /// While the link runs over Bluetooth (the network wasn't there when it
    /// connected), looks for the PC on the network every 30 s and, once it
    /// answers and nothing is being transcribed, drops the slow link so the
    /// next connection takes the network.
    async fn back_to_network(
        self: Arc<Self>,
        sessions: Sessions,
        pc_id: String,
        upgraded: Arc<AtomicBool>,
    ) {
        loop {
            let every = self.network_check_ms.load(Ordering::Relaxed);
            tokio::time::sleep(Duration::from_millis(every)).await;
            if self.via() != Some("bluetooth") || !self.waiting.lock().unwrap().is_empty() {
                continue;
            }
            let Some(pc) = self.paired_pc() else { return };
            if link::network_answers(&pc.addrs).await && self.waiting.lock().unwrap().is_empty() {
                upgraded.store(true, Ordering::Relaxed);
                sessions.close(&pc_id);
                return;
            }
        }
    }

    fn say(&self, line: impl Into<String>) {
        (self.say)(line.into())
    }

    pub fn conn(&self) -> Result<Connection> {
        self.engine.conn()
    }

    pub fn paired_pc(&self) -> Option<PairedPc> {
        let text = std::fs::read_to_string(self.dir.join("pc.json")).ok()?;
        serde_json::from_str(&text).ok()
    }

    pub fn save_pc(&self, pc: &PairedPc) -> Result<()> {
        std::fs::write(self.dir.join("pc.json"), serde_json::to_string_pretty(pc)?)?;
        Ok(())
    }

    pub fn forget_pc(&self) -> Result<()> {
        if let Some(pc) = self.paired_pc() {
            ledger::remove_peer(&self.conn()?, &pc.device_id)?;
        }
        let _ = std::fs::remove_file(self.dir.join("pc.json"));
        Ok(())
    }

    /// Records local edits so the PC can pull them.
    pub fn refresh(&self) -> Result<usize> {
        let all: Vec<String> = KINDS.iter().map(|k| k.to_string()).collect();
        self.engine.refresh(&all)
    }

    pub fn add_word(&self, text: &str) -> Result<()> {
        self.conn()?.execute(
            "INSERT INTO dictionary_entry (text, added_at) VALUES (?1, ?2)",
            params![text, now_s()],
        )?;
        Ok(())
    }

    pub fn remove_word(&self, text: &str) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "DELETE FROM dictionary_heard WHERE entry_id IN
                (SELECT id FROM dictionary_entry WHERE text = ?1 COLLATE NOCASE)",
            [text],
        )?;
        conn.execute(
            "DELETE FROM dictionary_entry WHERE text = ?1 COLLATE NOCASE",
            [text],
        )?;
        Ok(())
    }

    pub fn add_snippet(&self, trigger: &str, text: &str) -> Result<()> {
        self.conn()?.execute(
            "INSERT INTO snippet (trigger, text, added_at) VALUES (?1, ?2, ?3)",
            params![trigger, text, now_s()],
        )?;
        Ok(())
    }

    /// A dictation taken on the phone.
    pub fn dictate(&self, text: &str) -> Result<()> {
        let words = text.split_whitespace().count() as i64;
        self.conn()?.execute(
            "INSERT INTO transcription_history (file_name, timestamp, title, transcription_text,
                app_name, word_count, status)
             VALUES ('', ?1, 'Phone', ?2, 'Gboard', ?3, 'pasted')",
            params![now_s(), text, words],
        )?;
        Ok(())
    }

    /// A voice memo recorded on the phone: one line per stretch of speech,
    /// as (start ms, end ms, text). Returns the note's id.
    pub fn add_memo(
        &self,
        title: &str,
        started_at: i64,
        duration_ms: i64,
        lines: &[(i64, i64, String)],
    ) -> Result<i64> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO note (title, kind, status, started_at, duration_ms)
             VALUES (?1, 'memo', 'ready', ?2, ?3)",
            params![title, started_at, duration_ms],
        )?;
        let id = tx.last_insert_rowid();
        for (at_ms, end_ms, text) in lines {
            tx.execute(
                "INSERT INTO note_line (note_id, at_ms, end_ms, who, text)
                 VALUES (?1, ?2, ?3, 'you', ?4)",
                params![id, at_ms, end_ms, text],
            )?;
        }
        tx.commit()?;
        Ok(id)
    }

    /// Writes a Scratchpad note, or replaces the text of the one titled so.
    pub fn scratch(&self, title: &str, text: &str) -> Result<()> {
        let conn = self.conn()?;
        let changed = conn.execute(
            "UPDATE scratch_note SET text = ?2, updated_at = ?3 WHERE title = ?1",
            params![title, text, now_s()],
        )?;
        if changed == 0 {
            conn.execute(
                "INSERT INTO scratch_note (title, auto_title, text, created_at, updated_at)
                 VALUES (?1, 0, ?2, ?3, ?3)",
                params![title, text, now_s()],
            )?;
        }
        Ok(())
    }

    /// One line per item of a kind, for `list`.
    pub fn list(&self, kind: &str) -> Result<Vec<String>> {
        let conn = self.conn()?;
        let sql = match kind {
            "dictionary" | "words" => "SELECT text FROM dictionary_entry ORDER BY text",
            "snippets" => "SELECT trigger || ' = ' || text FROM snippet ORDER BY trigger",
            "history" => {
                "SELECT COALESCE(post_processed_text, transcription_text) FROM transcription_history
                 ORDER BY timestamp"
            }
            "notes" => "SELECT title FROM note ORDER BY started_at",
            "scratchpad" | "scratch" => {
                "SELECT title || ': ' || text FROM scratch_note ORDER BY id"
            }
            "transforms" | "style" => {
                let mut stmt =
                    conn.prepare("SELECT key, body FROM phone_extra WHERE kind = ?1 ORDER BY key")?;
                let rows = stmt.query_map([kind], |r| {
                    Ok(format!(
                        "{} {}",
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?
                    ))
                })?;
                return Ok(rows.collect::<rusqlite::Result<_>>()?);
            }
            _ => anyhow::bail!("no such kind: {kind}"),
        };
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    fn peer(&self, device_id: &str) -> Option<Peer> {
        ledger::peer(&self.conn().ok()?, device_id).ok().flatten()
    }
}

impl Host for Phone {
    fn describe(&self) -> Welcome {
        Welcome {
            protocol: PROTOCOL,
            device_id: self.identity.device_id.clone(),
            name: self.name.clone(),
            platform: "android".into(),
            app_version: "stand-in".into(),
            kinds: KINDS.iter().map(|k| k.to_string()).collect(),
            clipboard: true,
            addrs: Vec::new(),
            transcribe: false,
        }
    }

    fn admit(&self, _hello: &Hello, _fingerprint: &str) -> Result<(), Refusal> {
        Err(Refusal::UnknownDevice)
    }

    fn welcomed(&self, welcome: &Welcome, fingerprint: &str) {
        *self.pc_kinds.lock().unwrap() = welcome.kinds.clone();
        self.pc_transcribes
            .store(welcome.transcribe, Ordering::Relaxed);
        // Keep where the PC is now (a new Wi-Fi address, Bluetooth turned on
        // since pairing) for the next connection.
        if let Some(mut pc) = self.paired_pc() {
            if !welcome.addrs.is_empty() && pc.addrs != welcome.addrs {
                pc.addrs = welcome.addrs.clone();
                let _ = self.save_pc(&pc);
            }
        }
        if fingerprint.is_empty() {
            self.say(format!(
                "{} now syncs: {}",
                welcome.name,
                welcome.kinds.join(", ")
            ));
            return;
        }
        if self.peer(&welcome.device_id).is_none() {
            let peer = Peer {
                device_id: welcome.device_id.clone(),
                name: welcome.name.clone(),
                platform: welcome.platform.clone(),
                app_version: welcome.app_version.clone(),
                fingerprint: fingerprint.to_string(),
                paired_at: now_s(),
                last_seen: Some(now_s()),
                last_sync: None,
                sent_upto: 0,
                recv_upto: 0,
            };
            if let Ok(conn) = self.conn() {
                let _ = ledger::add_peer(&conn, &peer);
            }
            self.say(format!("Paired with {}", welcome.name));
        }
        self.say(format!(
            "Connected to {} (Legato {}). It syncs: {}. Shared clipboard {}.",
            welcome.name,
            welcome.app_version,
            welcome.kinds.join(", "),
            if welcome.clipboard { "on" } else { "off" }
        ));
    }

    fn changes(
        &self,
        peer: &str,
        after: u64,
        kinds: &[String],
        limit: usize,
    ) -> Result<(Vec<Change>, u64, bool)> {
        let all: Vec<String> = KINDS.iter().map(|k| k.to_string()).collect();
        let answer = self.engine.changes(peer, after, kinds, &all, limit)?;
        if !answer.0.is_empty() {
            self.say(format!("Sent {} changes", answer.0.len()));
        }
        Ok(answer)
    }

    fn accept(&self, peer: &str, items: Vec<Change>, upto: u64) -> Result<()> {
        let Some(known) = self.peer(peer) else {
            return Ok(());
        };
        let kinds = self.pc_kinds.lock().unwrap().clone();
        let done = self.engine.accept(&known, items, upto, &kinds)?;
        if done.applied > 0 || done.merges > 0 {
            self.say(format!(
                "Took {} changes ({}){}",
                done.applied,
                done.kinds.iter().cloned().collect::<Vec<_>>().join(", "),
                if done.merges > 0 {
                    format!(", {} merged", done.merges)
                } else {
                    String::new()
                }
            ));
        }
        Ok(())
    }

    fn recv_upto(&self, peer: &str) -> u64 {
        self.peer(peer).map_or(0, |p| p.recv_upto)
    }

    fn wanted(&self, _peer: &str) -> Vec<String> {
        self.pc_kinds.lock().unwrap().clone()
    }

    fn clip(&self, _peer: &str, clip: Clip) {
        let what = match (&clip.text, &clip.png) {
            (Some(text), _) => format!("“{text}”"),
            (_, Some(png)) => format!("an image ({} base64 bytes)", png.len()),
            _ => "an empty clip".into(),
        };
        self.say(format!(
            "Clip from the PC: {what}{}",
            clip.from_app
                .as_ref()
                .map(|a| format!(" from {a}"))
                .unwrap_or_default()
        ));
        if let Some(hear) = self.on_clip.lock().unwrap().as_ref() {
            hear(&clip);
        }
        let mut clips = self.clips.lock().unwrap();
        clips.push(clip);
        // The recent ones are enough; an evening of copying shouldn't grow without end.
        let over = clips.len().saturating_sub(20);
        clips.drain(..over);
    }

    fn online(&self, _peer: &str, via: Option<Via>) {
        self.online.store(via.is_some(), Ordering::Relaxed);
        let kind = via.map(|via| match via {
            Via::Bluetooth => "bluetooth",
            Via::Net(addr) => match addr.ip() {
                std::net::IpAddr::V4(ip) => sync::hub::address_kind(ip).unwrap_or("lan"),
                std::net::IpAddr::V6(_) => "lan",
            },
        });
        *self.via.lock().unwrap() = kind;
        // The reconnect loop says when the link drops.
        match kind {
            Some("bluetooth") => self.say("Connected over Bluetooth"),
            Some("netbird") => self.say("Connected through Netbird"),
            Some(_) => self.say("Connected on Wi-Fi"),
            None => {}
        }
    }

    fn transcript(&self, _peer: &str, id: &str, result: std::result::Result<String, String>) {
        if let Some(tx) = self.waiting.lock().unwrap().remove(id) {
            let _ = tx.send(result);
        }
    }

    fn forgotten(&self, _peer: &str) {
        let _ = self.forget_pc();
        self.say("The PC unpaired this phone");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retries_back_off_to_five_minutes_with_jitter() {
        assert_eq!(retry_after(1, 0.5), Duration::from_secs(2));
        assert_eq!(retry_after(3, 0.5), Duration::from_secs(8));
        assert_eq!(retry_after(40, 0.5), Duration::from_secs(300));
        assert_eq!(retry_after(40, 0.0), Duration::from_secs(240));
        assert_eq!(retry_after(40, 1.0), Duration::from_secs(360));
    }
}
