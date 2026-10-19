//! Transforms at work: on the selection (each transform's shortcut and the
//! Win+Alt+T picker), on a dictation, and Ask. Also what the Legato bar says
//! afterwards, its Keep and Undo, View last changes, and Paste or Copy last
//! transcript.

use crate::ai::{self, AiConfig, AiProblem, AiStatusKind};
use crate::context::{self, AppInfo};
use crate::managers::dictionary::DictionaryManager;
use crate::managers::dictionary_store::Learned;
use crate::managers::history::HistoryManager;
use crate::managers::transform_store::{self as store, NewRun, RunResult, RunSource};
use crate::selection;
use crate::settings::{get_settings, AppSettings, PasteMethod, Transform, TransformOutput};
use crate::transform_rules::{self as rules, PromptContext};
use crate::{overlay, tray};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_specta::Event;

/// How long a transform on the selection may take. Longer than the
/// dictation limit in Settings › AI: nothing is pasted until it answers, the
/// selection stays as it was if it doesn't, and the bar's cancel stops it.
const SELECTION_TIMEOUT: Duration = Duration::from_secs(30);

/// How long the bar shows a transform's changes before keeping them.
const CHANGES_MS: u64 = 15_000;
/// How long the bar confirms a paste.
const CONFIRM_MS: u64 = 2500;
/// How long the bar shows a problem.
const PROBLEM_MS: u64 = 5000;
/// How long the picker waits for a choice.
const PICK_MS: u64 = 12_000;

const VK_RIGHT: u32 = 0x27;
const VK_Z: u32 = 0x5A;

static BUSY: AtomicBool = AtomicBool::new(false);
/// Bumped by cancel, so a run that is still waiting for the model doesn't
/// paste afterwards.
static RUN_GENERATION: AtomicU64 = AtomicU64::new(0);

/// Sent when a run is added or its result changes.
#[derive(Clone, Debug, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct TransformRunsChanged;

/// What the bar says. The overlay words each kind itself.
#[derive(Serialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BarNotice {
    /// A dictation was pasted.
    Pasted {
        words: usize,
        app: Option<String>,
        undo: bool,
    },
    /// A transform's result was pasted.
    Transformed {
        name: String,
        words: usize,
        undo: bool,
    },
    /// A transform's result went to the clipboard.
    Copied {
        name: String,
    },
    PastedLast {
        words: usize,
    },
    CopiedLast {
        words: usize,
    },
    /// Undo was sent; with `clipboard`, the original is on the clipboard too.
    Undone {
        clipboard: bool,
    },
    /// No selection and no dictation to work on.
    NothingToTransform,
    /// A clip from a paired phone is on the clipboard.
    FromPhone {
        name: String,
        image: bool,
    },
    NoLastDictation,
    NoChanges,
    /// Ask heard no instruction.
    AskEmpty,
    /// The paste failed, so the text went to the Scratchpad. `admin` when the
    /// app runs as administrator.
    SavedToScratchpad {
        app: Option<String>,
        admin: bool,
        /// The note it went into, for the bar's Open.
        note_id: Option<i64>,
    },
    /// Words corrected after a paste wait under Needs review; `more` counts
    /// the others found with `word`.
    Spotted {
        word: String,
        more: usize,
        app: Option<String>,
    },
    /// Words corrected after a paste went straight into the Dictionary.
    Learned {
        word: String,
        more: usize,
        app: Option<String>,
    },
    /// A Notes recording is transcribed.
    NoteReady {
        id: i64,
        duration_ms: i64,
    },
    /// A Notes recording heard nothing, so no note was kept.
    NoteEmpty {
        duration_ms: i64,
    },
    /// Never here on the call card: no more offers in this app, with Undo.
    CallNever {
        app: String,
    },
    /// Start or stop notes didn't work.
    NotesFailed {
        reason: String,
    },
    /// A transform didn't run; the text stayed as it was.
    TransformFailed {
        name: String,
        problem: AiStatusKind,
        provider_id: String,
        provider_label: String,
    },
}

