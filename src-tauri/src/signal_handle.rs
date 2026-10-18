use crate::TranscriptionCoordinator;
use log::warn;
use tauri::{AppHandle, Manager};

/// Send a transcription input to the coordinator.
/// Used by CLI flags and any other external trigger.
pub fn send_transcription_input(app: &AppHandle, binding_id: &str, source: &str) {
    if let Some(c) = app.try_state::<TranscriptionCoordinator>() {
        c.send_external_input(binding_id, source);
    } else {
        warn!("TranscriptionCoordinator not initialized");
    }
}
