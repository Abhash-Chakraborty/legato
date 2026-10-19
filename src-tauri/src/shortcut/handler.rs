//! Shared shortcut event handling logic
//!
//! This module contains the common logic for handling shortcut events,
//! used by both the Tauri and handy-keys implementations.

use log::warn;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

use crate::actions::action_for;
use crate::managers::audio::AudioRecordingManager;
use crate::settings::get_settings;
use crate::transcription_coordinator::is_transcribe_binding;
use crate::TranscriptionCoordinator;

/// Handle a shortcut event from either implementation.
///
/// This function contains the shared logic for:
/// - Looking up the action in ACTION_MAP
/// - Handling the cancel binding (only fires when recording)
/// - Routing transcribe bindings to the coordinator, which applies the
///   configured activation mode (toggle / push-to-talk / hold-or-toggle)
///
/// # Arguments
/// * `app` - The Tauri app handle
/// * `binding_id` - The ID of the binding (e.g., "transcribe", "cancel")
/// * `hotkey_string` - The string representation of the hotkey
/// * `is_pressed` - Whether this is a key press (true) or release (false)
pub fn handle_shortcut_event(
    app: &AppHandle,
    binding_id: &str,
    hotkey_string: &str,
    is_pressed: bool,
) {
    let settings = get_settings(app);

    // Tap to open the Scratchpad, hold to dictate into it.
    if crate::scratchpad::is_shortcut(binding_id) {
        crate::scratchpad::shortcut(app, hotkey_string, is_pressed);
        return;
    }

    // The transform picker's arrows, Enter and Esc, while it is open.
    if crate::shortcut::PICK_KEYS
        .iter()
        .any(|(id, _)| *id == binding_id)
    {
        if is_pressed {
            crate::transforms::pick_key(app, binding_id);
        }
        return;
    }

    // Transcribe bindings are handled by the coordinator.
    if is_transcribe_binding(binding_id) {
        if let Some(coordinator) = app.try_state::<TranscriptionCoordinator>() {
            coordinator.send_input(
                binding_id,
                hotkey_string,
                is_pressed,
                settings.shortcut_activation,
                std::time::Duration::from_millis(settings.hold_threshold_ms),
            );
        } else {
            warn!("TranscriptionCoordinator is not initialized");
        }
        return;
    }

    let Some(action) = action_for(binding_id) else {
        warn!(
            "No action for shortcut ID '{}'. Shortcut: '{}', Pressed: {}",
            binding_id, hotkey_string, is_pressed
        );
        return;
    };

    // Cancel binding: only fires when recording and key is pressed
    if binding_id == "cancel" {
        let audio_manager = app.state::<Arc<AudioRecordingManager>>();
        if audio_manager.is_recording() && is_pressed {
            action.start(app, binding_id, hotkey_string);
        }
        return;
    }

    // Remaining bindings (e.g. "test") use simple start/stop on press/release.
    if is_pressed {
        action.start(app, binding_id, hotkey_string);
    } else {
        action.stop(app, binding_id, hotkey_string);
    }
}