/// The bar's changes state.
#[derive(Serialize, Clone, Debug)]
struct BarChanges {
    name: String,
    before: String,
    after: String,
    /// The View last changes shortcut, to bring it back.
    keys: String,
}

/// One entry in the bar's transform picker.
#[derive(Serialize, Clone, Debug)]
struct PickItem {
    id: String,
    name: String,
    keys: String,
}

/// What a transform changed, kept for View last changes and Undo.
#[derive(Clone)]
struct Change {
    run_id: Option<i64>,
    name: String,
    before: String,
    after: String,
}

/// What the bar's Undo undoes.
enum Undoable {
    Paste {
        run_id: Option<i64>,
    },
    Change(Change),
    /// Words Learn from my edits added on its own.
    Learned(Vec<Learned>),
}

static LAST_CHANGE: Mutex<Option<Change>> = Mutex::new(None);
static UNDO: Mutex<Option<Undoable>> = Mutex::new(None);
static PICKED: Mutex<Option<Captured>> = Mutex::new(None);
/// The picker's transforms in order, and the one the arrow keys are on.
static PICK_IDS: Mutex<Vec<String>> = Mutex::new(Vec::new());
static PICK_INDEX: AtomicUsize = AtomicUsize::new(0);
/// Bumped each time the picker opens, so an old timer leaves a new one be.
static PICK_GENERATION: AtomicU64 = AtomicU64::new(0);

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Text a transform runs on, and where it came from.
struct Captured {
    text: String,
    source: RunSource,
    app: Option<AppInfo>,
    /// The text is selected in the app, so a paste replaces it.
    selected: bool,
    /// Whitespace after the selected text that the result keeps, such as
    /// the trailing space a dictation was pasted with.
    suffix: String,
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn foreground_app() -> Option<AppInfo> {
    context::foreground_window().map(|w| context::resolve(&w))
}

/// The text of the last dictation, as it was pasted.
fn last_dictation(app: &AppHandle) -> Option<String> {
    let history = app.try_state::<Arc<HistoryManager>>()?;
    match history.get_latest_completed_entry() {
        Ok(Some(entry)) => {
            let text = tray::last_transcript_text(&entry).trim().to_string();
            (!text.is_empty()).then_some(text)
        }
        Ok(None) => None,
        Err(e) => {
            error!("Couldn't read the last dictation: {e}");
            None
        }
    }
}

/// The selection in the app in front, or the last dictation when nothing
/// is selected.
fn capture(app: &AppHandle) -> Option<Captured> {
    let app_info = foreground_app();
    let selected = match selection::copy_selection(app) {
        Ok(text) => text.filter(|t| !t.trim().is_empty()),
        Err(e) => {
            warn!("Couldn't copy the selection: {e}");
            None
        }
    };
    if let Some(text) = selected {
        return Some(Captured {
            text,
            source: RunSource::Selection,
            app: app_info,
            selected: true,
            suffix: String::new(),
        });
    }
    if let Some(pasted) = reselect_last_paste(app, app_info.as_ref()) {
        let text = pasted.trim_end().to_string();
        return Some(Captured {
            suffix: pasted[text.len()..].to_string(),
            text,
            source: RunSource::LastDictation,
            app: app_info,
            selected: true,
        });
    }
    // Pasted elsewhere or too long ago: the result goes in at the cursor.
    last_dictation(app).map(|text| Captured {
        text,
        source: RunSource::LastDictation,
        app: app_info,
        selected: false,
        suffix: String::new(),
    })
}

/// How long after a paste a transform can still select it again.
const RESELECT_WITHIN: Duration = Duration::from_secs(600);
/// Longer pastes aren't selected back key by key.
const MAX_RESELECT_CHARS: usize = 1500;

/// Selects the last paste again when it went into the window in front and
/// still sits right before the cursor, checked by copying the selection.
/// Otherwise the cursor goes back where it was. Terminals are left alone:
/// Shift+Left and Ctrl+C mean other things there.
fn reselect_last_paste(app: &AppHandle, info: Option<&AppInfo>) -> Option<String> {
    let pasted = crate::clipboard::last_paste_here(RESELECT_WITHIN)?;
    if info.is_some_and(|i| rules::is_terminal(&i.exe_path)) {
        return None;
    }
    let count = pasted.chars().filter(|c| *c != '\r').count();
    if count == 0 || count > MAX_RESELECT_CHARS {
        return None;
    }
    if let Err(e) = selection::select_back(app, count) {
        warn!("Couldn't select the last paste: {e}");
        return None;
    }
    std::thread::sleep(Duration::from_millis(30));
    match selection::copy_selection(app) {
        Ok(Some(copied)) if rules::same_text(&copied, &pasted) => Some(pasted),
        Ok(Some(_)) => {
            debug!("The text before the cursor isn't the last paste any more");
            // Collapses the selection to its end, where the cursor was.
            let _ = selection::send_key(app, VK_RIGHT);
            None
        }
        // Nothing selected, so nothing to undo.
        _ => None,
    }
}

fn runs_db(app: &AppHandle) -> Option<rusqlite::Connection> {
    let path = crate::portable::app_data_dir(app).ok()?.join("history.db");
    rusqlite::Connection::open(path)
        .map_err(|e| error!("Couldn't open the history database: {e}"))
        .ok()
}

fn record_run(app: &AppHandle, run: NewRun) -> Option<i64> {
    let id = runs_db(app).and_then(|conn| {
        store::insert(&conn, &run)
            .map_err(|e| error!("Couldn't record the transform run: {e}"))
            .ok()
    });
    let _ = TransformRunsChanged.emit(app);
    id
}

fn set_run_result(app: &AppHandle, id: i64, result: RunResult) {
    if let Some(conn) = runs_db(app) {
        if let Err(e) = store::set_result(&conn, id, result) {
            error!("Couldn't update the transform run: {e}");
        }
    }
    let _ = TransformRunsChanged.emit(app);
}

/// Pastes on the main thread, which owns the clipboard window the reliable
/// paste uses, and waits for it.
fn paste_on_main(app: &AppHandle, text: String) -> Result<(), String> {
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let _ = tx.send(crate::clipboard::paste_plain(text, handle));
    })
    .map_err(|e| e.to_string())?;
    rx.recv_timeout(Duration::from_secs(10))
        .map_err(|_| "The paste didn't finish".to_string())?
}

