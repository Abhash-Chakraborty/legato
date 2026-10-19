//! A phone for testing Legato's phone sync before the Android app exists.
//! It keeps a database shaped like the PC's and runs the app's own sync
//! modules, so pairing, syncing and the shared clipboard can be tried
//! against a real Legato over Wi-Fi or Netbird.

pub mod managers;
pub mod sync;

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use rusqlite_migration::Migrations;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use sync::engine::{now_s, Engine, Extra};
use sync::identity::Identity;
use sync::ledger::{self, Item, Peer};
use sync::link::Host;
use sync::protocol::{Change, Clip, Hello, Refusal, Welcome, KINDS, PROTOCOL};

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
    say: Box<dyn Fn(String) + Send + Sync>,
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
            say,
        })
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
        }
    }

    fn admit(&self, _hello: &Hello, _fingerprint: &str) -> Result<(), Refusal> {
        Err(Refusal::UnknownDevice)
    }

    fn welcomed(&self, welcome: &Welcome, fingerprint: &str) {
        *self.pc_kinds.lock().unwrap() = welcome.kinds.clone();
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
        self.clips.lock().unwrap().push(clip);
    }

    fn online(&self, _peer: &str, addr: Option<SocketAddr>) {
        if addr.is_none() {
            self.say("Disconnected");
        }
    }

    fn forgotten(&self, _peer: &str) {
        let _ = self.forget_pc();
        self.say("The PC unpaired this phone");
    }
}
