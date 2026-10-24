//! The Scratchpad: a floating window of notes you type or dictate into
//! instead of the app in front, and where pastes that fail are saved.
//!
//! Win+Alt+S opens the window when tapped and dictates into a new note when
//! held. The window's Dictate here sends the words to the note at its
//! cursor, and Send to pastes a note into the app that was in front when the
//! window opened.

use crate::context::{self, AppInfo};
use crate::managers::scratch_store::{
    self as store, DeletedNote, NewNote, NoteSource, ScratchNote, ScratchVersion,
};
use crate::managers::trash_store::{self, NewTrash, TrashKind};
use crate::settings::{get_settings, write_settings, ScratchpadOpen};
use crate::transcription_coordinator::TranscriptionCoordinator;
use crate::transforms::BarNotice;
use anyhow::Result;
use chrono::Utc;
use log::{debug, error, info, warn};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_specta::Event;

pub const WINDOW: &str = "scratchpad";
/// The binding the window's dictation runs under; not a user shortcut.
pub const DICTATE_BINDING: &str = "scratchpad_dictate";
const OPEN_BINDING: &str = "open_scratchpad";
const WINDOW_SIZE: (f64, f64) = (520.0, 520.0);
pub const WINDOW_MIN: (f64, f64) = (360.0, 300.0);
/// Time for the app to take focus back before Send to pastes.
const REFOCUS_MS: u64 = 150;

/// Sent when notes or tabs change, from any window.
#[derive(Clone, Debug, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ScratchpadChanged;

/// Tells the window which note to show.
#[derive(Clone, Debug, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ScratchpadShow {
    pub note_id: i64,
}

/// Dictated words for the note open in the window, inserted at its cursor.
#[derive(Clone, Debug, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ScratchpadInsert {
    pub note_id: i64,
    pub text: String,
}

/// Whether a Dictate here is listening: the window's, or with `notes`, the
/// one in a note's My notes.
#[derive(Clone, Debug, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ScratchpadDictating {
    pub active: bool,
    pub notes: bool,
}

/// What the window needs when it opens.
#[derive(Clone, Debug, Serialize, Type)]
pub struct ScratchpadState {
    pub tabs: Vec<ScratchNote>,
    pub active: Option<i64>,
    pub keep_on_top: bool,
    /// The app Send to pastes into.
    pub target_app: Option<String>,
    pub target_app_path: Option<String>,
}

/// An app's window Send to can paste into.
#[derive(Clone, Debug, Serialize, Type)]
pub struct SendTarget {
    pub handle: i64,
    pub name: String,
    pub title: String,
    pub path: String,
    /// The one Send to pastes into now.
    pub current: bool,
}

/// Windows listed in Send to's picker.
const MAX_TARGETS: usize = 12;

/// The app in front when the window opened, or the one picked in Send to.
struct Target {
    handle: isize,
    info: AppInfo,
}

#[derive(Default)]
struct State {
    target: Option<Target>,
    /// Where the running dictation goes: a note, or None for a new one.
    dictate_into: Option<i64>,
    /// `dictate_into` is a Notes recording's My notes, not a Scratchpad note.
    into_notes: bool,
    dictating: bool,
    /// Dictate here was clicked again to stop.
    stopping: bool,
    /// The shortcut is held and has started a dictation.
    holding: bool,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
/// Counts shortcut presses, so a release knows whether its press turned into
/// a hold.
static PRESS: AtomicU64 = AtomicU64::new(0);

fn with_state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(State::default))
}

fn now() -> i64 {
    Utc::now().timestamp()
}

pub struct ScratchpadManager {
    app: AppHandle,
    db_path: PathBuf,
}

impl ScratchpadManager {
    /// Opens the notes in the history database, which `HistoryManager` has
    /// already migrated.
    pub fn new(app: &AppHandle) -> Result<Self> {
        let manager = Self {
            app: app.clone(),
            db_path: crate::portable::app_data_dir(app)?.join("history.db"),
        };
        match manager.conn().and_then(|c| store::prune_blank(&c)) {
            Ok(0) => {}
            Ok(n) => info!("Removed {n} empty Scratchpad notes"),
            Err(e) => warn!("Couldn't remove empty Scratchpad notes: {e}"),
        }
        Ok(manager)
    }