fn binding_keys(settings: &AppSettings, id: &str) -> String {
    settings
        .bindings
        .get(id)
        .map(|b| b.current_binding.clone())
        .unwrap_or_default()
}

fn problem_notice(name: &str, problem: &AiProblem) -> BarNotice {
    BarNotice::TransformFailed {
        name: name.to_string(),
        problem: problem.kind,
        provider_id: problem.provider_id.clone(),
        provider_label: problem.provider_label.clone(),
    }
}

/// The problem in the words Recent runs shows.
fn problem_text(problem: &AiProblem) -> String {
    let name = if problem.provider_id == "custom" {
        "Your server".to_string()
    } else {
        problem.provider_label.clone()
    };
    match problem.kind {
        AiStatusKind::None => "No AI model is set up".to_string(),
        AiStatusKind::Unreachable if problem.provider_id == "ollama" => {
            "Ollama isn't running".to_string()
        }
        AiStatusKind::Unreachable => format!("{name} isn't reachable"),
        AiStatusKind::Rejected => format!("{name} rejected the request"),
        AiStatusKind::NotFound => format!("{name} doesn't have that model"),
        AiStatusKind::RateLimited => format!("{name} is rate limiting"),
        AiStatusKind::Slow => format!("{name} didn't answer in time"),
        _ => format!("{name} didn't answer properly"),
    }
}

