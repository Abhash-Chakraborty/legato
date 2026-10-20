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
    /// Seconds since 1970 of the last sync, if any.
    pub last_sync: Option<i64>,
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
        let phone = Arc::new(Phone::open(Path::new(&dir), &name, say)?);
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
            last_sync,
        }
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

    /// Turns 16 kHz mono samples (-1..1) into text with the loaded model.
    pub fn transcribe(&self, samples: Vec<f32>) -> Result<String, CoreError> {
        #[cfg(feature = "speech")]
        {
            self.speech.transcribe(&samples)
        }
        #[cfg(not(feature = "speech"))]
        {
            let _ = samples;
            Err(CoreError::Failed("built without speech".into()))
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