    fn conn(&self) -> Result<Connection> {
        Ok(Connection::open(&self.db_path)?)
    }

    pub(crate) fn changed(&self) {
        if let Err(e) = ScratchpadChanged.emit(&self.app) {
            error!("Failed to emit scratchpad-changed: {}", e);
        }
    }

    pub fn list(&self) -> Result<Vec<ScratchNote>> {
        store::list(&self.conn()?)
    }

    pub fn get(&self, id: i64) -> Result<Option<ScratchNote>> {
        store::get(&self.conn()?, id)
    }

    pub fn tabs(&self) -> Result<Vec<ScratchNote>> {
        store::tabs(&self.conn()?)
    }

    pub fn create(&self, new: &NewNote) -> Result<ScratchNote> {
        let note = store::create(&mut self.conn()?, new, now())?;
        self.changed();
        Ok(note)
    }

    pub fn save_text(&self, id: i64, text: &str) -> Result<ScratchNote> {
        let note = store::save_text(&mut self.conn()?, id, text, now())?;
        self.changed();
        Ok(note)
    }

    pub fn append(&self, id: i64, text: &str) -> Result<ScratchNote> {
        let note = store::append(&mut self.conn()?, id, text, now())?;
        self.changed();
        Ok(note)
    }

    pub fn rename(&self, id: i64, title: &str) -> Result<ScratchNote> {
        let note = store::rename(&self.conn()?, id, title)?;
        self.changed();
        Ok(note)
    }

    pub fn set_pinned(&self, id: i64, pinned: bool) -> Result<ScratchNote> {
        let note = store::set_pinned(&self.conn()?, id, pinned)?;
        self.changed();
        Ok(note)
    }

    pub fn dismiss_reason(&self, id: i64) -> Result<ScratchNote> {
        let note = store::dismiss_reason(&self.conn()?, id)?;
        self.changed();
        Ok(note)
    }

    pub fn versions(&self, id: i64) -> Result<Vec<ScratchVersion>> {
        store::versions(&self.conn()?, id)
    }

    pub fn restore_version(&self, id: i64, version_id: i64) -> Result<ScratchNote> {
        let note = store::restore_version(&mut self.conn()?, id, version_id, now())?;
        self.changed();
        Ok(note)
    }

    /// Removes notes to Recently deleted, versions and all.
    pub fn delete(&self, ids: &[i64]) -> Result<Vec<DeletedNote>> {
        let mut conn = self.conn()?;
        let gone = store::delete(&mut conn, ids)?;
        let at = now();
        let kept = gone.iter().try_for_each(|d| -> Result<()> {
            trash_store::put(
                &conn,
                &NewTrash {
                    kind: TrashKind::Scratch,
                    item_id: d.note.id,
                    title: &d.note.title,
                    preview: &trash_store::preview(&d.note.text),
                    duration_ms: None,
                    payload: serde_json::to_string(d)?,
                },
                at,
            )?;
            Ok(())
        });
        if let Err(e) = kept {
            // Never gone with no way back: put them back and say so.
            store::restore(&mut conn, &gone)?;
            for d in &gone {
                trash_store::forget(&conn, TrashKind::Scratch, d.note.id)?;
            }
            return Err(e);
        }
        if !gone.is_empty() {
            crate::trash::changed(&self.app);
        }
        self.changed();
        Ok(gone)
    }

    /// Puts deleted notes back, from the toast's Undo or Recently deleted.
    pub fn restore(&self, deleted: &[DeletedNote]) -> Result<()> {
        let mut conn = self.conn()?;
        store::restore(&mut conn, deleted)?;
        for d in deleted {
            trash_store::forget(&conn, TrashKind::Scratch, d.note.id)?;
        }
        crate::trash::changed(&self.app);
        self.changed();
        Ok(())
    }