/// Dictionary words for prompts that ask for them, most used first.
fn dictionary_words(app: &AppHandle) -> Vec<String> {
    let Some(dictionary) = app.try_state::<Arc<DictionaryManager>>() else {
        return Vec::new();
    };
    let mut entries = dictionary.list().unwrap_or_default();
    entries.sort_by(|a, b| b.pinned.cmp(&a.pinned).then(b.hits.cmp(&a.hits)));
    entries
        .into_iter()
        .take(rules::PROMPT_DICTIONARY_LIMIT)
        .map(|e| e.text)
        .collect()
}

/// Runs a transform's prompt on `text`. Ok(None) means the model answered
/// with nothing.
pub async fn apply(
    app: &AppHandle,
    settings: &AppSettings,
    transform: &Transform,
    text: &str,
    app_name: Option<&str>,
    timeout: Duration,
) -> Result<Option<String>, AiProblem> {
    let Some(config) = AiConfig::for_transform(settings, transform) else {
        return Err(AiProblem::not_set_up(
            settings,
            transform.provider_id.as_deref(),
        ));
    };
    let words = if rules::wants_dictionary(&transform.prompt) {
        dictionary_words(app)
    } else {
        Vec::new()
    };
    let clipboard = if rules::wants_clipboard(&transform.prompt) {
        app.clipboard().read_text().ok()
    } else {
        None
    };
    let system = rules::system_prompt(
        &transform.prompt,
        &PromptContext {
            app: app_name,
            dictionary: &words,
            clipboard: clipboard.as_deref(),
        },
    );
    debug!(
        "Running transform '{}' on {} ({})",
        transform.id, config.provider.id, config.model
    );
    let answer = ai::run(app, &config, &system, text.to_string(), timeout).await?;
    let answer = rules::clean_answer(&answer, text);
    Ok((!answer.is_empty()).then_some(answer))
}

/// A request to run on captured text.
struct Job {
    transform_id: String,
    name: String,
    output: TransformOutput,
    show_changes: bool,
}

/// Runs a job's request, puts the result where the job says and tells the
/// bar. `request` gets the captured text and returns the answer.
fn execute<F>(app: &AppHandle, job: Job, captured: Captured, request: F)
where
    F: std::future::Future<Output = Result<Option<String>, AiProblem>>,
{
    let generation = RUN_GENERATION.load(Ordering::SeqCst);
    let settings = get_settings(app);
    overlay::set_working_label(app, Some(&job.name));
    overlay::show_processing_overlay(app);
    let started = Instant::now();
    let answer = tauri::async_runtime::block_on(request);
    let ms = started.elapsed().as_millis() as i64;
    overlay::set_working_label(app, None);
    if RUN_GENERATION.load(Ordering::SeqCst) != generation {
        debug!("Transform '{}' was cancelled", job.transform_id);
        return;
    }

    let mut run = NewRun {
        at: now(),
        transform_id: job.transform_id.clone(),
        name: job.name.clone(),
        source: Some(captured.source),
        app_name: captured.app.as_ref().map(|a| a.name.clone()),
        app_path: captured.app.as_ref().map(|a| a.exe_path.clone()),
        in_words: rules::word_count(&captured.text) as i64,
        ms,
        ..Default::default()
    };

    let result = match answer {
        Ok(Some(result)) => result,
        other => {
            let problem = match other {
                Err(problem) => problem,
                _ => AiProblem {
                    kind: AiStatusKind::Failed,
                    provider_id: settings.post_process_provider_id.clone(),
                    provider_label: String::new(),
                },
            };
            warn!(
                "Transform '{}' didn't run: {:?}",
                job.transform_id, problem.kind
            );
            run.result = Some(RunResult::Failed);
            run.error = Some(problem_text(&problem));
            record_run(app, run);
            overlay::show_overlay_notice_for(app, &problem_notice(&job.name, &problem), PROBLEM_MS);
            return;
        }
    };
    run.out_words = rules::word_count(&result) as i64;

    let copy_only =
        job.output == TransformOutput::Copy || settings.paste_method == PasteMethod::None;
    if copy_only {
        if let Err(e) = app.clipboard().write_text(result.clone()) {
            error!("Couldn't put the transform's result on the clipboard: {e}");
        }
        run.result = Some(RunResult::Copied);
        let run_id = record_run(app, run);
        remember_change(run_id, &job.name, &captured.text, &result);
        overlay::show_overlay_notice(app, &BarNotice::Copied { name: job.name });
        return;
    }

    let text = if job.output == TransformOutput::Below && captured.selected {
        if let Err(e) = selection::send_key(app, VK_RIGHT) {
            warn!("Couldn't move past the selection: {e}");
        }
        std::thread::sleep(Duration::from_millis(30));
        format!("\n\n{result}")
    } else {
        format!("{result}{}", captured.suffix)
    };
    if let Err(e) = paste_on_main(app, text) {
        error!("Couldn't paste the transform's result: {e}");
        let _ = app.clipboard().write_text(result.clone());
        run.result = Some(RunResult::Copied);
        record_run(app, run);
        overlay::show_overlay_notice(app, &BarNotice::Copied { name: job.name });
        return;
    }
    run.result = Some(RunResult::Kept);
    let run_id = record_run(app, run);
    let change = remember_change(run_id, &job.name, &captured.text, &result);
    if job.show_changes && settings.bar_show_changes && job.output == TransformOutput::Replace {
        show_changes(app, &settings, change);
    } else {
        *lock(&UNDO) = Some(Undoable::Change(change));
        overlay::show_overlay_notice_for(
            app,
            &BarNotice::Transformed {
                name: job.name,
                words: rules::word_count(&result),
                undo: true,
            },
            CONFIRM_MS,
        );
    }
}

