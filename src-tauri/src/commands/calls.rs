//! Commands behind Settings › Context & calls, the call card and the Notes
//! page's call strip.

use crate::calls::{self, CallAction, CallState};
use crate::settings::CallSettings;
use tauri::AppHandle;

#[tauri::command]
#[specta::specta]
pub fn get_call_state(app: AppHandle) -> CallState {
    calls::state(&app)
}

#[tauri::command]
#[specta::specta]
pub fn change_call_settings(app: AppHandle, calls: CallSettings) {
    calls::set_settings(&app, calls);
}

#[tauri::command]
#[specta::specta]
pub fn call_card_action(app: AppHandle, action: CallAction) {
    calls::act(&app, action);
}

#[tauri::command]
#[specta::specta]
pub fn test_call_card(app: AppHandle) -> Result<(), String> {
    calls::test_card(&app)
}