    pub fn open_tab(&self, id: i64) -> Result<()> {
        store::open_tab(&mut self.conn()?, id)?;
        self.changed();
        Ok(())
    }

    /// Closes a tab; a note left empty goes with it.
    pub fn close_tab(&self, id: i64) -> Result<()> {
        let mut conn = self.conn()?;
        store::close_tab(&mut conn, id)?;
        store::prune_blank(&conn)?;
        self.changed();
        Ok(())
    }

    /// A blank tab already open, or a new note, so New note doesn't pile
    /// up empty ones.
    pub fn blank_or_new(&self) -> Result<ScratchNote> {
        let blank = self
            .tabs()?
            .into_iter()
            .find(|n| n.text.trim().is_empty() && n.title.is_empty());
        match blank {
            Some(note) => Ok(note),
            None => self.create(&NewNote::default()),
        }
    }

    pub fn move_tab(&self, id: i64, to: usize) -> Result<()> {
        store::move_tab(&mut self.conn()?, id, to)?;
        self.changed();
        Ok(())
    }

    fn last_pinned(&self) -> Result<Option<ScratchNote>> {
        store::last_pinned(&self.conn()?)
    }
}

fn manager(app: &AppHandle) -> Arc<ScratchpadManager> {
    app.state::<Arc<ScratchpadManager>>().inner().clone()
}

/// Puts deleted notes back, for Recently deleted.
pub(crate) fn restore_deleted(app: &AppHandle, deleted: &[DeletedNote]) -> Result<()> {
    manager(app).restore(deleted)
}

/// Remembers the note on screen, for Resume.
pub fn set_active(app: &AppHandle, id: Option<i64>) {
    let mut settings = get_settings(app);
    if settings.scratchpad_last_note != id {
        settings.scratchpad_last_note = id;
        write_settings(app, settings);
    }
}

pub fn state(app: &AppHandle) -> Result<ScratchpadState> {
    let tabs = manager(app).tabs()?;
    let settings = get_settings(app);
    let active = settings
        .scratchpad_last_note
        .filter(|id| tabs.iter().any(|n| n.id == *id))
        .or_else(|| tabs.last().map(|n| n.id));
    let (target_app, target_app_path) = with_state(|s| {
        s.target
            .as_ref()
            .map(|t| (Some(t.info.name.clone()), Some(t.info.exe_path.clone())))
            .unwrap_or((None, None))
    });
    Ok(ScratchpadState {
        tabs,
        active,
        keep_on_top: settings.scratchpad_keep_on_top,
        target_app,
        target_app_path,
    })
}

/// The windows Send to can paste into, front to back.
pub fn send_targets() -> Vec<SendTarget> {
    let current = with_state(|s| s.target.as_ref().map(|t| t.handle));
    context::open_windows()
        .into_iter()
        .take(MAX_TARGETS)
        .map(|(handle, window)| {
            let info = context::resolve(&window);
            SendTarget {
                handle: handle as i64,
                name: info.name,
                title: window.title,
                path: window.exe_path,
                current: current == Some(handle),
            }
        })
        .collect()
}

/// Points Send to at another window. False when it has closed since the
/// picker listed it.
pub fn set_send_target(handle: i64) -> bool {
    let handle = handle as isize;
    let Some((_, window)) = context::open_windows()
        .into_iter()
        .find(|(h, _)| *h == handle)
    else {
        return false;
    };
    let info = context::resolve(&window);
    with_state(|s| s.target = Some(Target { handle, info }));
    true
}

