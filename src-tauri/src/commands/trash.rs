use crate::managers::trash_store::{TrashItem, TrashKind};
use tauri::AppHandle;

#[tauri::command]
#[specta::specta]
pub fn list_trash(app: AppHandle) -> Result<Vec<TrashItem>, String> {
    crate::trash::list(&app).map_err(|e| e.to_string())
}

/// Puts a deleted note back; says which kind it was.
#[tauri::command]
#[specta::specta]
pub fn restore_trash(app: AppHandle, id: i64) -> Result<TrashKind, String> {
    crate::trash::restore(&app, id).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn delete_trash_forever(app: AppHandle, id: i64) -> Result<(), String> {
    crate::trash::delete_forever(&app, id).map_err(|e| e.to_string())
}

/// Deletes every note of a kind in Recently deleted for good; returns how
/// many.
#[tauri::command]
#[specta::specta]
pub fn empty_trash(app: AppHandle, kind: TrashKind) -> Result<u32, String> {
    crate::trash::empty(&app, kind)
        .map(|n| n as u32)
        .map_err(|e| e.to_string())
}
