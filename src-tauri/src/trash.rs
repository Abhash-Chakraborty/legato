//! Recently deleted: notes and Scratchpad notes wait here for
//! `trash_store::KEEP_DAYS` after they're deleted, then go for good.

use crate::managers::notes_store::DeletedRecording;
use crate::managers::scratch_store::DeletedNote;
use crate::managers::trash_store::{self, Taken, TrashItem, TrashKind};
use anyhow::{anyhow, Result};
use log::{error, info, warn};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::AppHandle;
use tauri_specta::Event;

/// Sent when something goes into Recently deleted or leaves it.
#[derive(Clone, Debug, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct TrashChanged;

pub fn changed(app: &AppHandle) {
    if let Err(e) = TrashChanged.emit(app) {
        error!("Failed to emit trash-changed: {e}");
    }
}

fn conn(app: &AppHandle) -> Result<Connection> {
    Ok(Connection::open(
        crate::portable::app_data_dir(app)?.join("history.db"),
    )?)
}

pub fn list(app: &AppHandle) -> Result<Vec<TrashItem>> {
    // Legato can run for weeks; what's past its 30 days goes now, not at
    // the next start.
    purge_expired(app);
    trash_store::list(&conn(app)?)
}

/// Puts an item back where it was deleted from, with its id. It stays in
/// Recently deleted if that fails.
pub fn restore(app: &AppHandle, id: i64) -> Result<TrashKind> {
    let Some(item) = trash_store::peek(&conn(app)?, id)? else {
        return Err(anyhow!("It's no longer in Recently deleted"));
    };
    // Each manager takes the item out of Recently deleted once it's back.
    match item.kind {
        TrashKind::Note => {
            let deleted: DeletedRecording = serde_json::from_str(&item.payload)?;
            crate::notes::manager(app).restore(&deleted)?;
        }
        TrashKind::Scratch => {
            let deleted: DeletedNote = serde_json::from_str(&item.payload)?;
            crate::scratchpad::restore_deleted(app, &[deleted])?;
        }
    }
    Ok(item.kind)
}

/// Removes what leaves Recently deleted for good: a note's audio.
fn remove_for_good(app: &AppHandle, taken: &Taken) {
    if taken.kind == TrashKind::Note {
        match serde_json::from_str::<DeletedRecording>(&taken.payload) {
            Ok(d) => crate::notes::manager(app).remove_deleted_audio(&d),
            Err(e) => warn!("A deleted note couldn't be read to remove its audio: {e}"),
        }
    }
}

pub fn delete_forever(app: &AppHandle, id: i64) -> Result<()> {
    if let Some(taken) = trash_store::take(&conn(app)?, id)? {
        remove_for_good(app, &taken);
    }
    changed(app);
    Ok(())
}

/// Deletes every item of a kind for good.
pub fn empty(app: &AppHandle, kind: TrashKind) -> Result<usize> {
    let c = conn(app)?;
    let mut removed = 0;
    for item in trash_store::list(&c)?
        .into_iter()
        .filter(|i| i.kind == kind)
    {
        if let Some(taken) = trash_store::take(&c, item.id)? {
            remove_for_good(app, &taken);
            removed += 1;
        }
    }
    changed(app);
    Ok(removed)
}

/// Removes what was deleted more than `KEEP_DAYS` ago; runs at startup.
pub fn purge_expired(app: &AppHandle) {
    let before = chrono::Utc::now().timestamp() - trash_store::KEEP_DAYS * 86_400;
    let taken = match conn(app).and_then(|c| trash_store::take_older(&c, Some(before))) {
        Ok(taken) => taken,
        Err(e) => {
            warn!("Couldn't clear old items from Recently deleted: {e}");
            return;
        }
    };
    if taken.is_empty() {
        return;
    }
    for t in &taken {
        remove_for_good(app, t);
    }
    info!(
        "Removed {} items deleted more than {} days ago",
        taken.len(),
        trash_store::KEEP_DAYS
    );
    changed(app);
}
