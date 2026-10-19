use crate::ai::AiStatusKind;
use crate::managers::transform_store::{self as store, TransformRun, TransformUsage};
use crate::settings::{
    ensure_transform_bindings, get_settings, write_settings, AppSettings, Transform,
    TransformOutput,
};
use crate::shortcut;
use crate::transform_rules::{self as rules, DEFAULT_DICTATE_TRANSFORM};
use crate::transforms::{self, TransformRunsChanged};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tauri_specta::Event;

const RECENT_RUNS: i64 = 200;
const MAX_MIN_WORDS: u32 = 30;
/// Try it waits as long as a transform on the selection.
const TRY_TIMEOUT: Duration = Duration::from_secs(30);

fn runs_db(app: &AppHandle) -> Result<rusqlite::Connection, String> {
    let path = crate::portable::app_data_dir(app)
        .map_err(|e| e.to_string())?
        .join("history.db");
    rusqlite::Connection::open(path).map_err(|e| e.to_string())
}

/// Saves settings and moves shortcut registrations to match.
fn commit(app: &AppHandle, before: &AppSettings, mut after: AppSettings) {
    ensure_transform_bindings(&mut after);
    write_settings(app, after.clone());
    shortcut::sync_bindings(app, before, &after);
}

#[derive(Serialize, Type)]
pub struct TransformRuns {
    pub runs: Vec<TransformRun>,
    pub usage: Vec<TransformUsage>,
}

#[tauri::command]
#[specta::specta]
pub fn get_transform_runs(app: AppHandle) -> Result<TransformRuns, String> {
    let conn = runs_db(&app)?;
    Ok(TransformRuns {
        runs: store::recent(&conn, RECENT_RUNS).map_err(|e| e.to_string())?,
        usage: store::usage(&conn).map_err(|e| e.to_string())?,
    })
}

#[tauri::command]
#[specta::specta]
pub fn clear_transform_runs(app: AppHandle) -> Result<(), String> {
    store::clear(&runs_db(&app)?).map_err(|e| e.to_string())?;
    let _ = TransformRunsChanged.emit(&app);
    Ok(())
}

/// Adds a transform, when its id is empty, or saves changes to one.
#[tauri::command]
#[specta::specta]
pub fn save_transform(app: AppHandle, transform: Transform) -> Result<Transform, String> {
    let mut transform = transform;
    transform.name = transform.name.trim().to_string();
    transform.description = transform.description.trim().to_string();
    if transform.name.is_empty() {
        return Err("A transform needs a name".to_string());
    }
    if transform.prompt.trim().is_empty() {
        return Err("A transform needs a prompt".to_string());
    }
    transform.model = transform
        .model
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty());
    if transform.model.is_none() {
        transform.provider_id = None;
    }

    let before = get_settings(&app);
    let mut after = before.clone();
    if transform.id.is_empty() {
        transform.id = format!("t{}", chrono::Utc::now().timestamp_millis());
        transform.built_in = false;
        after.transforms.push(transform.clone());
    } else {
        let existing = after
            .transforms
            .iter_mut()
            .find(|t| t.id == transform.id)
            .ok_or("That transform no longer exists")?;
        transform.built_in = existing.built_in;
        *existing = transform.clone();
    }
    if let Some(binding) = after.bindings.get_mut(&rules::binding_id(&transform.id)) {
        binding.name = transform.name.clone();
        binding.description = transform.description.clone();
    }
    commit(&app, &before, after);
    Ok(transform)
}

/// A deleted transform with what it held, so Undo can put it back.
#[derive(Serialize, Deserialize, Type)]
pub struct DeletedTransform {
    pub transform: Transform,
    pub index: u32,
    pub shortcut: String,
    pub was_auto: bool,
    pub was_dictate: bool,
}

#[tauri::command]
#[specta::specta]
pub fn delete_transform(app: AppHandle, id: String) -> Result<DeletedTransform, String> {
    let before = get_settings(&app);
    let mut after = before.clone();
    let index = after
        .transforms
        .iter()
        .position(|t| t.id == id)
        .ok_or("That transform no longer exists")?;
    if after.transforms[index].built_in {
        return Err("Built-in transforms can be switched off, not deleted".to_string());
    }
    let transform = after.transforms.remove(index);
    let shortcut = after
        .bindings
        .remove(&rules::binding_id(&id))
        .map(|b| b.current_binding)
        .unwrap_or_default();
    let was_auto = after.auto_transform_id.as_deref() == Some(id.as_str());
    if was_auto {
        after.auto_transform_id = None;
    }
    let was_dictate = after.dictate_transform_id == id;
    if was_dictate {
        after.dictate_transform_id = DEFAULT_DICTATE_TRANSFORM.to_string();
    }
    commit(&app, &before, after);
    Ok(DeletedTransform {
        transform,
        index: index as u32,
        shortcut,
        was_auto,
        was_dictate,
    })
}