fn remember_change(run_id: Option<i64>, name: &str, before: &str, after: &str) -> Change {
    let change = Change {
        run_id,
        name: name.to_string(),
        before: before.to_string(),
        after: after.to_string(),
    };
    *lock(&LAST_CHANGE) = Some(change.clone());
    change
}

fn show_changes(app: &AppHandle, settings: &AppSettings, change: Change) {
    let payload = BarChanges {
        name: change.name.clone(),
        before: change.before.clone(),
        after: change.after.clone(),
        keys: binding_keys(settings, "view_changes"),
    };
    *lock(&UNDO) = Some(Undoable::Change(change));
    overlay::show_bar(app, "changes", "bar-changes", &payload, Some(CHANGES_MS));
}

/// Runs `work` on its own thread once the shortcut's keys are up, unless
/// another transform is already running.
fn on_shortcut(app: &AppHandle, work: impl FnOnce(&AppHandle) + Send + 'static) {
    if BUSY.swap(true, Ordering::SeqCst) {
        return;
    }
    selection::mask_menu_key(app);
    let app = app.clone();
    std::thread::spawn(move || {
        selection::wait_for_keys_up();
        work(&app);
        BUSY.store(false, Ordering::SeqCst);
    });
}

fn run_transform_on(app: &AppHandle, transform: Transform, captured: Captured) {
    let settings = get_settings(app);
    let app_name = captured.app.as_ref().map(|a| a.name.clone());
    let text = captured.text.clone();
    let job = Job {
        transform_id: transform.id.clone(),
        name: transform.name.clone(),
        output: transform.output,
        show_changes: transform.show_changes,
    };
    let handle = app.clone();
    execute(app, job, captured, async move {
        apply(
            &handle,
            &settings,
            &transform,
            &text,
            app_name.as_deref(),
            SELECTION_TIMEOUT,
        )
        .await
    });
}

/// A transform's shortcut: runs it on the selection, or on the last
/// dictation when nothing is selected.
pub fn run_shortcut(app: &AppHandle, transform_id: &str) {
    let Some(transform) = get_settings(app).transform(transform_id).cloned() else {
        warn!("No transform '{transform_id}'");
        return;
    };
    on_shortcut(app, move |app| match capture(app) {
        Some(captured) => run_transform_on(app, transform, captured),
        None => overlay::show_overlay_notice(app, &BarNotice::NothingToTransform),
    });
}