fn build_window(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    if let Some(window) = app.get_webview_window(WINDOW) {
        return Some(window);
    }
    let mut builder = WebviewWindowBuilder::new(
        app,
        WINDOW,
        WebviewUrl::App("src/scratchpad/index.html".into()),
    )
    .title("Scratchpad")
    .inner_size(WINDOW_SIZE.0, WINDOW_SIZE.1)
    .min_inner_size(WINDOW_MIN.0, WINDOW_MIN.1)
    .decorations(false)
    .resizable(true)
    .maximizable(true)
    .always_on_top(get_settings(app).scratchpad_keep_on_top)
    .visible(false);
    if let Some(data_dir) = crate::portable::data_dir() {
        builder = builder.data_directory(data_dir.join("webview"));
    }
    match builder.build() {
        Ok(window) => {
            crate::fit_window_to_screen(&window, WINDOW_MIN);
            Some(window)
        }
        Err(e) => {
            error!("Failed to create the Scratchpad window: {}", e);
            None
        }
    }
}

/// Remembers the app in front, unless it's one of Legato's own windows.
fn remember_target() {
    let own = std::env::current_exe()
        .ok()
        .map(|p| p.to_string_lossy().to_lowercase());
    let (Some(window), Some(handle)) = (context::foreground_window(), context::foreground_handle())
    else {
        return;
    };
    if own.as_deref() == Some(window.exe_path.to_lowercase().as_str()) {
        return;
    }
    let info = context::resolve(&window);
    with_state(|s| s.target = Some(Target { handle, info }));
}

/// The note the shortcut opens, by Settings: the last one, a new one or the
/// last pinned one.
fn note_to_open(app: &AppHandle) -> Result<i64> {
    let m = manager(app);
    let settings = get_settings(app);
    let pick = match settings.scratchpad_open {
        ScratchpadOpen::New => None,
        ScratchpadOpen::Pinned => m.last_pinned()?.map(|n| n.id),
        ScratchpadOpen::Resume => settings
            .scratchpad_last_note
            .filter(|id| m.get(*id).ok().flatten().is_some())
            .or_else(|| m.tabs().ok().and_then(|t| t.last().map(|n| n.id))),
    };
    match pick {
        Some(id) => Ok(id),
        None => Ok(m.create(&NewNote::default())?.id),
    }
}

/// Opens the window on a note, or on the one Settings picks.
pub fn show(app: &AppHandle, note: Option<i64>) -> Result<()> {
    let visible = app
        .get_webview_window(WINDOW)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    if !visible {
        remember_target();
    }
    let id = match note {
        Some(id) => id,
        None => note_to_open(app)?,
    };
    manager(app).open_tab(id)?;
    set_active(app, Some(id));
    let Some(window) = build_window(app) else {
        return Ok(());
    };
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
    let _ = ScratchpadShow { note_id: id }.emit_to(app, WINDOW);
    Ok(())
}

/// Opens the window on a new note, or on a blank tab already open, so
/// repeated clicks on the jump list's New note don't pile up empty notes.
pub fn show_new(app: &AppHandle) -> Result<()> {
    let id = manager(app).blank_or_new()?.id;
    show(app, Some(id))
}

pub fn hide(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(WINDOW) {
        let _ = window.hide();
    }
}

/// The shortcut tapped: open the window, or close it when it's in front.
fn toggle(app: &AppHandle) {
    let focused = app
        .get_webview_window(WINDOW)
        .is_some_and(|w| w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false));
    if focused {
        hide(app);
    } else if let Err(e) = show(app, None) {
        error!("Couldn't open the Scratchpad: {}", e);
    }
}

/// The bubble's Scratchpad button: the bubble never has focus, so this
/// goes by whether the window shows at all.
pub fn toggle_from_bubble(app: &AppHandle) {
    let visible = app
        .get_webview_window(WINDOW)
        .is_some_and(|w| w.is_visible().unwrap_or(false));
    if visible {
        hide(app);
    } else if let Err(e) = show(app, None) {
        error!("Couldn't open the Scratchpad: {}", e);
    }
}

pub fn set_keep_on_top(app: &AppHandle, on: bool) {
    let mut settings = get_settings(app);
    settings.scratchpad_keep_on_top = on;
    write_settings(app, settings);
    if let Some(window) = app.get_webview_window(WINDOW) {
        let _ = window.set_always_on_top(on);
    }
    // The page's toggle follows the window's button.
    let _ = ScratchpadChanged.emit(app);
}

