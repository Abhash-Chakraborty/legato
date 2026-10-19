use crate::ai::{self, AiStatus, AiTestResult};
use crate::secrets;
use crate::settings::{get_settings, write_settings};
use std::collections::HashMap;
use tauri::AppHandle;

/// The longest the AI may take before a dictation goes out without it.
const TIMEOUT_CHOICES: [u32; 4] = [3, 6, 10, 20];
const MAX_MIN_WORDS: u32 = 30;

#[tauri::command]
#[specta::specta]
pub fn get_ai_status(app: AppHandle) -> AiStatus {
    ai::status(&get_settings(&app))
}

#[tauri::command]
#[specta::specta]
pub async fn test_ai_connection(app: AppHandle) -> AiTestResult {
    ai::test_connection(&app).await
}

/// The last four characters of each saved key.
#[tauri::command]
#[specta::specta]
pub fn get_ai_key_hints(app: AppHandle) -> HashMap<String, String> {
    secrets::key_hints(&get_settings(&app))
}

#[tauri::command]
#[specta::specta]
pub fn change_ai_correction_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.ai_correction_enabled = enabled;
    write_settings(&app, settings);
    ai::notify(&app);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_ai_notes_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.ai_notes_enabled = enabled;
    write_settings(&app, settings);
    ai::notify(&app);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_ai_min_words_setting(app: AppHandle, words: u32) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.ai_min_words = words.min(MAX_MIN_WORDS);
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_ai_timeout_setting(app: AppHandle, seconds: u32) -> Result<(), String> {
    if !TIMEOUT_CHOICES.contains(&seconds) {
        return Err(format!("Unsupported AI time limit: {seconds} seconds"));
    }
    let mut settings = get_settings(&app);
    settings.ai_timeout_secs = seconds;
    write_settings(&app, settings);
    Ok(())
}