/// Transform the selection: copies it, then lets the bar's picker choose.
pub fn pick_for_selection(app: &AppHandle) {
    on_shortcut(app, |app| {
        let Some(captured) = capture(app) else {
            overlay::show_overlay_notice(app, &BarNotice::NothingToTransform);
            return;
        };
        let settings = get_settings(app);
        let items: Vec<PickItem> = settings
            .transforms
            .iter()
            .filter(|t| t.enabled)
            .map(|t| PickItem {
                id: t.id.clone(),
                name: t.name.clone(),
                keys: binding_keys(&settings, &rules::binding_id(&t.id)),
            })
            .collect();
        *lock(&PICKED) = Some(captured);
        *lock(&PICK_IDS) = items.iter().map(|item| item.id.clone()).collect();
        PICK_INDEX.store(0, Ordering::SeqCst);
        overlay::show_bar(app, "pick", "bar-pick", &items, Some(PICK_MS));
        if !items.is_empty() {
            release_pick_keys_later(app);
        }
    });
}

/// Takes the arrows, Enter and Esc while the picker is open, and gives them
/// back once it has a choice, is closed, or times out.
fn release_pick_keys_later(app: &AppHandle) {
    let generation = PICK_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    crate::shortcut::set_pick_keys(app, true);
    let app = app.clone();
    std::thread::spawn(move || {
        // The bar hides itself a little after PICK_MS.
        let deadline = Instant::now() + Duration::from_millis(PICK_MS + 200);
        while Instant::now() < deadline && lock(&PICKED).is_some() {
            std::thread::sleep(Duration::from_millis(100));
        }
        if PICK_GENERATION.load(Ordering::SeqCst) == generation {
            lock(&PICKED).take();
            crate::shortcut::set_pick_keys(&app, false);
        }
    });
}

/// A picker key: Up and Down move, Enter runs, Esc closes.
pub fn pick_key(app: &AppHandle, binding_id: &str) {
    if lock(&PICKED).is_none() {
        crate::shortcut::set_pick_keys(app, false);
        return;
    }
    let ids = lock(&PICK_IDS).clone();
    if ids.is_empty() {
        return;
    }
    let index = PICK_INDEX.load(Ordering::SeqCst).min(ids.len() - 1);
    let moved = match binding_id {
        "pick_up" => (index + ids.len() - 1) % ids.len(),
        "pick_down" => (index + 1) % ids.len(),
        "pick_enter" => return pick(app, &ids[index]),
        _ => return dismiss(app),
    };
    PICK_INDEX.store(moved, Ordering::SeqCst);
    let _ = app.emit_to("recording_overlay", "bar-pick-focus", moved);
}

/// The picker's choice.
pub fn pick(app: &AppHandle, transform_id: &str) {
    crate::shortcut::set_pick_keys(app, false);
    overlay::hide_recording_overlay(app);
    let Some(captured) = lock(&PICKED).take() else {
        return;
    };
    let Some(transform) = get_settings(app).transform(transform_id).cloned() else {
        return;
    };
    if BUSY.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        // Let the bar hide first; it never had focus, so the app keeps it.
        std::thread::sleep(Duration::from_millis(120));
        run_transform_on(&app, transform, captured);
        BUSY.store(false, Ordering::SeqCst);
    });
}

pub fn dismiss(app: &AppHandle) {
    lock(&PICKED).take();
    crate::shortcut::set_pick_keys(app, false);
    overlay::hide_recording_overlay(app);
}

/// Ask: the spoken instruction, applied to the selection or the last
/// dictation, or on its own when there is neither.
pub fn ask(app: &AppHandle, instruction: String) {
    if BUSY.swap(true, Ordering::SeqCst) {
        overlay::hide_recording_overlay(app);
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        run_ask(&app, instruction);
        BUSY.store(false, Ordering::SeqCst);
    });
}

