//! Settings › Devices: pairing a phone, what syncs and the shared clipboard.

use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::settings::{get_settings, write_settings, SyncSettings};
use crate::sync::ledger::{ClipRow, Merge};
use crate::sync::{PairingInfo, SyncManager, SyncState};

type Sync<'a> = State<'a, Arc<SyncManager>>;

fn err(e: anyhow::Error) -> String {
    format!("{e:#}")
}

#[tauri::command]
#[specta::specta]
pub fn get_sync_state(sync: Sync<'_>) -> Result<SyncState, String> {
    sync.state().map_err(err)
}

#[tauri::command]
#[specta::specta]
pub fn start_pairing(sync: Sync<'_>) -> Result<PairingInfo, String> {
    sync.inner().start_pairing().map_err(err)
}

/// Copies the pairing link without it reaching clipboard history, the cloud
/// clipboard or a connected phone: it holds the pairing secret.
#[tauri::command]
#[specta::specta]
pub fn copy_pairing_link(app: AppHandle, link: String) -> Result<(), String> {
    crate::sync::clipboard::copy_private(&app, &link).map_err(err)
}

#[tauri::command]
#[specta::specta]
pub fn cancel_pairing(sync: Sync<'_>) {
    sync.inner().cancel_pairing();
}

#[tauri::command]
#[specta::specta]
pub fn unpair_phone(sync: Sync<'_>, device_id: String) -> Result<(), String> {
    sync.inner().unpair(&device_id).map_err(err)
}

#[tauri::command]
#[specta::specta]
pub fn rename_phone(sync: Sync<'_>, device_id: String, name: String) -> Result<(), String> {
    sync.rename(&device_id, &name).map_err(err)
}

/// False when the phone isn't connected.
#[tauri::command]
#[specta::specta]
pub fn sync_phone_now(sync: Sync<'_>, device_id: String) -> Result<bool, String> {
    sync.sync_now(&device_id).map_err(err)
}

#[tauri::command]
#[specta::specta]
pub fn get_sync_merges(sync: Sync<'_>) -> Result<Vec<Merge>, String> {
    sync.merges().map_err(err)
}

#[tauri::command]
#[specta::specta]
pub fn change_sync_settings(
    app: AppHandle,
    sync: Sync<'_>,
    value: SyncSettings,
) -> Result<(), String> {
    let before = get_settings(&app);
    let mut after = before.clone();
    after.sync = SyncSettings {
        kinds: value
            .kinds
            .into_iter()
            .filter(|k| crate::sync::protocol::KINDS.contains(&k.as_str()))
            .collect(),
        pc_name: value
            .pc_name
            .map(|n| n.trim().chars().take(40).collect::<String>())
            .filter(|n| !n.is_empty()),
        clipboard_images_mb: value.clipboard_images_mb.min(10),
        clipboard_never: value
            .clipboard_never
            .into_iter()
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .collect(),
        ..value
    };
    write_settings(&app, after.clone());
    sync.settings_changed(&before, &after);
    Ok(())
}

/// Returns the phone's name.
#[tauri::command]
#[specta::specta]
pub fn send_clipboard_to_phone(sync: Sync<'_>) -> Result<String, String> {
    sync.send_clipboard_now().map_err(err)
}

#[tauri::command]
#[specta::specta]
pub fn get_recent_clips(sync: Sync<'_>) -> Result<Vec<ClipRow>, String> {
    sync.clips().map_err(err)
}

#[tauri::command]
#[specta::specta]
pub fn clear_recent_clips(sync: Sync<'_>) -> Result<(), String> {
    sync.clear_clips().map_err(err)
}

#[tauri::command]
#[specta::specta]
pub fn copy_recent_clip(sync: Sync<'_>, id: String) -> Result<(), String> {
    sync.copy_clip(&id).map_err(err)
}

#[tauri::command]
#[specta::specta]
pub fn get_clip_image(sync: Sync<'_>, id: String) -> Result<String, String> {
    sync.clip_image(&id).map_err(err)
}
