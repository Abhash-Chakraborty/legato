//! The Notes side panel: the recording in progress, docked at the right of
//! the screen, with its live transcript, your own notes, Pause and Stop.
//! The bubble's Notes button opens it. Closing it leaves the recording
//! running; the bar's Notes pill and the bubble's green glow show that
//! until the recording stops or the panel comes back.

use log::warn;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewWindowBuilder};

pub const WINDOW: &str = "notes_panel";
/// The panel's width, and its gap from the screen's edges, in logical pixels.
const WIDTH: f64 = 360.0;
const MARGIN: f64 = 12.0;

static OPEN: AtomicBool = AtomicBool::new(false);

/// Whether the panel is on screen.
pub fn is_open() -> bool {
    OPEN.load(Ordering::Relaxed)
}

/// Builds the hidden window the first time the panel opens, so a PC that
/// never uses Notes doesn't keep a webview for it. Not on the main thread:
/// building a webview from there deadlocks on Windows. The page loads the
/// recording in progress, lines so far included, when it starts.
fn build_window(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    if let Some(window) = app.get_webview_window(WINDOW) {
        return Some(window);
    }
    let mut builder = WebviewWindowBuilder::new(
        app,
        WINDOW,
        tauri::WebviewUrl::App("src/notespanel/index.html".into()),
    )
    .title("Notes")
    .inner_size(WIDTH, 640.0)
    .decorations(false)
    // Windows sizes an undecorated window with a shadow as if it had a
    // title bar, by a different amount each time; the page draws its own
    // border instead.
    .shadow(false)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false);
    if let Some(data_dir) = crate::portable::data_dir() {
        builder = builder.data_directory(data_dir.join("webview"));
    }
    builder
        .build()
        .map_err(|e| warn!("Couldn't create the Notes panel: {e}"))
        .ok()
}

/// Docks the window at the right of the work area of the screen the
/// pointer is on.
fn dock(app: &AppHandle, window: &tauri::WebviewWindow) {
    let pointer = app.cursor_position().ok().and_then(|p| {
        let scale = window.scale_factor().ok()?;
        Some((p.x / scale, p.y / scale))
    });
    let Some(monitor) = crate::bubble::monitor_at(app, pointer) else {
        return;
    };
    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let (x, y) = (
        f64::from(area.position.x) / scale,
        f64::from(area.position.y) / scale,
    );
    let (w, h) = (
        f64::from(area.size.width) / scale,
        f64::from(area.size.height) / scale,
    );
    let width = WIDTH.min(w - 2.0 * MARGIN);
    let height = (h - 2.0 * MARGIN).max(320.0);
    let _ = window.set_size(LogicalSize::new(width, height));
    let _ = window.set_position(LogicalPosition::new(x + w - width - MARGIN, y + MARGIN));
}

fn set_open(app: &AppHandle, open: bool) {
    if OPEN.swap(open, Ordering::SeqCst) != open {
        let _ = app.emit("notes-panel", open);
        crate::notes::refresh_pill(app);
    }
}

/// Shows the panel on the recording in progress. Called from async
/// commands, off the main thread.
pub fn show(app: &AppHandle) {
    let Some(window) = build_window(app) else {
        return;
    };
    dock(app, &window);
    let _ = window.show();
    let _ = window.set_focus();
    set_open(app, true);
}

/// Hides the panel; a recording carries on.
pub fn hide(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(WINDOW) {
        let _ = window.hide();
    }
    set_open(app, false);
}
