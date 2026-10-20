//! The desktop bubble's commands: its setting, hover, dragging and buttons.

use crate::bubble;
use tauri::AppHandle;

#[tauri::command]
#[specta::specta]
pub fn change_bubble_enabled_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    bubble::set_enabled(&app, enabled);
    Ok(())
}

/// The pointer came onto the bubble (true) or left it (false).
#[tauri::command]
#[specta::specta]
pub fn bubble_expand(app: AppHandle, expanded: bool) {
    bubble::expand(&app, expanded);
}

/// A drag in progress: the dot's centre in logical screen pixels.
#[tauri::command]
#[specta::specta]
pub fn bubble_move_to(app: AppHandle, x: f64, y: f64) {
    bubble::move_to(&app, x, y);
}

/// The drag ended.
#[tauri::command]
#[specta::specta]
pub fn bubble_drop(app: AppHandle) {
    bubble::drop_here(&app);
}

/// One of the bubble's buttons or menu items: `dictate`, `notes`,
/// `scratchpad`, `open` (Legato's window), `snooze` (an hour) or `off`.
/// Async because the Scratchpad's window may not exist yet, and building a
/// webview from a sync command, on the main thread, deadlocks on Windows.
#[tauri::command]
#[specta::specta]
pub async fn bubble_action(app: AppHandle, action: String) -> Result<(), String> {
    log::debug!("Bubble: {action}");
    match action.as_str() {
        "dictate" => crate::signal_handle::send_transcription_input(&app, "transcribe", "bubble"),
        "notes" => crate::notes::toggle(&app),
        "scratchpad" => crate::scratchpad::toggle_from_bubble(&app),
        "open" => crate::show_main_window(&app),
        "snooze" => bubble::snooze(&app),
        "off" => bubble::set_enabled(&app, false),
        other => return Err(format!("Unknown bubble action: {other}")),
    }
    Ok(())
}
