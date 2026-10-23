//! Word copies of every note in a folder you pick (Settings › Notes), kept
//! in step as notes change. Pick the folder Google Drive for desktop or
//! OneDrive syncs, and the copies reach the cloud through that app: Legato
//! itself never signs in anywhere or goes online for them.
//!
//! A copy is rewritten when its note changes, renamed with the note's
//! title, and never deleted: a note deleted in Legato leaves its last copy.
//! A file with the same name that Legato didn't write is never overwritten.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Duration;

use anyhow::Result;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use specta::Type;
use tauri::{AppHandle, Manager};

use crate::managers::notes_store::NoteStatus;
use crate::notes::NotesManager;
use crate::notes_rules::{copy_name, unique_docx};
use crate::settings::get_settings;

/// Changes arrive in bursts (a line every few seconds while recording, a
/// summary's parts), so a pass waits for them to settle and runs at most
/// twice a minute.
const SETTLE: Duration = Duration::from_secs(5);
const BETWEEN: Duration = Duration::from_secs(25);

struct Wake {
    pending: Mutex<bool>,
    signal: Condvar,
}

static WAKE: OnceLock<Arc<Wake>> = OnceLock::new();

/// Says a note may have changed; the copies catch up shortly after.
pub fn nudge(app: &AppHandle) {
    let wake = WAKE.get_or_init(|| {
        let wake = Arc::new(Wake {
            pending: Mutex::new(false),
            signal: Condvar::new(),
        });
        let (app, w) = (app.clone(), wake.clone());
        std::thread::Builder::new()
            .name("note-copies".into())
            .spawn(move || run(app, w))
            .ok();
        wake
    });
    if let Ok(mut pending) = wake.pending.lock() {
        *pending = true;
        wake.signal.notify_one();
    }
}

fn run(app: AppHandle, wake: Arc<Wake>) {
    loop {
        {
            let Ok(mut pending) = wake.pending.lock() else {
                return;
            };
            while !*pending {
                pending = match wake.signal.wait(pending) {
                    Ok(p) => p,
                    Err(_) => return,
                };
            }
        }
        std::thread::sleep(SETTLE);
        if let Ok(mut pending) = wake.pending.lock() {
            *pending = false;
        }
        if let Err(e) = sync(&app) {
            warn!("Couldn't update the notes' Word copies: {e:#}");
        }
        std::thread::sleep(BETWEEN);
    }
}

/// What was written where, so a pass rewrites only changed notes and
/// renames a copy when its note's title changes.
#[derive(Default, Serialize, Deserialize)]
struct Ledger {
    folder: String,
    notes: HashMap<i64, Copy>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Copy {
    file: String,
    base: String,
    hash: String,
}

fn ledger_path(app: &AppHandle) -> Result<PathBuf> {
    Ok(crate::portable::app_data_dir(app)?.join("note-copies.json"))
}

fn sync(app: &AppHandle) -> Result<()> {
    let Some(folder) = folder(app) else {
        return Ok(());
    };
    let dir = Path::new(&folder);
    if !dir.is_dir() {
        // Google Drive's drive is missing while Drive isn't running.
        debug!("The notes folder isn't there right now; copies wait");
        return Ok(());
    }
    let Some(notes) = app.try_state::<Arc<NotesManager>>() else {
        return Ok(());
    };
    let path = ledger_path(app)?;
    let mut ledger: Ledger = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    if ledger.folder != folder {
        ledger = Ledger {
            folder: folder.clone(),
            notes: HashMap::new(),
        };
    }
    let mut written = 0;
    for note in notes.list()? {
        if note.status == NoteStatus::Recording {
            continue;
        }
        let hash = hex(&Sha256::digest(serde_json::to_vec(&note)?));
        let previous = ledger.notes.get(&note.id).cloned();
        if previous
            .as_ref()
            .is_some_and(|p| p.hash == hash && dir.join(&p.file).exists())
        {
            continue;
        }
        let date = chrono::DateTime::from_timestamp(note.started_at, 0)
            .map(|t| {
                t.with_timezone(&chrono::Local)
                    .format("%Y-%m-%d")
                    .to_string()
            })
            .unwrap_or_default();
        let base = copy_name(&note.title, &date);
        let file = match &previous {
            Some(p) if p.base == base => p.file.clone(),
            _ => unique_docx(&base, |name| {
                let ours = previous
                    .as_ref()
                    .is_some_and(|p| p.file.eq_ignore_ascii_case(name));
                let others = ledger
                    .notes
                    .iter()
                    .any(|(id, c)| *id != note.id && c.file.eq_ignore_ascii_case(name));
                others || (!ours && dir.join(name).exists())
            }),
        };
        let bytes = match notes.docx(note.id) {
            Ok(bytes) => bytes,
            Err(e) => {
                warn!("Couldn't make note {}'s Word copy: {e:#}", note.id);
                continue;
            }
        };
        if let Some(p) = previous.as_ref().filter(|p| p.file != file) {
            // A renamed note: its copy follows, rather than leaving two.
            let _ = std::fs::rename(dir.join(&p.file), dir.join(&file));
        }
        match std::fs::write(dir.join(&file), bytes) {
            Ok(()) => {
                written += 1;
                ledger.notes.insert(note.id, Copy { file, base, hash });
            }
            // Open in Word, say: the next pass tries again.
            Err(e) => warn!("Couldn't write {file}: {e}"),
        }
    }
    if written > 0 {
        info!("Updated {written} note copies in the notes folder");
        std::fs::write(&path, serde_json::to_vec(&ledger)?)?;
    }
    Ok(())
}

fn folder(app: &AppHandle) -> Option<String> {
    get_settings(app)
        .notes_folder
        .filter(|f| !f.trim().is_empty())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A folder a sync app keeps in the cloud, offered when you pick where the
/// copies go.
#[derive(Clone, Debug, Serialize, Type)]
pub struct FolderSuggestion {
    /// "Google Drive" or "OneDrive".
    pub service: String,
    /// A "Legato notes" folder inside it, made when chosen.
    pub path: String,
}

/// Google Drive for desktop's My Drive (on its own drive letter, or in your
/// profile when it mirrors) and OneDrive's folder, where they exist.
pub fn suggestions() -> Vec<FolderSuggestion> {
    let mut out = Vec::new();
    let mut drive: Vec<PathBuf> = ('D'..='Z')
        .map(|l| PathBuf::from(format!("{l}:\\My Drive")))
        .collect();
    if let Ok(profile) = std::env::var("USERPROFILE") {
        drive.push(Path::new(&profile).join("My Drive"));
        drive.push(Path::new(&profile).join("Google Drive"));
    }
    if let Some(dir) = drive.into_iter().find(|d| d.is_dir()) {
        out.push(FolderSuggestion {
            service: "Google Drive".into(),
            path: dir.join("Legato notes").to_string_lossy().into_owned(),
        });
    }
    if let Some(dir) = std::env::var("OneDrive")
        .ok()
        .map(PathBuf::from)
        .filter(|d| d.is_dir())
    {
        out.push(FolderSuggestion {
            service: "OneDrive".into(),
            path: dir.join("Legato notes").to_string_lossy().into_owned(),
        });
    }
    out
}
