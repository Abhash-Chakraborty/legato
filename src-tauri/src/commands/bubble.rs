//! The desktop bubble's commands: its setting, hover, dragging and buttons.

use crate::bubble;
use tauri::AppHandle;

#[tauri::command]
#[specta::specta]
pub fn change_bubble_enabled_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    bubble::set_enabled(&app, enabled);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_live_caption_setting(
    app: AppHandle,
    place: crate::settings::LiveCaption,
) -> Result<(), String> {
    let mut settings = crate::settings::get_settings(&app);
    settings.live_caption = place;
    crate::settings::write_settings(&app, settings);
    Ok(())
}

/// Where the dot is in the bubble's window, for its page as it opens.
#[tauri::command]
#[specta::specta]
pub fn bubble_layout() -> Option<bubble::BubbleLayout> {
    bubble::layout()
}

/// The pointer came onto the bubble (true) or left it (false).
#[tauri::command]
#[specta::specta]
pub fn bubble_expand(app: AppHandle, expanded: bool) {
    bubble::expand(&app, expanded);
}

/// A press on the bubble: if the cursor moves with the button held, the
/// bubble follows it until the button comes up. `dx`, `dy`: the dot's
/// centre less the pointer, in logical pixels.
#[tauri::command]
#[specta::specta]
pub fn bubble_press(app: AppHandle, dx: f64, dy: f64) {
    bubble::press(&app, (dx, dy));
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
        // Starts a dictation, or while one records, finishes and pastes it.
        "dictate" => crate::signal_handle::send_transcription_input(&app, "transcribe", "bubble"),
        // Drops the dictation in progress, like the cancel key.
        "cancel" => crate::utils::cancel_current_operation(&app),
        // Starts notes and opens the panel; while they run, the panel,
        // which has Stop.
        "notes" => {
            if !crate::notes::is_recording() {
                crate::notes::toggle(&app);
            }
            if crate::notes::is_recording() {
                crate::notes_panel::show(&app);
            }
        }
        "stop-notes" => {
            if crate::notes::is_recording() {
                crate::notes::stop(&app).map_err(|e| e.to_string())?;
            }
        }
        "scratchpad" => crate::scratchpad::toggle_from_bubble(&app),
        "open" => crate::show_main_window(&app),
        "snooze" => bubble::snooze(&app),
        "off" => bubble::set_enabled(&app, false),
        other => return Err(format!("Unknown bubble action: {other}")),
    }
    Ok(())
}

/// Where the island's shape sits in its window, in physical pixels from the
/// window's corner, so clicks elsewhere pass through to the app below.
#[tauri::command]
#[specta::specta]
pub fn island_shape(x: f64, y: f64, width: f64, height: f64) {
    crate::overlay::island_hit::set_shape(x, y, width, height);
}

/// Opens the Notes side panel on the recording in progress (the bar's
/// Notes pill). Async for the same reason as `bubble_action`.
#[tauri::command]
#[specta::specta]
pub async fn show_notes_panel(app: AppHandle) {
    if crate::notes::is_recording() {
        crate::notes_panel::show(&app);
    }
}

/// Closes the Notes side panel; the recording carries on.
#[tauri::command]
#[specta::specta]
pub async fn hide_notes_panel(app: AppHandle) {
    crate::notes_panel::hide(&app);
}