/// Undo for a delete.
#[tauri::command]
#[specta::specta]
pub fn restore_transform(app: AppHandle, deleted: DeletedTransform) -> Result<(), String> {
    let before = get_settings(&app);
    let mut after = before.clone();
    if after.transform(&deleted.transform.id).is_some() {
        return Ok(());
    }
    let id = deleted.transform.id.clone();
    let index = (deleted.index as usize).min(after.transforms.len());
    after.transforms.insert(index, deleted.transform.clone());
    // Its old keys, unless something took them in the meantime.
    let taken = after
        .bindings
        .values()
        .any(|b| !deleted.shortcut.is_empty() && b.current_binding == deleted.shortcut);
    ensure_transform_bindings(&mut after);
    if !taken {
        if let Some(binding) = after.bindings.get_mut(&rules::binding_id(&id)) {
            binding.current_binding = deleted.shortcut.clone();
        }
    }
    if deleted.was_auto {
        after.auto_transform_id = Some(id.clone());
    }
    if deleted.was_dictate {
        after.dictate_transform_id = id;
    }
    commit(&app, &before, after);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn set_transform_enabled(app: AppHandle, id: String, enabled: bool) -> Result<(), String> {
    let before = get_settings(&app);
    let mut after = before.clone();
    let transform = after
        .transforms
        .iter_mut()
        .find(|t| t.id == id)
        .ok_or("That transform no longer exists")?;
    transform.enabled = enabled;
    commit(&app, &before, after);
    Ok(())
}

/// Moves a transform up or down the list, which is the picker's order.
#[tauri::command]
#[specta::specta]
pub fn move_transform(app: AppHandle, id: String, to: u32) -> Result<(), String> {
    let before = get_settings(&app);
    let mut after = before.clone();
    let from = after
        .transforms
        .iter()
        .position(|t| t.id == id)
        .ok_or("That transform no longer exists")?;
    let transform = after.transforms.remove(from);
    let to = (to as usize).min(after.transforms.len());
    after.transforms.insert(to, transform);
    write_settings(&app, after);
    Ok(())
}

/// Removes a transform's shortcut. Setting one goes through change_binding.
#[tauri::command]
#[specta::specta]
pub fn clear_transform_shortcut(app: AppHandle, id: String) -> Result<(), String> {
    let before = get_settings(&app);
    let mut after = before.clone();
    if let Some(binding) = after.bindings.get_mut(&rules::binding_id(&id)) {
        binding.current_binding = String::new();
    }
    commit(&app, &before, after);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_auto_transform_setting(app: AppHandle, id: Option<String>) -> Result<(), String> {
    let mut settings = get_settings(&app);
    if let Some(id) = &id {
        if settings.transform(id).is_none() {
            return Err("That transform no longer exists".to_string());
        }
    }
    settings.auto_transform_id = id;
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_dictate_transform_setting(app: AppHandle, id: String) -> Result<(), String> {
    let mut settings = get_settings(&app);
    if settings.transform(&id).is_none() {
        return Err("That transform no longer exists".to_string());
    }
    settings.dictate_transform_id = id;
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_auto_transform_skip_code_setting(
    app: AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.auto_transform_skip_code = enabled;
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_auto_transform_skip_ai_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.auto_transform_skip_ai = enabled;
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_auto_transform_min_words_setting(app: AppHandle, words: u32) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.auto_transform_min_words = words.min(MAX_MIN_WORDS);
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_ask_enabled_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let before = get_settings(&app);
    let mut after = before.clone();
    after.ask_enabled = enabled;
    commit(&app, &before, after);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_bar_confirm_paste_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.bar_confirm_paste = enabled;
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_bar_show_changes_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.bar_show_changes = enabled;
    write_settings(&app, settings);
    Ok(())
}

/// What Try it got back.
#[derive(Serialize, Type)]
pub struct TryOutcome {
    pub text: Option<String>,
    pub ms: u32,
    /// Why there is no text, when the provider didn't answer properly.
    pub problem: Option<AiStatusKind>,
}

/// Runs a transform on sample text without pasting it, for Try it. The
/// transform may be unsaved, as in the editor.
#[tauri::command]
#[specta::specta]
pub async fn try_transform(
    app: AppHandle,
    transform: Transform,
    text: String,
) -> Result<TryOutcome, String> {
    if text.trim().is_empty() {
        return Err("Type or paste some text first".to_string());
    }
    let settings = get_settings(&app);
    let started = Instant::now();
    let result = transforms::apply(&app, &settings, &transform, &text, None, TRY_TIMEOUT).await;
    let ms = started.elapsed().as_millis() as u32;
    Ok(match result {
        Ok(text) => TryOutcome {
            text,
            ms,
            problem: None,
        },
        Err(problem) => TryOutcome {
            text: None,
            ms,
            problem: Some(problem.kind),
        },
    })
}

/// A blank transform for the editor.
#[tauri::command]
#[specta::specta]
pub fn new_transform() -> Transform {
    Transform {
        id: String::new(),
        name: String::new(),
        description: String::new(),
        prompt: String::new(),
        built_in: false,
        enabled: true,
        provider_id: None,
        model: None,
        output: TransformOutput::Replace,
        show_changes: true,
    }
}

#[tauri::command]
#[specta::specta]
pub fn bar_undo(app: AppHandle) {
    transforms::undo(&app);
}

#[tauri::command]
#[specta::specta]
pub fn bar_keep(app: AppHandle) {
    transforms::keep(&app);
}

#[tauri::command]
#[specta::specta]
pub fn bar_pick(app: AppHandle, id: String) {
    transforms::pick(&app, &id);
}

#[tauri::command]
#[specta::specta]
pub fn bar_dismiss(app: AppHandle) {
    transforms::dismiss(&app);
}
