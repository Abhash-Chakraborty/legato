//! Commands behind first-run setup: the microphone check, the model pick and
//! the practice scenes.

use crate::settings::{get_settings, write_settings};
use crate::setup::{self, PracticeMode};
use tauri::AppHandle;

/// Where dictations go while setup is open; Off when it closes.
#[tauri::command]
#[specta::specta]
pub fn set_practice_mode(mode: PracticeMode) {
    setup::set_mode(mode);
}

/// The practice scene's mic button: starts a dictation, or stops one, the way
/// the dictate shortcut does.
#[tauri::command]
#[specta::specta]
pub fn practice_toggle(app: AppHandle) {
    crate::signal_handle::send_transcription_input(&app, "transcribe", "setup");
}

#[tauri::command]
#[specta::specta]
pub fn start_mic_check(app: AppHandle) {
    setup::start_mic_check(&app);
}

#[tauri::command]
#[specta::specta]
pub fn stop_mic_check() {
    setup::stop_mic_check();
}

/// The Windows input languages, such as "en-US" and "de-DE".
#[tauri::command]
#[specta::specta]
pub fn get_input_languages() -> Vec<String> {
    setup::input_languages()
}

#[tauri::command]
#[specta::specta]
pub fn change_setup_step_setting(app: AppHandle, step: String) {
    let mut settings = get_settings(&app);
    settings.setup_step = step;
    write_settings(&app, settings);
}

/// Whether Legato may send keys to other apps and watch for its shortcuts.
/// A Mac asks the person first (Accessibility); Windows always allows it.
#[tauri::command]
#[specta::specta]
pub fn keyboard_access_granted() -> bool {
    #[cfg(target_os = "macos")]
    {
        crate::mac::accessibility_granted()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Asks macOS for that access: its own prompt adds Legato to the list in
/// System Settings, which opens on Accessibility. Nothing to ask elsewhere.
#[tauri::command]
#[specta::specta]
pub fn request_keyboard_access(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let _ = app;
        // Opening a connection for sending keys has macOS show its prompt
        // while access is missing; the connection itself isn't kept.
        if !crate::mac::accessibility_granted() {
            let _ = enigo::Enigo::new(&enigo::Settings::default());
        }
        crate::mac::open_accessibility_settings()
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Ok(())
    }
}