fn coordinator_input(app: &AppHandle, hotkey: &str, pressed: bool) {
    let Some(coordinator) = app.try_state::<TranscriptionCoordinator>() else {
        warn!("TranscriptionCoordinator is not initialized");
        return;
    };
    coordinator.send_input(
        DICTATE_BINDING,
        hotkey,
        pressed,
        crate::settings::ShortcutActivation::PushToTalk,
        Duration::ZERO,
    );
}

fn set_dictating(app: &AppHandle, active: bool) {
    let notes = with_state(|s| {
        s.dictating = active;
        s.into_notes
    });
    let _ = ScratchpadDictating { active, notes }.emit(app);
}

/// Win+Alt+S: a tap opens the window; held past the hold threshold, it
/// dictates into a new note until released.
pub fn shortcut(app: &AppHandle, hotkey: &str, pressed: bool) {
    if pressed {
        let press = PRESS.fetch_add(1, Ordering::SeqCst) + 1;
        let threshold = Duration::from_millis(get_settings(app).hold_threshold_ms);
        let app = app.clone();
        let hotkey = hotkey.to_string();
        std::thread::spawn(move || {
            std::thread::sleep(threshold);
            // Released (or pressed again) in the meantime: it was a tap.
            if PRESS.load(Ordering::SeqCst) != press {
                return;
            }
            with_state(|s| {
                s.holding = true;
                s.dictate_into = None;
                s.into_notes = false;
            });
            remember_target();
            set_dictating(&app, true);
            coordinator_input(&app, &hotkey, true);
        });
    } else {
        // Ends the press; a pending hold sees the count move and stops.
        PRESS.fetch_add(1, Ordering::SeqCst);
        if with_state(|s| std::mem::take(&mut s.holding)) {
            coordinator_input(app, hotkey, false);
        } else {
            toggle(app);
        }
    }
}

/// Whether this binding is the Scratchpad's shortcut.
pub fn is_shortcut(binding_id: &str) -> bool {
    binding_id == OPEN_BINDING
}

/// The window's Dictate here: starts a dictation into a note, or stops the
/// one running.
pub fn dictate_here(app: &AppHandle, note_id: i64) {
    dictate(app, note_id, false);
}

/// Dictate here in a Notes recording's My notes.
pub fn dictate_into_notes(app: &AppHandle, note_id: i64) {
    dictate(app, note_id, true);
}

fn dictate(app: &AppHandle, note_id: i64, notes: bool) {
    let Some(coordinator) = app.try_state::<TranscriptionCoordinator>() else {
        return;
    };
    let starting = !with_state(|s| s.dictating);
    with_state(|s| {
        s.stopping = !starting;
        if starting {
            s.dictate_into = Some(note_id);
            s.into_notes = notes;
        }
    });
    if starting {
        set_dictating(app, true);
    }
    coordinator.send_external_input(DICTATE_BINDING, "scratchpad");
    if starting {
        // No model, or the microphone failed: the button shouldn't stay on.
        let app = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(600));
            let recording = app
                .try_state::<Arc<crate::managers::audio::AudioRecordingManager>>()
                .is_some_and(|a| a.is_recording());
            if !recording && with_state(|s| s.dictating && !s.stopping) {
                set_dictating(&app, false);
            }
        });
    }
}

/// A dictation was cancelled.
pub fn dictation_cancelled(app: &AppHandle) {
    if with_state(|s| s.dictating) {
        with_state(|s| s.holding = false);
        set_dictating(app, false);
        with_state(|s| s.into_notes = false);
    }
}