fn run_ask(app: &AppHandle, instruction: String) {
    if instruction.trim().is_empty() {
        overlay::show_overlay_notice(app, &BarNotice::AskEmpty);
        return;
    }
    let settings = get_settings(app);
    let Some(config) = AiConfig::from_settings(&settings) else {
        let problem = AiProblem::not_set_up(&settings, None);
        overlay::show_overlay_notice_for(app, &problem_notice("Ask", &problem), PROBLEM_MS);
        return;
    };
    selection::wait_for_keys_up();
    let captured = capture(app).unwrap_or(Captured {
        text: String::new(),
        source: RunSource::Ask,
        app: foreground_app(),
        selected: false,
        suffix: String::new(),
    });
    let rewriting = !captured.text.trim().is_empty();
    let user = rules::ask_request(&instruction, rewriting.then_some(captured.text.as_str()));
    let input = captured.text.clone();
    let job = Job {
        transform_id: "ask".to_string(),
        name: "Ask".to_string(),
        output: TransformOutput::Replace,
        show_changes: rewriting,
    };
    let handle = app.clone();
    execute(
        app,
        job,
        Captured {
            source: RunSource::Ask,
            ..captured
        },
        async move {
            let answer = ai::run(
                &handle,
                &config,
                rules::ASK_SYSTEM_PROMPT,
                user,
                SELECTION_TIMEOUT,
            )
            .await?;
            let answer = rules::clean_answer(&answer, &input);
            Ok((!answer.is_empty()).then_some(answer))
        },
    );
}

/// The transform a dictation ran, for the bar after the paste.
pub struct DictationTransform {
    pub id: String,
    pub name: String,
    pub before: String,
    pub after: String,
    pub ms: i64,
    pub show_changes: bool,
}

/// What happened to a dictation, for the bar after the paste.
pub struct AfterPaste {
    pub pasted: bool,
    pub words: usize,
    pub app: Option<AppInfo>,
    pub history_id: Option<i64>,
    pub transform: Option<DictationTransform>,
    pub problem: Option<AiProblem>,
}

/// Records the dictation's transform run and says what happened: the AI
/// step that was skipped, the transform's changes, or where the text went.
pub fn after_dictation_paste(app: &AppHandle, done: AfterPaste) {
    let settings = get_settings(app);
    let mut change = None;
    if let Some(t) = &done.transform {
        let run_id = record_run(
            app,
            NewRun {
                at: now(),
                transform_id: t.id.clone(),
                name: t.name.clone(),
                source: Some(RunSource::Dictation),
                app_name: done.app.as_ref().map(|a| a.name.clone()),
                app_path: done.app.as_ref().map(|a| a.exe_path.clone()),
                in_words: rules::word_count(&t.before) as i64,
                out_words: rules::word_count(&t.after) as i64,
                ms: t.ms,
                result: Some(if done.pasted {
                    RunResult::Kept
                } else {
                    RunResult::Copied
                }),
                error: None,
                history_id: done.history_id,
            },
        );
        change = Some(remember_change(run_id, &t.name, &t.before, &t.after));
    }

    if let Some(problem) = &done.problem {
        overlay::show_overlay_notice_for(app, &ai::AiNotice::from(problem), PROBLEM_MS);
        return;
    }
    if !done.pasted {
        return;
    }
    if let (Some(change), Some(t)) = (change.clone(), &done.transform) {
        if t.show_changes && settings.bar_show_changes {
            show_changes(app, &settings, change);
            return;
        }
    }
    if settings.bar_confirm_paste {
        *lock(&UNDO) = Some(match change {
            Some(change) => Undoable::Change(change),
            None => Undoable::Paste { run_id: None },
        });
        overlay::show_overlay_notice_for(
            app,
            &BarNotice::Pasted {
                words: done.words,
                app: done.app.map(|a| a.name),
                undo: true,
            },
            CONFIRM_MS,
        );
    }
}

