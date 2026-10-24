//! Commands behind the Scratchpad window and page.

use crate::managers::scratch_store::{DeletedNote, ScratchNote, ScratchVersion};
use crate::scratchpad::{self, ScratchpadManager, ScratchpadState, SendTarget};
use crate::settings::{get_settings, write_settings, ScratchpadOpen};
use std::sync::Arc;
use tauri::{AppHandle, State};

type Notes<'a> = State<'a, Arc<ScratchpadManager>>;

fn text<T>(result: anyhow::Result<T>) -> Result<T, String> {
    result.map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn get_scratch_notes(notes: Notes<'_>) -> Result<Vec<ScratchNote>, String> {
    text(notes.list())
}

#[tauri::command]
#[specta::specta]
pub fn get_scratchpad_state(app: AppHandle) -> Result<ScratchpadState, String> {
    text(scratchpad::state(&app))
}

/// A new, empty note, open as a tab.
#[tauri::command]
#[specta::specta]
pub fn new_scratch_note(app: AppHandle, notes: Notes<'_>) -> Result<ScratchNote, String> {
    let note = text(notes.blank_or_new())?;
    scratchpad::set_active(&app, Some(note.id));
    Ok(note)
}

#[tauri::command]
#[specta::specta]
pub fn save_scratch_note(notes: Notes<'_>, id: i64, text: String) -> Result<ScratchNote, String> {
    self::text(notes.save_text(id, &text))
}

#[tauri::command]
#[specta::specta]
pub fn rename_scratch_note(
    notes: Notes<'_>,
    id: i64,
    title: String,
) -> Result<ScratchNote, String> {
    text(notes.rename(id, &title))
}

#[tauri::command]
#[specta::specta]
pub fn set_scratch_note_pinned(
    notes: Notes<'_>,
    id: i64,
    pinned: bool,
) -> Result<ScratchNote, String> {
    text(notes.set_pinned(id, pinned))
}

/// Hides a failed paste's message.
#[tauri::command]
#[specta::specta]
pub fn dismiss_scratch_reason(notes: Notes<'_>, id: i64) -> Result<ScratchNote, String> {
    text(notes.dismiss_reason(id))
}

/// Deletes notes and returns them for Undo.
#[tauri::command]
#[specta::specta]
pub fn delete_scratch_notes(notes: Notes<'_>, ids: Vec<i64>) -> Result<Vec<DeletedNote>, String> {
    text(notes.delete(&ids))
}

#[tauri::command]
#[specta::specta]
pub fn restore_scratch_notes(notes: Notes<'_>, deleted: Vec<DeletedNote>) -> Result<(), String> {
    text(notes.restore(&deleted))
}

#[tauri::command]
#[specta::specta]
pub fn get_scratch_versions(notes: Notes<'_>, id: i64) -> Result<Vec<ScratchVersion>, String> {
    text(notes.versions(id))
}

#[tauri::command]
#[specta::specta]
pub fn restore_scratch_version(
    notes: Notes<'_>,
    id: i64,
    version_id: i64,
) -> Result<ScratchNote, String> {
    text(notes.restore_version(id, version_id))
}

/// Opens a note as a tab and makes it the one on screen.
#[tauri::command]
#[specta::specta]
pub fn open_scratch_tab(app: AppHandle, notes: Notes<'_>, id: i64) -> Result<(), String> {
    text(notes.open_tab(id))?;
    scratchpad::set_active(&app, Some(id));
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn close_scratch_tab(notes: Notes<'_>, id: i64) -> Result<(), String> {
    text(notes.close_tab(id))
}

#[tauri::command]
#[specta::specta]
pub fn move_scratch_tab(notes: Notes<'_>, id: i64, to: u32) -> Result<(), String> {
    text(notes.move_tab(id, to as usize))
}

/// Opens the window, on a note or the one Settings picks. Async, as every
/// command that may create a window must be: a sync one runs on the main
/// thread, and building a webview there deadlocks on Windows.
#[tauri::command]
#[specta::specta]
pub async fn open_scratchpad(app: AppHandle, note_id: Option<i64>) -> Result<(), String> {
    text(scratchpad::show(&app, note_id))
}

/// The windows Send to can paste into, front to back.
#[tauri::command]
#[specta::specta]
pub fn get_send_targets() -> Vec<SendTarget> {
    scratchpad::send_targets()
}

/// Points Send to at another window.
#[tauri::command]
#[specta::specta]
pub fn set_send_target(handle: i64) -> Result<(), String> {
    if scratchpad::set_send_target(handle) {
        Ok(())
    } else {
        Err("That window has closed.".into())
    }
}

#[tauri::command]
#[specta::specta]
pub fn hide_scratchpad(app: AppHandle) {
    scratchpad::hide(&app);
}

/// Starts or stops Dictate here for a note.
#[tauri::command]
#[specta::specta]
pub fn scratchpad_dictate(app: AppHandle, note_id: i64) {
    scratchpad::dictate_here(&app, note_id);
}

/// Pastes a note into the app that was in front when the window opened,
/// and returns that app's name.
#[tauri::command]
#[specta::specta]
pub async fn send_scratch_note(app: AppHandle, text: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || scratchpad::send_to_app(&app, text))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
#[specta::specta]
pub fn change_scratchpad_keep_on_top_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    scratchpad::set_keep_on_top(&app, enabled);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_scratchpad_open_setting(app: AppHandle, open: ScratchpadOpen) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.scratchpad_open = open;
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_scratchpad_failed_pastes_setting(
    app: AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.scratchpad_failed_pastes = enabled;
    write_settings(&app, settings);
    Ok(())
}