/// Where a Scratchpad dictation's words go: the note the window has open at
/// its cursor, or a new note that opens.
pub fn receive(app: &AppHandle, text: String) {
    let into = with_state(|s| s.dictate_into.take());
    set_dictating(app, false);
    if with_state(|s| std::mem::take(&mut s.into_notes)) {
        if let Some(note_id) = into {
            crate::notes::receive_dictation(app, note_id, text);
        }
        return;
    }
    if text.trim().is_empty() {
        return;
    }
    let m = manager(app);
    let window_open = app
        .get_webview_window(WINDOW)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    let result = match into {
        Some(note_id) if window_open => ScratchpadInsert { note_id, text }
            .emit_to(app, WINDOW)
            .map_err(anyhow::Error::from),
        Some(note_id) => m.append(note_id, &text).map(|_| ()),
        None => m
            .create(&NewNote {
                text,
                source: Some(NoteSource::Dictation),
                ..Default::default()
            })
            .and_then(|note| show(app, Some(note.id))),
    };
    if let Err(e) = result {
        error!("Couldn't save the dictation in the Scratchpad: {}", e);
    }
}

/// A paste that failed: the text goes into a new note, which opens when
/// Settings › Output says so. Returns the note.
pub fn save_failed_paste(
    app: &AppHandle,
    text: &str,
    target: Option<&AppInfo>,
    reason: &str,
) -> Option<i64> {
    let note = manager(app).create(&NewNote {
        text: text.to_string(),
        source: Some(NoteSource::FailedPaste),
        source_app: target.map(|t| t.name.clone()),
        source_app_path: target.map(|t| t.exe_path.clone()),
        reason: Some(reason.to_string()),
    });
    match note {
        Ok(note) => {
            debug!("Saved a failed paste as note {}", note.id);
            let id = note.id;
            if !get_settings(app).scratchpad_failed_pastes {
                // Nothing opens to show it, so Windows says where it went.
                crate::desktop::notify_failed_paste(app, note.id, target.map(|t| t.name.as_str()));
            } else {
                let app = app.clone();
                // After the bar has said what happened.
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(400));
                    if let Err(e) = show(&app, Some(note.id)) {
                        error!("Couldn't open the Scratchpad: {}", e);
                    }
                });
            }
            Some(id)
        }
        Err(e) => {
            error!("Couldn't save the failed paste: {}", e);
            None
        }
    }
}

/// The bar's word for a failed paste. `admin` when the app runs as
/// administrator.
pub fn failed_paste_notice(
    target: Option<&AppInfo>,
    admin: bool,
    note_id: Option<i64>,
) -> BarNotice {
    BarNotice::SavedToScratchpad {
        app: target.map(|t| t.name.clone()),
        admin,
        note_id,
    }
}

/// Send to: hides the window, brings back the app that was in front and
/// pastes the note there.
pub fn send_to_app(app: &AppHandle, text: String) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("The note is empty.".into());
    }
    let (handle, info) = with_state(|s| {
        s.target
            .as_ref()
            .map(|t| (Some(t.handle), Some(t.info.clone())))
            .unwrap_or((None, None))
    });
    let (Some(handle), Some(info)) = (handle, info) else {
        return Err("Click into the app you want the text in, then open the Scratchpad.".into());
    };
    hide(app);
    if !context::focus_window(handle) {
        return Err(format!("{} isn't open any more.", info.name));
    }
    std::thread::sleep(Duration::from_millis(REFOCUS_MS));
    if context::foreground_is_elevated() {
        return Err(format!(
            "{} runs as administrator, so Windows blocks typing into it.",
            info.name
        ));
    }
    let overrides = crate::clipboard::PasteOverrides::for_category(
        &get_settings(app),
        Some(info.category.as_str()),
    );
    let (tx, rx) = std::sync::mpsc::channel();
    let app_for_paste = app.clone();
    app.run_on_main_thread(move || {
        let _ = tx.send(crate::clipboard::paste_dictation(
            text,
            app_for_paste,
            overrides,
        ));
    })
    .map_err(|e| e.to_string())?;
    rx.recv_timeout(Duration::from_secs(10))
        .map_err(|_| "The paste didn't finish.".to_string())??;
    Ok(info.name)
}