/// The bar's Undo: Ctrl+Z in the app, which still has focus since the bar
/// never takes it. After a transform the original also goes on the
/// clipboard, in case the app can't undo the paste.
pub fn undo(app: &AppHandle) {
    let undoable = lock(&UNDO).take();
    overlay::hide_recording_overlay(app);
    let Some(undoable) = undoable else {
        return;
    };
    if let Undoable::Learned(learned) = undoable {
        unlearn(app, &learned);
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(120));
        if let Err(e) = selection::send_ctrl(&app, VK_Z) {
            error!("Couldn't send Undo: {e}");
        }
        let (run_id, clipboard) = match undoable {
            Undoable::Paste { run_id } => (run_id, false),
            Undoable::Change(change) => {
                std::thread::sleep(Duration::from_millis(80));
                let saved = app.clipboard().write_text(change.before).is_ok();
                (change.run_id, saved)
            }
            Undoable::Learned(_) => return,
        };
        if let Some(id) = run_id {
            set_run_result(&app, id, RunResult::Undone);
        }
        info!("Undid the last paste from the Legato bar");
        overlay::show_overlay_notice(&app, &BarNotice::Undone { clipboard });
    });
}

/// Lets the bar's Undo take back words Learn from my edits just added.
pub fn offer_unlearn(learned: Vec<Learned>) {
    *lock(&UNDO) = Some(Undoable::Learned(learned));
}

fn unlearn(app: &AppHandle, learned: &[Learned]) {
    let Some(dictionary) = app.try_state::<Arc<DictionaryManager>>() else {
        return;
    };
    for l in learned.iter().rev() {
        if let Err(e) = dictionary.unlearn(l) {
            error!("Couldn't take back a learned word: {e}");
        }
    }
    info!("Took back words learned from an edit");
    overlay::show_overlay_notice(app, &BarNotice::Undone { clipboard: false });
}

/// The bar's Review: opens Needs review in the Dictionary.
pub fn review(app: &AppHandle) {
    overlay::hide_recording_overlay(app);
    crate::open_page(app, "dictionary", Some("review"));
}

/// The bar's Keep, or a click that dismisses it.
pub fn keep(app: &AppHandle) {
    lock(&UNDO).take();
    overlay::hide_recording_overlay(app);
}

/// View last changes (Win+Alt+O).
pub fn view_last_changes(app: &AppHandle) {
    let change = lock(&LAST_CHANGE).clone();
    match change {
        Some(change) => show_changes(app, &get_settings(app), change),
        None => overlay::show_overlay_notice(app, &BarNotice::NoChanges),
    }
}

/// Paste last transcript (Alt+Shift+Z).
pub fn paste_last(app: &AppHandle) {
    on_shortcut(app, |app| {
        let Some(text) = last_dictation(app) else {
            overlay::show_overlay_notice(app, &BarNotice::NoLastDictation);
            return;
        };
        let words = rules::word_count(&text);
        match paste_on_main(app, text) {
            Ok(()) => {
                *lock(&UNDO) = Some(Undoable::Paste { run_id: None });
                overlay::show_overlay_notice_for(app, &BarNotice::PastedLast { words }, CONFIRM_MS);
            }
            Err(e) => error!("Couldn't paste the last transcript: {e}"),
        }
    });
}

/// Copy last transcript (Alt+Shift+X).
pub fn copy_last(app: &AppHandle) {
    on_shortcut(app, |app| {
        let Some(text) = last_dictation(app) else {
            overlay::show_overlay_notice(app, &BarNotice::NoLastDictation);
            return;
        };
        let words = rules::word_count(&text);
        match app.clipboard().write_text(text) {
            Ok(()) => overlay::show_overlay_notice(app, &BarNotice::CopiedLast { words }),
            Err(e) => error!("Couldn't copy the last transcript: {e}"),
        }
    });
}

/// Stops a transform that is waiting for the model from pasting.
pub fn cancel() {
    RUN_GENERATION.fetch_add(1, Ordering::SeqCst);
    lock(&PICKED).take();
}
