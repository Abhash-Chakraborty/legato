//! Legato for Android's core, exposed to Kotlin through UniFFI: the phone
//! half of sync (pairing, the ledger, the shared clipboard) from
//! `legato-phone`, plus speech on the phone with transcribe-cpp.
//!
//! The Kotlin side owns the UI, the microphone and the bubble; this side owns
//! everything that must behave exactly like the PC.

uniffi::setup_scaffolding!();

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use legato_phone::sync::identity::uuid;
use legato_phone::sync::link::{Command, Sessions};
use legato_phone::sync::protocol::{Clip, ClipKind, PairLink};
use legato_phone::{LinkEnd, PairedPc, Phone};
use tokio::runtime::Runtime;
use tokio::sync::watch;

#[cfg(feature = "speech")]
mod speech;

/// Anything that went wrong, as one line for the UI.
#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum CoreError {
    #[error("{0}")]
    Failed(String),
}

impl From<anyhow::Error> for CoreError {
    fn from(e: anyhow::Error) -> Self {
        CoreError::Failed(format!("{e:#}"))
    }
}

impl From<rusqlite::Error> for CoreError {
    fn from(e: rusqlite::Error) -> Self {
        CoreError::Failed(e.to_string())
    }
}

/// Where the phone stands with its PC.
#[derive(Clone, Debug, uniffi::Record)]
pub struct PhoneState {
    pub paired: bool,
    pub pc_name: Option<String>,
    pub online: bool,
    /// How it's connected: "lan", "netbird" or "bluetooth".
    pub via: Option<String>,
    /// Connected to a PC that transcribes recordings with its own model.
    pub pc_transcribes: bool,
    /// Seconds since 1970 of the last sync, if any.
    pub last_sync: Option<i64>,
}

/// What a pairing link would pair with, shown before anything changes.
#[derive(Clone, Debug, uniffi::Record)]
pub struct PairPreview {
    pub pc_name: String,
    /// "192.168.1.20:47613", or "Bluetooth" for a `bt:` address.
    pub addresses: Vec<String>,
    /// The PC this one would replace, when it's paired with another.
    pub replaces: Option<String>,
}

/// One dictation on Home.
#[derive(Clone, Debug, uniffi::Record)]
pub struct HistoryEntry {
    pub id: i64,
    /// Seconds since 1970.
    pub at: i64,
    pub text: String,
    pub app: String,
    /// The device name when it came from the PC; None for this phone.
    pub device: Option<String>,
}

/// A clip the PC sent.
#[derive(Clone, Debug, uniffi::Record)]
pub struct ClipItem {
    pub text: Option<String>,
    pub from_app: Option<String>,
    pub age_ms: u64,
}

/// What the Kotlin side hears from the core.
#[uniffi::export(with_foreign)]
pub trait CoreListener: Send + Sync {
    /// A line about what just happened, in plain words (connected, synced, clip from the PC).
    fn on_line(&self, line: String);
    /// The pairing was refused or ended by the PC; the phone is unpaired now.
    fn on_unpaired(&self, why: String);
    /// Connects to the PC over Bluetooth ("AA:BB:CC:DD:EE:FF") when no
    /// network address answers, and returns a port on 127.0.0.1 that carries
    /// the link, or 0 when it can't. It may block while it connects. The
    /// port carries only the first connection that sends `token`; the core
    /// sends it before anything else.
    fn open_bluetooth(&self, address: String, token: String) -> u16;
    /// A text the PC copied, as it arrives; images stay in `clips`.
    /// `age_ms` is how long ago it was copied on the PC.
    fn on_clip(&self, text: String, from_app: Option<String>, age_ms: u64);
}

/// The phone's core. One per app process.
#[derive(uniffi::Object)]
pub struct Core {
    phone: Arc<Phone>,
    rt: Runtime,
    sessions: Sessions,
    stop: Mutex<Option<watch::Sender<bool>>>,
    listener: Arc<dyn CoreListener>,
    #[cfg(feature = "speech")]
    speech: speech::Speech,
}

