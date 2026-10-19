use crate::actions::{process_transcription_output, DictationMode};
use crate::managers::{
    history::{HistoryManager, HistoryQuery, HistorySummary, PaginatedHistory},
    model::ModelManager,
    transcription::TranscriptionManager,
};
use base64::Engine;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, State};

/// History rows, newest first, narrowed by search, filter and app.
#[tauri::command]
#[specta::specta]
pub async fn get_history_entries(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    query: HistoryQuery,
) -> Result<PaginatedHistory, String> {
    history_manager.query(&query).map_err(|e| e.to_string())
}

/// Today's totals, the filter counts and the apps for Home's header.
#[tauri::command]
#[specta::specta]
pub async fn get_history_summary(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
) -> Result<HistorySummary, String> {
    history_manager.summary().map_err(|e| e.to_string())
}

/// Copies a dictation's recording to where the person chose to save it.
#[tauri::command]
#[specta::specta]
pub async fn save_history_audio(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    id: i64,
    destination: String,
) -> Result<(), String> {
    let entry = history_manager
        .get_entry_by_id(id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("History entry {} not found", id))?;
    let source = history_manager.get_audio_file_path(&entry.file_name);
    std::fs::copy(&source, &destination)
        .map(|_| ())
        .map_err(|e| format!("Couldn't save the recording: {e}"))
}

/// An app's icon as a PNG data URL, or None when it has none. Cached per
/// executable for the life of the app.
#[tauri::command]
#[specta::specta]
pub async fn get_app_icon(path: String) -> Option<String> {
    static ICONS: Mutex<Option<HashMap<String, Option<String>>>> = Mutex::new(None);
    if let Some(hit) = ICONS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(HashMap::new)
        .get(&path)
    {
        return hit.clone();
    }
    let lookup = path.clone();
    let icon = tauri::async_runtime::spawn_blocking(move || crate::context::app_icon_png(&lookup))
        .await
        .ok()
        .flatten()
        .map(|png| {
            format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(png)
            )
        });
    ICONS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(HashMap::new)
        .insert(path, icon.clone());
    icon
}

#[tauri::command]
#[specta::specta]
pub async fn toggle_history_entry_saved(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    id: i64,
) -> Result<(), String> {
    history_manager
        .toggle_saved_status(id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_audio_file_path(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    file_name: String,
) -> Result<String, String> {
    let path = history_manager.get_audio_file_path(&file_name);
    path.to_str()
        .ok_or_else(|| "Invalid file path".to_string())
        .map(|s| s.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_history_entry(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    id: i64,
) -> Result<(), String> {
    history_manager
        .delete_entry(id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn retry_history_entry_transcription(
    app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    transcription_manager: State<'_, Arc<TranscriptionManager>>,
    id: i64,
) -> Result<(), String> {
    let entry = history_manager
        .get_entry_by_id(id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("History entry {} not found", id))?;

    let audio_path = history_manager.get_audio_file_path(&entry.file_name);
    let samples = crate::audio_toolkit::read_wav_samples(&audio_path)
        .map_err(|e| format!("Failed to load audio: {}", e))?;

    if samples.is_empty() {
        return Err("Recording has no audio samples".to_string());
    }

    transcription_manager.initiate_model_load();

    let tm = Arc::clone(&transcription_manager);
    let transcription = tauri::async_runtime::spawn_blocking(move || tm.transcribe(samples))
        .await
        .map_err(|e| format!("Transcription task panicked: {}", e))?
        .map_err(|e| e.to_string())?;

    if transcription.text.is_empty() {
        return Err("Recording contains no speech".to_string());
    }

    // A retry runs the text through the same steps as the dictation did,
    // with the style of the app it went to.
    let mode = if entry.post_process_requested {
        DictationMode::Transform
    } else {
        DictationMode::Plain
    };
    let target = entry.app_path.clone().map(|exe_path| {
        crate::context::resolve(&crate::context::ForegroundWindow {
            exe_path,
            title: entry.app_title.clone().unwrap_or_default(),
        })
    });
    let processed =
        process_transcription_output(&app, &transcription.text, mode, target.as_ref()).await;
    let fixes = processed.all_fixes(&transcription.fixes);
    let model_id = crate::settings::get_settings(&app).selected_model;
    let model_name = app
        .state::<Arc<ModelManager>>()
        .get_model_info(&model_id)
        .map(|m| m.name)
        .unwrap_or_else(|| model_id.clone());
    history_manager
        .update_transcription(
            id,
            processed.transcription,
            processed.post_processed_text,
            processed.post_process_prompt,
            processed.transform,
            Some((model_id, model_name)),
            &fixes,
            Some(&transcription.heard),
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn update_history_limit(
    app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    limit: usize,
) -> Result<(), String> {
    let mut settings = crate::settings::get_settings(&app);
    settings.history_limit = limit;
    crate::settings::write_settings(&app, settings);

    history_manager
        .cleanup_old_entries()
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn update_recording_retention_period(
    app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    period: String,
) -> Result<(), String> {
    use crate::settings::RecordingRetentionPeriod;

    let retention_period = match period.as_str() {
        "never" => RecordingRetentionPeriod::Never,
        "preserve_limit" => RecordingRetentionPeriod::PreserveLimit,
        "days3" => RecordingRetentionPeriod::Days3,
        "weeks2" => RecordingRetentionPeriod::Weeks2,
        "months3" => RecordingRetentionPeriod::Months3,
        _ => return Err(format!("Invalid retention period: {}", period)),
    };

    let mut settings = crate::settings::get_settings(&app);
    settings.recording_retention_period = retention_period;
    crate::settings::write_settings(&app, settings);

    history_manager
        .cleanup_old_entries()
        .map_err(|e| e.to_string())?;

    Ok(())
}