#[uniffi::export]
impl Core {
    /// Opens the phone's database and identity in `dir` (the app's files dir).
    /// `name` is what the PC shows for this phone.
    #[uniffi::constructor]
    pub fn new(
        dir: String,
        name: String,
        listener: Arc<dyn CoreListener>,
    ) -> Result<Arc<Self>, CoreError> {
        let say = {
            let listener = listener.clone();
            Box::new(move |line: String| listener.on_line(line))
        };
        let mut phone = Phone::open(Path::new(&dir), &name, say)?;
        phone.app_version = env!("CARGO_PKG_VERSION").to_string();
        let phone = Arc::new(phone);
        *phone.bluetooth.lock().unwrap() = Some({
            let listener = listener.clone();
            Arc::new(move |address: &str, token: &str| {
                Some(listener.open_bluetooth(address.to_string(), token.to_string()))
                    .filter(|port| *port != 0)
            })
        });
        *phone.on_clip.lock().unwrap() = Some({
            let listener = listener.clone();
            Box::new(move |clip: &Clip| {
                if let Some(text) = clip.text.clone().filter(|t| !t.trim().is_empty()) {
                    listener.on_clip(text, clip.from_app.clone(), clip.age_ms);
                }
            })
        });
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|e| CoreError::Failed(e.to_string()))?;
        Ok(Arc::new(Self {
            phone,
            rt,
            sessions: Sessions::default(),
            stop: Mutex::new(None),
            listener,
            #[cfg(feature = "speech")]
            speech: speech::Speech::default(),
        }))
    }

    pub fn state(&self) -> PhoneState {
        let pc = self.phone.paired_pc();
        let last_sync = pc.as_ref().and_then(|pc| {
            let conn = self.phone.conn().ok()?;
            legato_phone::sync::ledger::peer(&conn, &pc.device_id)
                .ok()
                .flatten()
                .and_then(|p| p.last_sync)
        });
        PhoneState {
            paired: pc.is_some(),
            pc_name: pc.map(|p| p.name),
            online: self.phone.is_online(),
            via: self.phone.via().map(str::to_string),
            pc_transcribes: self.phone.pc_transcribes(),
            last_sync,
        }
    }

    /// Reads a pairing link without pairing, so the app can ask first. Errs
    /// on a link that isn't Legato's or points outside the local network.
    pub fn inspect(&self, link: String) -> Result<PairPreview, CoreError> {
        let link = PairLink::parse(link.trim())?;
        let replaces = self
            .phone
            .paired_pc()
            .filter(|pc| pc.device_id != link.device_id)
            .map(|pc| pc.name);
        Ok(PairPreview {
            pc_name: link.name,
            addresses: link
                .addrs
                .iter()
                .map(|a| {
                    if a.starts_with(legato_phone::sync::protocol::BT_PREFIX) {
                        "Bluetooth".to_string()
                    } else {
                        a.clone()
                    }
                })
                .collect(),
            replaces,
        })
    }

    /// Pairs with the PC whose link this is (scanned or pasted), and connects.
    pub fn pair(&self, link: String) -> Result<(), CoreError> {
        let link = PairLink::parse(link.trim())?;
        self.disconnect();
        self.phone.save_pc(&PairedPc {
            device_id: link.device_id,
            name: link.name,
            fingerprint: link.fingerprint,
            addrs: link.addrs,
        })?;
        self.phone.refresh()?;
        self.start(Some(link.secret));
        Ok(())
    }

    /// Connects to the paired PC and keeps trying while the app is up.
    pub fn connect(&self) -> Result<(), CoreError> {
        if self.phone.paired_pc().is_none() {
            return Err(CoreError::Failed("Not paired with a PC yet".into()));
        }
        if self.stop.lock().unwrap().is_some() {
            // Already trying: try now instead of after the backoff.
            self.phone.wake();
            return Ok(());
        }
        self.phone.refresh()?;
        self.start(None);
        Ok(())
    }

    /// Drops the connection (the app went to the background, say).
    pub fn disconnect(&self) {
        if let Some(stop) = self.stop.lock().unwrap().take() {
            let _ = stop.send(true);
        }
        self.sessions.close_all();
    }

    /// Tells the PC goodbye and forgets it. What already synced stays.
    pub fn unpair(&self) -> Result<(), CoreError> {
        if let Some(pc) = self.phone.paired_pc() {
            if self
                .sessions
                .send(&pc.device_id, Command::Bye("unpaired".into()))
            {
                std::thread::sleep(Duration::from_millis(300));
            }
        }
        self.disconnect();
        self.phone.forget_pc()?;
        Ok(())
    }

    /// Asks the PC for its changes now and offers ours.
    pub fn sync_now(&self) -> Result<bool, CoreError> {
        let Some(pc) = self.phone.paired_pc() else {
            return Ok(false);
        };
        self.phone.refresh()?;
        let sent = self.sessions.send(&pc.device_id, Command::Nudge);
        self.sessions.send(&pc.device_id, Command::Pull);
        Ok(sent)
    }

    pub fn words(&self) -> Result<Vec<String>, CoreError> {
        Ok(self.phone.list("dictionary")?)
    }

    pub fn add_word(&self, text: String) -> Result<(), CoreError> {
        let text = text.trim();
        if text.is_empty() {
            return Ok(());
        }
        self.phone.add_word(text)?;
        self.changed()
    }

    pub fn remove_word(&self, text: String) -> Result<(), CoreError> {
        self.phone.remove_word(text.trim())?;
        self.changed()
    }

    pub fn snippets(&self) -> Result<Vec<String>, CoreError> {
        Ok(self.phone.list("snippets")?)
    }

    pub fn add_snippet(&self, trigger: String, text: String) -> Result<(), CoreError> {
        self.phone
            .add_snippet(trigger.trim().to_lowercase().as_str(), text.trim())?;
        self.changed()
    }

    /// The newest dictations first, from both devices.
    pub fn history(&self, limit: u32) -> Result<Vec<HistoryEntry>, CoreError> {
        let conn = self.phone.conn()?;
        let mut stmt = conn.prepare(
            "SELECT h.id, h.timestamp, COALESCE(h.post_processed_text, h.transcription_text),
                    COALESCE(h.app_name, ''),
                    (SELECT device FROM sync_local WHERE kind = 'history' AND local_id = h.id)
             FROM transcription_history h
             ORDER BY h.timestamp DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit as i64], |r| {
            Ok(HistoryEntry {
                id: r.get(0)?,
                at: r.get(1)?,
                text: r.get(2)?,
                app: r.get(3)?,
                device: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Records a dictation made on this phone, so Home and the PC see it.
    pub fn dictate(&self, text: String) -> Result<(), CoreError> {
        let text = text.trim();
        if text.is_empty() {
            return Ok(());
        }
        self.phone.dictate(text)?;
        self.changed()
    }

    /// Saves a voice memo recorded on the phone as a note, so Notes and the
    /// PC see it. `started_at` is in seconds since 1970. Returns its id.
    pub fn add_memo(
        &self,
        title: String,
        started_at: i64,
        duration_ms: i64,
        lines: Vec<MemoLine>,
    ) -> Result<i64, CoreError> {
        let lines: Vec<(i64, i64, String)> = lines
            .into_iter()
            .map(|l| (l.at_ms, l.end_ms, l.text.trim().to_string()))
            .filter(|l| !l.2.is_empty())
            .collect();
        let id = self
            .phone
            .add_memo(title.trim(), started_at, duration_ms, &lines)?;
        self.changed()?;
        Ok(id)
    }

    /// Sends what's on the phone's clipboard to the PC. False when not connected.
    pub fn send_clip(&self, text: String) -> Result<bool, CoreError> {
        let Some(pc) = self.phone.paired_pc() else {
            return Ok(false);
        };
        let clip = Clip {
            id: uuid(),
            kind: ClipKind::Text,
            text: Some(text),
            png: None,
            age_ms: 0,
            from_app: None,
        };
        Ok(self.sessions.send(&pc.device_id, Command::Clip(clip)))
    }

    /// Clips the PC sent since the app started, newest last.
    pub fn clips(&self) -> Vec<ClipItem> {
        self.phone
            .clips
            .lock()
            .unwrap()
            .iter()
            .map(|c| ClipItem {
                text: c.text.clone(),
                from_app: c.from_app.clone(),
                age_ms: c.age_ms,
            })
            .collect()
    }

    /// Turns a recording (16 kHz mono, signed 16-bit little-endian) into text
    /// with the PC's speech model, over the sync link, styled for the app
    /// `package` (empty when none). Blocks until the PC answers.
    pub fn transcribe_on_pc(&self, pcm: Vec<u8>, package: String) -> Result<String, CoreError> {
        let samples = legato_phone::pieces::from_pcm16(&pcm);
        let app = (!package.is_empty()).then(|| legato_phone::finish::category_of(&package));
        // A minute for the PC to answer, plus time for a long recording to
        // travel (slowly, over Bluetooth) and be written up.
        let seconds = samples.len() as u64 / legato_phone::pieces::RATE as u64;
        let wait = Duration::from_secs(60 + 2 * seconds);
        Ok(self
            .phone
            .transcribe_on_pc(&self.sessions, &samples, wait, app)?)
    }

    /// Text from this device's model, finished as the PC finishes a dictation
    /// for the app `package`: the Dictionary, snippets, cleanup and Style.
    pub fn finish_dictation(&self, raw: String, package: String) -> Result<String, CoreError> {
        Ok(self.phone.finish_dictation(&raw, &package)?)
    }

    /// Asks the PC to load its speech model now, as the user starts talking.
    /// False when not connected.
    pub fn warm_pc(&self) -> bool {
        self.phone.warm_pc(&self.sessions)
    }

    /// Loads a speech model (a GGUF file) for `transcribe`.
    pub fn load_model(&self, path: String) -> Result<String, CoreError> {
        #[cfg(feature = "speech")]
        {
            self.speech.load(Path::new(&path))
        }
        #[cfg(not(feature = "speech"))]
        {
            let _ = path;
            Err(CoreError::Failed("built without speech".into()))
        }
    }

    /// Turns a recording (16 kHz mono, signed 16-bit little-endian) into text
    /// with the loaded model.
    /// `language` is an ISO code for a model that's told what to expect, or
    /// None for one that works it out.
    pub fn transcribe(&self, pcm: Vec<u8>, language: Option<String>) -> Result<String, CoreError> {
        #[cfg(feature = "speech")]
        {
            self.speech
                .transcribe(&legato_phone::pieces::from_pcm16(&pcm), language)
        }
        #[cfg(not(feature = "speech"))]
        {
            let _ = (pcm, language);
            Err(CoreError::Failed("built without speech".into()))
        }
    }

    /// Lets go of the speech model after a while unused; `load_model` again
    /// before the next `transcribe`.
    pub fn unload_model(&self) {
        #[cfg(feature = "speech")]
        self.speech.unload();
    }

    /// Whether a speech model is in memory.
    pub fn model_loaded(&self) -> bool {
        #[cfg(feature = "speech")]
        {
            self.speech.loaded()
        }
        #[cfg(not(feature = "speech"))]
        {
            false
        }
    }
}

impl Core {
    fn start(&self, secret: Option<String>) {
        let (stop_tx, stop) = watch::channel(false);
        *self.stop.lock().unwrap() = Some(stop_tx);
        let (phone, sessions, listener) = (
            self.phone.clone(),
            self.sessions.clone(),
            self.listener.clone(),
        );
        self.rt.spawn(async move {
            if let LinkEnd::Refused(why) = phone.link_loop(sessions, secret, stop).await {
                listener.on_unpaired(why);
            }
        });
    }

    /// After a local edit: note it in the ledger and tell the PC.
    fn changed(&self) -> Result<(), CoreError> {
        let changed = self.phone.refresh()?;
        if changed > 0 {
            if let Some(pc) = self.phone.paired_pc() {
                self.sessions.send(&pc.device_id, Command::Nudge);
            }
        }
        Ok(())
    }
}

/// The core's version, for the About row.
#[uniffi::export]
pub fn core_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

// ---- the app's screens (step A3): structured rows and edits ---------------

/// A Dictionary word with the forms the model mishears it as.
#[derive(Clone, Debug, uniffi::Record)]
pub struct WordRow {
    pub id: i64,
    pub text: String,
    pub heard: Vec<String>,
    pub pinned: bool,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SnippetRow {
    pub id: i64,
    pub trigger: String,
    pub text: String,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct ScratchNote {
    pub id: i64,
    pub title: String,
    pub text: String,
    /// Seconds since 1970.
    pub updated_at: i64,
}

/// A Notes recording, as the list shows it.
#[derive(Clone, Debug, uniffi::Record)]
pub struct NoteRow {
    pub id: i64,
    pub title: String,
    pub started_at: i64,
    pub duration_ms: i64,
    pub summary: String,
    pub open_actions: u32,
    /// The device name when it came from the PC.
    pub device: Option<String>,
}

/// One stretch of a voice memo recorded on the phone.
#[derive(Clone, Debug, uniffi::Record)]
pub struct MemoLine {
    pub at_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct ActionItem {
    pub id: i64,
    pub text: String,
    pub done: bool,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct NoteDetail {
    pub note: NoteRow,
    pub actions: Vec<ActionItem>,
    /// The transcript, one line per entry, as "who: text".
    pub transcript: Vec<String>,
}

#[uniffi::export]
impl Core {
    pub fn word_rows(&self) -> Result<Vec<WordRow>, CoreError> {
        let conn = self.phone.conn()?;
        let mut stmt = conn.prepare(
            "SELECT e.id, e.text, e.pinned,
                    (SELECT group_concat(h.heard, char(31)) FROM dictionary_heard h WHERE h.entry_id = e.id)
             FROM dictionary_entry e ORDER BY e.pinned DESC, e.text COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |r| {
            let heard: Option<String> = r.get(3)?;
            Ok(WordRow {
                id: r.get(0)?,
                text: r.get(1)?,
                pinned: r.get::<_, i64>(2)? != 0,
                heard: heard
                    .map(|h| h.split('\u{1f}').map(str::to_string).collect())
                    .unwrap_or_default(),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Adds a word, with the form the model writes instead when there is one.
    pub fn add_word_heard(&self, text: String, heard: String) -> Result<(), CoreError> {
        let text = text.trim();
        if text.is_empty() {
            return Ok(());
        }
        let conn = self.phone.conn()?;
        conn.execute(
            "INSERT OR IGNORE INTO dictionary_entry (text, added_at) VALUES (?1, ?2)",
            rusqlite::params![text, legato_phone::sync::engine::now_s()],
        )?;
        let heard = heard.trim().to_lowercase();
        if !heard.is_empty() {
            conn.execute(
                "INSERT OR IGNORE INTO dictionary_heard (entry_id, heard)
                 SELECT id, ?2 FROM dictionary_entry WHERE text = ?1 COLLATE NOCASE",
                rusqlite::params![text, heard],
            )?;
        }
        self.changed()
    }

    pub fn snippet_rows(&self) -> Result<Vec<SnippetRow>, CoreError> {
        let conn = self.phone.conn()?;
        let mut stmt = conn.prepare("SELECT id, trigger, text FROM snippet ORDER BY trigger")?;
        let rows = stmt.query_map([], |r| {
            Ok(SnippetRow {
                id: r.get(0)?,
                trigger: r.get(1)?,
                text: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn save_snippet(
        &self,
        id: Option<i64>,
        trigger: String,
        text: String,
    ) -> Result<(), CoreError> {
        let trigger = trigger.trim().to_lowercase();
        let text = text.trim().to_string();
        if trigger.is_empty() || text.is_empty() {
            return Ok(());
        }
        let conn = self.phone.conn()?;
        match id {
            Some(id) => conn.execute(
                "UPDATE snippet SET trigger = ?2, text = ?3 WHERE id = ?1",
                rusqlite::params![id, trigger, text],
            )?,
            None => conn.execute(
                "INSERT INTO snippet (trigger, text, added_at) VALUES (?1, ?2, ?3)",
                rusqlite::params![trigger, text, legato_phone::sync::engine::now_s()],
            )?,
        };
        self.changed()
    }

    pub fn remove_snippet(&self, id: i64) -> Result<(), CoreError> {
        self.phone
            .conn()?
            .execute("DELETE FROM snippet WHERE id = ?1", [id])?;
        self.changed()
    }

    pub fn scratch_notes(&self) -> Result<Vec<ScratchNote>, CoreError> {
        let conn = self.phone.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, text, updated_at FROM scratch_note ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(ScratchNote {
                id: r.get(0)?,
                title: r.get(1)?,
                text: r.get(2)?,
                updated_at: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Saves a Scratchpad note; None makes a new one. Returns its id.
    pub fn save_scratch(
        &self,
        id: Option<i64>,
        title: String,
        text: String,
    ) -> Result<i64, CoreError> {
        let conn = self.phone.conn()?;
        let now = legato_phone::sync::engine::now_s();
        let id = match id {
            Some(id) => {
                conn.execute(
                    "UPDATE scratch_note SET title = ?2, auto_title = 0, text = ?3, updated_at = ?4 WHERE id = ?1",
                    rusqlite::params![id, title.trim(), text, now],
                )?;
                id
            }
            None => {
                conn.execute(
                    "INSERT INTO scratch_note (title, auto_title, text, created_at, updated_at)
                     VALUES (?1, 0, ?2, ?3, ?3)",
                    rusqlite::params![title.trim(), text, now],
                )?;
                conn.last_insert_rowid()
            }
        };
        self.changed()?;
        Ok(id)
    }

    pub fn delete_scratch(&self, id: i64) -> Result<(), CoreError> {
        let conn = self.phone.conn()?;
        conn.execute("DELETE FROM scratch_version WHERE note_id = ?1", [id])?;
        conn.execute("DELETE FROM scratch_note WHERE id = ?1", [id])?;
        self.changed()
    }

    pub fn notes(&self) -> Result<Vec<NoteRow>, CoreError> {
        let conn = self.phone.conn()?;
        let mut stmt = conn.prepare(
            "SELECT n.id, n.title, n.started_at, n.duration_ms, n.summary,
                    (SELECT COUNT(*) FROM note_action a WHERE a.note_id = n.id AND a.done = 0),
                    (SELECT device FROM sync_local WHERE kind = 'notes' AND local_id = n.id)
             FROM note n WHERE n.status = 'ready' ORDER BY n.started_at DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(NoteRow {
                id: r.get(0)?,
                title: r.get(1)?,
                started_at: r.get(2)?,
                duration_ms: r.get(3)?,
                summary: r.get(4)?,
                open_actions: r.get::<_, i64>(5)? as u32,
                device: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn note(&self, id: i64) -> Result<NoteDetail, CoreError> {
        let note = self
            .notes()?
            .into_iter()
            .find(|n| n.id == id)
            .ok_or_else(|| CoreError::Failed("That note is gone".into()))?;
        let conn = self.phone.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, text, done FROM note_action WHERE note_id = ?1 ORDER BY position, id",
        )?;
        let actions = stmt
            .query_map([id], |r| {
                Ok(ActionItem {
                    id: r.get(0)?,
                    text: r.get(1)?,
                    done: r.get::<_, i64>(2)? != 0,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut stmt =
            conn.prepare("SELECT who, text FROM note_line WHERE note_id = ?1 ORDER BY at_ms, id")?;
        let transcript = stmt
            .query_map([id], |r| {
                Ok(format!(
                    "{}: {}",
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(NoteDetail {
            note,
            actions,
            transcript,
        })
    }

    /// Ticks or unticks an action item; the PC sees it on the next sync.
    pub fn set_action_done(&self, id: i64, done: bool) -> Result<(), CoreError> {
        self.phone.conn()?.execute(
            "UPDATE note_action SET done = ?2 WHERE id = ?1",
            rusqlite::params![id, done as i64],
        )?;
        self.changed()
    }

    /// Deletes a dictation here and, through a tombstone, on the PC.
    pub fn delete_history(&self, id: i64) -> Result<(), CoreError> {
        let conn = self.phone.conn()?;
        legato_phone::sync::ledger::forget(
            &conn,
            "history",
            id,
            legato_phone::sync::engine::now_s(),
        )?;
        conn.execute("DELETE FROM transcription_history WHERE id = ?1", [id])?;
        self.changed()
    }
}
