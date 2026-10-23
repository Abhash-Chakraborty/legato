use crate::input;
use crate::settings;
use crate::settings::{
    AppSettings, BarLook, IslandIdle, LiveCaption, OverlayPosition, OverlayStyle,
};
use log::debug;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewWindowBuilder};

// Native overlay window sizes (logical pixels). One window is reused for every
// state and resized in `show_overlay_state`; each size need only be at least as
// large as the card it hosts (the `--ov-*` vars in RecordingOverlay.css). The
// card is CSS-anchored flush to the screen edge, so window height doesn't move
// where the card sits — only OVERLAY_TOP_OFFSET / OVERLAY_BOTTOM_OFFSET do. Keep
// these in sync with the CSS card geometry.
//
// These sizes are additionally multiplied by the accessibility text scale (see
// windows_text_scale_factor), which WebView2 applies as a zoom.
//
// Compact overlay (Minimal / transcribing / processing): the 40h pill animates
// width from 172 (--ov-rest-w) to 216 (--ov-work-w) and expands from center, so
// the window must fit the widest state plus a little slack.
const OVERLAY_WIDTH: f64 = 256.0;
const OVERLAY_HEIGHT: f64 = 50.0;

const OVERLAY_CHANGES_WIDTH: f64 = 480.0;
const OVERLAY_CHANGES_HEIGHT: f64 = 290.0;
const OVERLAY_PICK_WIDTH: f64 = 320.0;
const OVERLAY_PICK_HEIGHT: f64 = 360.0;

const OVERLAY_NOTES_WIDTH: f64 = 300.0;
const OVERLAY_CALL_WIDTH: f64 = 380.0;
const OVERLAY_CALL_HEIGHT: f64 = 124.0;
/// An agent's question from Inyon: a few lines and up to five options.
const OVERLAY_AGENT_WIDTH: f64 = 440.0;
const OVERLAY_AGENT_HEIGHT: f64 = 320.0;

// Actual is 394x118, just a little extra
const OVERLAY_STREAM_WIDTH: f64 = 400.0;
const OVERLAY_STREAM_HEIGHT: f64 = 120.0;

/// Overlay window size (logical) for a given UI state.
fn overlay_dimensions(state: &str) -> (f64, f64) {
    match state {
        "streaming" => (OVERLAY_STREAM_WIDTH, OVERLAY_STREAM_HEIGHT),
        // A notice is one row, as wide as the Live panel so the message fits.
        "notice" => (OVERLAY_STREAM_WIDTH, OVERLAY_HEIGHT),
        // A transform's word diff with Keep and Undo.
        "changes" => (OVERLAY_CHANGES_WIDTH, OVERLAY_CHANGES_HEIGHT),
        // The transform picker for the selection.
        "pick" => (OVERLAY_PICK_WIDTH, OVERLAY_PICK_HEIGHT),
        // A Notes recording: its clock, two meters and Stop.
        "notes" => (OVERLAY_NOTES_WIDTH, OVERLAY_HEIGHT),
        // The call card: offer notes, or say the call ended.
        "call" => (OVERLAY_CALL_WIDTH, OVERLAY_CALL_HEIGHT),
        // An agent waiting on Inyon: its question and options.
        "agent" => (OVERLAY_AGENT_WIDTH, OVERLAY_AGENT_HEIGHT),
        _ => (OVERLAY_WIDTH, OVERLAY_HEIGHT),
    }
}

static LAST_MIC_LEVEL_EMIT: AtomicU64 = AtomicU64::new(0);
/// Set while the main window shows a level meter (Home's right rail).
static MAIN_WANTS_MIC_LEVEL: AtomicBool = AtomicBool::new(false);

/// Lets the main window ask for microphone levels while it shows a meter.
pub fn set_main_wants_mic_level(enabled: bool) {
    MAIN_WANTS_MIC_LEVEL.store(enabled, Ordering::Relaxed);
}
const EMIT_THROTTLE_MS: u64 = 33; // ~30 FPS

/// The island's window: one fixed size, room for its largest state (the
/// transform picker), so the shape grows and shrinks inside it and the window
/// never resizes mid-animation. Clicks pass through all of it but the shape.
const ISLAND_WIDTH: f64 = 520.0;
const ISLAND_HEIGHT: f64 = 380.0;

const OVERLAY_TOP_OFFSET: f64 = 4.0;
const OVERLAY_BOTTOM_OFFSET: f64 = 40.0;

/// Forces a window to be topmost using the Win32 API.
/// This is more reliable than Tauri's set_always_on_top which can be overridden
fn force_overlay_topmost(overlay_window: &tauri::webview::WebviewWindow) {
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
    };

    // Clone because run_on_main_thread takes 'static
    let overlay_clone = overlay_window.clone();

    // Make sure the Win32 call happens on the UI thread
    let _ = overlay_clone.clone().run_on_main_thread(move || {
        if let Ok(hwnd) = overlay_clone.hwnd() {
            unsafe {
                // Force Z-order: make this window topmost without changing size/pos or stealing focus
                let _ = SetWindowPos(
                    hwnd,
                    Some(HWND_TOPMOST),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            }
        }
    });
}

fn get_monitor_with_cursor(app_handle: &AppHandle) -> Option<tauri::Monitor> {
    if let Some(mouse_location) = input::get_cursor_position(app_handle) {
        if let Ok(monitors) = app_handle.available_monitors() {
            for monitor in monitors {
                // Both the cursor (enigo -> GetCursorPos) and the monitor bounds
                // are physical pixels, so compare them directly.
                if is_mouse_within_monitor(mouse_location, monitor.position(), monitor.size()) {
                    return Some(monitor);
                }
            }
        }
    }

    app_handle.primary_monitor().ok().flatten()
}

fn is_mouse_within_monitor(
    mouse_pos: (i32, i32),
    monitor_pos: &PhysicalPosition<i32>,
    monitor_size: &PhysicalSize<u32>,
) -> bool {
    let (mouse_x, mouse_y) = mouse_pos;
    let PhysicalPosition {
        x: monitor_x,
        y: monitor_y,
    } = *monitor_pos;
    let PhysicalSize {
        width: monitor_width,
        height: monitor_height,
    } = *monitor_size;

    mouse_x >= monitor_x
        && mouse_x < (monitor_x + monitor_width as i32)
        && mouse_y >= monitor_y
        && mouse_y < (monitor_y + monitor_height as i32)
}

static WINDOWS_OVERLAY_IS_STREAMING: AtomicBool = AtomicBool::new(false);

/// Windows accessibility text size (Settings > Accessibility > Text size), a
/// separate axis from display scaling that WebView2 applies as a document zoom.
fn windows_text_scale_factor() -> f64 {
    // Absent until the user moves the slider off 100%; stored as a percentage.
    winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
        .open_subkey(r"Software\Microsoft\Accessibility")
        .and_then(|key| key.get_value::<u32, _>("TextScaleFactor"))
        .map(|percent| (percent as f64 / 100.0).clamp(1.0, 2.25))
        .unwrap_or(1.0)
}

/// Overlay rectangle in the destination monitor's physical pixels, so nothing
/// is converted through the window's previous-monitor DPI.
fn windows_overlay_bounds(
    monitor_position: PhysicalPosition<i32>,
    monitor_size: PhysicalSize<u32>,
    scale: f64,
    text_scale: f64,
    logical_width: f64,
    logical_height: f64,
    overlay_position: OverlayPosition,
) -> (i32, i32, i32, i32) {
    // Grow the window with the text scale; offsets stay DPI-only since the
    // card sits flush against the window's screen-edge side.
    let content_scale = scale * text_scale;
    let width = (logical_width * content_scale).round().max(1.0) as i32;
    let height = (logical_height * content_scale).round().max(1.0) as i32;
    let x = (monitor_position.x as f64 + (monitor_size.width as f64 - width as f64) / 2.0).round()
        as i32;
    let y = match overlay_position {
        OverlayPosition::Top => {
            (monitor_position.y as f64 + OVERLAY_TOP_OFFSET * scale).round() as i32
        }
        OverlayPosition::Bottom => (monitor_position.y as f64 + monitor_size.height as f64
            - height as f64
            - OVERLAY_BOTTOM_OFFSET * scale)
            .round() as i32,
    };

    (x, y, width, height)
}

/// Moves and sizes the overlay in one native SetWindowPos, bypassing tao's
/// current-DPI logical conversion that mislands cross-monitor moves.
fn place_windows_overlay(
    app_handle: &AppHandle,
    overlay_window: &tauri::webview::WebviewWindow,
    logical_width: f64,
    logical_height: f64,
) -> Result<(), String> {
    use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER};

    let monitor = get_monitor_with_cursor(app_handle)
        .ok_or_else(|| "failed to determine the monitor containing the cursor".to_string())?;
    let text_scale = windows_text_scale_factor();
    let settings = settings::get_settings(app_handle);
    let island = island_on(&settings);
    let (x, mut y, width, height) = windows_overlay_bounds(
        *monitor.position(),
        *monitor.size(),
        monitor.scale_factor(),
        text_scale,
        logical_width,
        logical_height,
        if island {
            OverlayPosition::Top
        } else {
            settings.overlay_position
        },
    );
    // The island hangs from the very top of the screen.
    if island {
        y = monitor.position().y;
    }
    let hwnd = overlay_window
        .hwnd()
        .map_err(|error| format!("failed to get overlay window handle: {error}"))?;

    unsafe {
        SetWindowPos(
            hwnd,
            None,
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE | SWP_NOZORDER,
        )
        .map_err(|error| format!("failed to set overlay bounds: {error}"))?;
    }

    log::debug!(
        "windows overlay bounds: x={} y={} width={} height={} scale={} text_scale={}",
        x,
        y,
        width,
        height,
        monitor.scale_factor(),
        text_scale
    );
    Ok(())
}

/// Creates the recording overlay window and keeps it hidden by default
pub fn create_recording_overlay(app_handle: &AppHandle) {
    if get_monitor_with_cursor(app_handle).is_none() {
        debug!("Failed to determine overlay position, not creating overlay window");
        return;
    }

    // Position starts unset — `place_windows_overlay` sets the bounds before the
    // overlay is shown.
    let mut builder = WebviewWindowBuilder::new(
        app_handle,
        "recording_overlay",
        tauri::WebviewUrl::App("src/overlay/index.html".into()),
    )
    .title("Recording")
    .resizable(false)
    .inner_size(OVERLAY_WIDTH, OVERLAY_HEIGHT)
    .shadow(false)
    .maximizable(false)
    .minimizable(false)
    .closable(false)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .transparent(true)
    .focusable(false)
    .focused(false)
    .visible(false);

    if let Some(data_dir) = crate::portable::data_dir() {
        builder = builder.data_directory(data_dir.join("webview"));
    }

    match builder.build() {
        Ok(_) => {
            debug!("Recording overlay window created successfully (hidden)");
            // The resting island, once the page is listening.
            if island_rests(&settings::get_settings(app_handle)) {
                let handle = app_handle.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(1500));
                    if SHOWN.lock().unwrap_or_else(|e| e.into_inner()).is_none() {
                        show_overlay_state(&handle, "rest");
                    }
                });
            }
        }
        Err(e) => {
            debug!("Failed to create recording overlay window: {}", e);
        }
    }
}

/// A dictation running or being written up: what the bubble shows in place
/// of the bar while it's on screen.
const BUBBLE_STATES: [&str; 4] = ["recording", "streaming", "transcribing", "processing"];

/// Shows a dictation's state: on the bubble while it's turned on, so there
/// aren't two of them, else on the bar.
/// Counts the dictation states sent while the bubble has the dictation, so
/// a delayed showing of the bar knows whether its state still holds.
static BUBBLE_STATE_GENERATION: AtomicU64 = AtomicU64::new(0);

/// How long the bubble alone shows a working dictation before the bar
/// comes up to name the step; quick ones never show the bar.
const WORK_ON_BAR_AFTER_MS: u64 = 1200;

/// When the bubble's dictation stopped and the work began, in Unix ms; 0
/// while none is working. The bar, shown later, counts its seconds from
/// here: it never sees the bubble's recording state, so its own start time
/// would carry over from the dictation before.
static BUBBLE_WORK_SINCE: AtomicU64 = AtomicU64::new(0);

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn show_dictation_state(app_handle: &AppHandle, state: &str) {
    if state == "recording" || state == "streaming" {
        set_stage(app_handle, None);
        BUBBLE_WORK_SINCE.store(0, Ordering::SeqCst);
    }
    if BUBBLE_STATES.contains(&state) && crate::bubble::takes_dictation(app_handle) {
        BUBBLE_DICTATION.store(true, Ordering::SeqCst);
        let generation = BUBBLE_STATE_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
        crate::bubble::set_dictating(app_handle, true);
        let _ = app_handle.emit_to(crate::bubble::WINDOW, "bubble-dictation", Some(state));
        // The bubble is too small to say what it's doing; a step that takes
        // a while is named on the bar, with Cancel.
        if state == "transcribing" || state == "processing" {
            let _ = BUBBLE_WORK_SINCE.compare_exchange(
                0,
                unix_ms(),
                Ordering::SeqCst,
                Ordering::SeqCst,
            );
            let handle = app_handle.clone();
            let state = state.to_string();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(WORK_ON_BAR_AFTER_MS));
                if BUBBLE_DICTATION.load(Ordering::SeqCst)
                    && BUBBLE_STATE_GENERATION.load(Ordering::SeqCst) == generation
                {
                    let since = BUBBLE_WORK_SINCE.load(Ordering::SeqCst);
                    let _ = handle.emit_to("recording_overlay", "overlay-work-since", since);
                    show_overlay_state(&handle, &state);
                }
            });
        }
        // The words come only from a model that writes as you speak.
        if state == "streaming"
            && settings::get_settings(app_handle).live_caption != LiveCaption::Off
        {
            show_caption(app_handle);
        }
        return;
    }
    show_overlay_state(app_handle, state);
}

/// The live transcript box: about three lines of words.
const CAPTION_WIDTH: f64 = 640.0;
const CAPTION_HEIGHT: f64 = 104.0;

/// Set while the bar's window shows the live transcript box, which lets
/// clicks through to what's under it.
static CAPTION_SHOWN: AtomicBool = AtomicBool::new(false);

/// Shows the live transcript box at the top or bottom of the screen the
/// bubble is on, clear of the bubble when it sits on the same edge.
fn show_caption(app_handle: &AppHandle) {
    let handle = app_handle.clone();
    let _ = app_handle.run_on_main_thread(move || {
        let Some(overlay_window) = handle.get_webview_window("recording_overlay") else {
            return;
        };
        OVERLAY_SHOW_GENERATION.fetch_add(1, Ordering::SeqCst);
        CAPTION_SHOWN.store(true, Ordering::SeqCst);
        WINDOWS_OVERLAY_IS_STREAMING.store(false, Ordering::Relaxed);
        exclude_from_capture(&overlay_window, false);
        let _ = overlay_window.set_ignore_cursor_events(true);
        // Placed again after showing, for the same DPI reason as the bar.
        if let Err(error) = place_caption(&handle, &overlay_window) {
            log::error!("Failed to place the live transcript box: {error}");
        }
        let _ = overlay_window.show();
        force_overlay_topmost(&overlay_window);
        let _ = place_caption(&handle, &overlay_window);
        let _ = handle.emit_to("recording_overlay", "show-overlay", "caption");
    });
}

/// Moves and sizes the bar's window for the live transcript box, in the
/// monitor's physical pixels like `place_windows_overlay`.
fn place_caption(
    app_handle: &AppHandle,
    overlay_window: &tauri::webview::WebviewWindow,
) -> Result<(), String> {
    use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER};

    let top = settings::get_settings(app_handle).live_caption == LiveCaption::Top;
    let (area, scale, bubble) = match crate::bubble::caption_area(app_handle) {
        Some((area, scale, bubble)) => (area, scale, Some(bubble)),
        None => {
            let monitor = get_monitor_with_cursor(app_handle)
                .ok_or_else(|| "no monitor under the cursor".to_string())?;
            (
                crate::bubble::logical_area(&monitor),
                monitor.scale_factor(),
                None,
            )
        }
    };
    let text_scale = windows_text_scale_factor();
    let r = crate::bubble_rules::caption_rect(
        area,
        (CAPTION_WIDTH * text_scale, CAPTION_HEIGHT * text_scale),
        top,
        bubble,
    );
    let hwnd = overlay_window
        .hwnd()
        .map_err(|error| format!("failed to get overlay window handle: {error}"))?;
    let px = |v: f64| (v * scale).round() as i32;
    // SAFETY: the overlay window is ours and alive.
    unsafe {
        SetWindowPos(
            hwnd,
            None,
            px(r.x),
            px(r.y),
            px(r.w).max(1),
            px(r.h).max(1),
            SWP_NOACTIVATE | SWP_NOZORDER,
        )
        .map_err(|error| format!("failed to set the box's bounds: {error}"))
    }
}

/// Set while the bubble shows a dictation, so only the dictation's end
/// takes it back, never a notice timing out on the bar.
static BUBBLE_DICTATION: AtomicBool = AtomicBool::new(false);

/// A dictation finished, failed or was cancelled: the bubble goes back to
/// rest and the bar hides.
pub fn end_dictation(app_handle: &AppHandle) {
    set_stage(app_handle, None);
    BUBBLE_WORK_SINCE.store(0, Ordering::SeqCst);
    if BUBBLE_DICTATION.swap(false, Ordering::SeqCst) {
        BUBBLE_STATE_GENERATION.fetch_add(1, Ordering::SeqCst);
        let _ = app_handle.emit_to(crate::bubble::WINDOW, "bubble-dictation", None::<&str>);
        crate::bubble::set_dictating(app_handle, false);
    }
    hide_recording_overlay(app_handle);
}

fn show_overlay_state(app_handle: &AppHandle, state: &str) {
    // Whether the overlay shows at all is governed by overlay_style; position
    // only chooses Top vs Bottom placement. Checked here (off the main thread)
    // so the common overlay-disabled case never pays for a main-thread hop.
    let settings = settings::get_settings(app_handle);
    if settings.overlay_style == OverlayStyle::None {
        return;
    }

    // The rest queries monitors and the cursor and mutates window geometry, so
    // it runs on the main thread, which owns the window. run_on_main_thread runs
    // the closure inline when already on the main thread, so this never
    // deadlocks.
    let handle = app_handle.clone();
    let state = state.to_string();
    let _ = app_handle.run_on_main_thread(move || show_overlay_state_on_main(&handle, &state));
}

fn show_overlay_state_on_main(app_handle: &AppHandle, state: &str) {
    // Size the overlay for this state (compact vs. streaming), then position it.
    let island = island_on(&settings::get_settings(app_handle));
    if island && crate::bubble::screen_covered() && state == "rest" {
        return;
    }
    let (width, height) = if island {
        (ISLAND_WIDTH, ISLAND_HEIGHT)
    } else {
        overlay_dimensions(state)
    };
    *SHOWN.lock().unwrap_or_else(|e| e.into_inner()) = Some(state.to_string());
    if let Some(overlay_window) = app_handle.get_webview_window("recording_overlay") {
        // Invalidate any delayed hide still in flight from a previous session
        // (see `hide_recording_overlay`).
        OVERLAY_SHOW_GENERATION.fetch_add(1, Ordering::SeqCst);

        WINDOWS_OVERLAY_IS_STREAMING.store(state == "streaming", Ordering::Relaxed);
        if CAPTION_SHOWN.swap(false, Ordering::SeqCst) {
            let _ = overlay_window.set_ignore_cursor_events(false);
        }

        exclude_from_capture(
            &overlay_window,
            state == "call" && CALL_CARD_HIDDEN_FROM_CAPTURE.load(Ordering::Relaxed),
        );

        let set_pos_started = std::time::Instant::now();
        if let Err(error) = place_windows_overlay(app_handle, &overlay_window, width, height) {
            log::error!("Failed to place recording overlay: {error}");
        }
        let set_pos_elapsed = set_pos_started.elapsed();

        let show_started = std::time::Instant::now();
        let _ = overlay_window.show();
        let show_elapsed = show_started.elapsed();

        // Aggressively re-assert "topmost" in the native Z-order after showing
        force_overlay_topmost(&overlay_window);
        if island {
            island_hit::arm(&overlay_window);
        } else {
            island_hit::disarm(&overlay_window);
        }

        // Re-assert bounds after show(): the pre-show move crosses the DPI
        // boundary, and tao's WM_DPICHANGED reflow clobbers the first placement.
        if let Err(error) = place_windows_overlay(app_handle, &overlay_window, width, height) {
            log::error!("Failed to re-assert recording overlay position: {error}");
        }

        log::debug!(
            "overlay '{}': set_pos={:?} show={:?}",
            state,
            set_pos_elapsed,
            show_elapsed
        );

        // `emit` would reach every window, and the bubble must not take the
        // bar's notices for its own states.
        let _ = app_handle.emit_to("recording_overlay", "show-overlay", state);
    }
}

/// Notify the visible recording overlay that the input stream has delivered its
/// first sample chunk. Audio feedback uses the same backend readiness signal,
/// but this targeted event is skipped when overlays are disabled.
pub fn emit_recording_ready(app_handle: &AppHandle) {
    if !OVERLAY_ENABLED.load(Ordering::Relaxed) {
        return;
    }

    // Showing the overlay is also queued onto the main thread. Queue readiness
    // there as well so a very fast always-on stream cannot overtake show-overlay
    // and then get reset back to the arming state by the frontend.
    let handle = app_handle.clone();
    let _ = app_handle.run_on_main_thread(move || {
        let _ = handle.emit_to("recording_overlay", "recording-ready", ());
    });
}

/// Shows the recording overlay window with fade-in animation
pub fn show_recording_overlay(app_handle: &AppHandle) {
    show_dictation_state(app_handle, "recording");
}

/// Shows the larger streaming overlay that displays live transcription text
pub fn show_streaming_overlay(app_handle: &AppHandle) {
    show_dictation_state(app_handle, "streaming");
}

/// Shows the transcribing overlay window
pub fn show_transcribing_overlay(app_handle: &AppHandle) {
    show_dictation_state(app_handle, "transcribing");
}

/// Shows the processing overlay window after a dictation
pub fn show_processing_overlay(app_handle: &AppHandle) {
    show_dictation_state(app_handle, "processing");
}

/// Shows the processing state on the bar for a transform, whose result
/// shows there too.
pub fn show_transform_working(app_handle: &AppHandle) {
    show_overlay_state(app_handle, "processing");
}

/// How long a notice stays up.
const NOTICE_MS: u64 = 2600;

/// A one-line message in the overlay, such as a word added from a selection.
/// It hides itself, unless a dictation shows the overlay in the meantime.
pub fn show_overlay_notice<T: serde::Serialize>(app_handle: &AppHandle, notice: &T) {
    show_overlay_notice_for(app_handle, notice, NOTICE_MS);
}

/// A notice that stays up for `ms`.
pub fn show_overlay_notice_for<T: serde::Serialize>(app_handle: &AppHandle, notice: &T, ms: u64) {
    show_bar(app_handle, "notice", "overlay-notice", notice, Some(ms));
}

/// Shows a Legato bar state with its payload. With `hide_after_ms`, the bar
/// hides itself then, unless something else shows it in the meantime.
pub fn show_bar<T: serde::Serialize>(
    app_handle: &AppHandle,
    state: &str,
    event: &str,
    payload: &T,
    hide_after_ms: Option<u64>,
) {
    let _ = app_handle.emit_to("recording_overlay", event, payload);
    BAR_HELD.store(false, Ordering::SeqCst);
    show_overlay_state(app_handle, state);
    let Some(ms) = hide_after_ms else {
        return;
    };
    let handle = app_handle.clone();
    std::thread::spawn(move || {
        // Showing runs on the main thread; let it land before taking the
        // generation the hide is tied to.
        std::thread::sleep(std::time::Duration::from_millis(150));
        let shown = OVERLAY_SHOW_GENERATION.load(Ordering::SeqCst);
        std::thread::sleep(std::time::Duration::from_millis(ms));
        // A panel under the pointer stays until it leaves, then a moment.
        let mut held = false;
        while BAR_HELD.load(Ordering::SeqCst)
            && OVERLAY_SHOW_GENERATION.load(Ordering::SeqCst) == shown
        {
            held = true;
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        if held {
            std::thread::sleep(std::time::Duration::from_millis(1500));
        }
        if OVERLAY_SHOW_GENERATION.load(Ordering::SeqCst) == shown
            && !BAR_HELD.load(Ordering::SeqCst)
        {
            hide_recording_overlay(&handle);
        }
    });
}

/// Whether the pointer is over a panel the bar shows for a while, such as a
/// transform's changes; it isn't hidden while it is.
static BAR_HELD: AtomicBool = AtomicBool::new(false);

pub fn hold_bar(held: bool) {
    BAR_HELD.store(held, Ordering::SeqCst);
}

/// Sets the working label ("Polishing…") the next processing state shows;
/// None goes back to the default.
pub fn set_working_label(app_handle: &AppHandle, label: Option<&str>) {
    let _ = app_handle.emit_to("recording_overlay", "overlay-working-label", label);
}

/// Names the step a dictation is on after you stop talking ("fixing",
/// "polishing", "transforming", "waiting", "pasting"); None goes back to
/// transcribing. The bar shows it with the seconds the step has taken.
pub fn set_stage(app_handle: &AppHandle, stage: Option<&str>) {
    let _ = app_handle.emit_to("recording_overlay", "overlay-stage", stage);
}

/// Updates the overlay window position based on current settings
pub fn update_overlay_position(app_handle: &AppHandle) {
    // Positioning queries monitors/cursor and moves the window, so it runs on
    // the main thread — see show_overlay_state.
    let handle = app_handle.clone();
    let _ = app_handle.run_on_main_thread(move || update_overlay_position_on_main(&handle));
}

fn update_overlay_position_on_main(app_handle: &AppHandle) {
    if let Some(overlay_window) = app_handle.get_webview_window("recording_overlay") {
        if CAPTION_SHOWN.load(Ordering::SeqCst) {
            if let Err(error) = place_caption(app_handle, &overlay_window) {
                log::error!("Failed to place the live transcript box: {error}");
            }
            return;
        }
        let state = if WINDOWS_OVERLAY_IS_STREAMING.load(Ordering::Relaxed) {
            "streaming"
        } else {
            "recording"
        };
        let (width, height) = if island_on(&settings::get_settings(app_handle)) {
            (ISLAND_WIDTH, ISLAND_HEIGHT)
        } else {
            overlay_dimensions(state)
        };
        if let Err(error) = place_windows_overlay(app_handle, &overlay_window, width, height) {
            log::error!("Failed to update recording overlay position: {error}");
        }
    }
}

/// Generation counter bumped every time the overlay is shown. The delayed
/// `hide()` below only unmaps the window if no show happened after it was
/// scheduled, so a hide left over from a finished transcription can never
/// take down the overlay of a session that started in the meantime — e.g. a
/// press the coordinator remembered while the pipeline was busy and started
/// the instant it drained, well inside the 300 ms hide delay.
static OVERLAY_SHOW_GENERATION: AtomicU64 = AtomicU64::new(0);

/// Set while a Notes recording runs: the bar shows its pill whenever
/// nothing else is on it.
static NOTES_PILL: AtomicBool = AtomicBool::new(false);

/// Shows or hides the Notes pill.
pub fn set_notes_pill(app_handle: &AppHandle, on: bool) {
    if NOTES_PILL.swap(on, Ordering::SeqCst) == on {
        return;
    }
    if on {
        show_overlay_state(app_handle, "notes");
    } else {
        hide_recording_overlay(app_handle);
    }
}

/// The call card, while one is up; it comes back whenever the bar would
/// otherwise go quiet.
static CALL_CARD: Mutex<Option<serde_json::Value>> = Mutex::new(None);
static CALL_CARD_HIDDEN_FROM_CAPTURE: AtomicBool = AtomicBool::new(true);
/// Whether the overlay window is excluded from screen capture now.
static EXCLUDED_FROM_CAPTURE: AtomicBool = AtomicBool::new(false);

fn call_card() -> Option<serde_json::Value> {
    CALL_CARD.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Shows the call card, or takes it away with None. A dictation in
/// progress keeps the bar; the card shows when it's done.
pub fn set_call_card<T: serde::Serialize>(
    app_handle: &AppHandle,
    card: Option<&T>,
    hide_from_capture: bool,
) {
    let value = card.and_then(|c| serde_json::to_value(c).ok());
    let had = std::mem::replace(
        &mut *CALL_CARD.lock().unwrap_or_else(|e| e.into_inner()),
        value.clone(),
    );
    CALL_CARD_HIDDEN_FROM_CAPTURE.store(hide_from_capture, Ordering::Relaxed);
    match value {
        Some(card) => {
            if !dictating(app_handle) {
                show_bar(app_handle, "call", "bar-call", &card, None);
            }
        }
        None if had.is_some() => hide_recording_overlay(app_handle),
        None => {}
    }
}

/// The agent waiting for an answer, from Inyon, while one waits. It shows
/// whenever no dictation or call card has the bar.
static AGENT_CARD: Mutex<Option<serde_json::Value>> = Mutex::new(None);

fn agent_card() -> Option<serde_json::Value> {
    AGENT_CARD.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

fn dictating(app_handle: &AppHandle) -> bool {
    app_handle
        .try_state::<std::sync::Arc<crate::managers::audio::AudioRecordingManager>>()
        .is_some_and(|a| a.is_recording())
}

/// Shows the agent card, updates it, or takes it away with None. A
/// dictation or a call card keeps the bar; the agent shows after them.
pub fn set_agent_card<T: serde::Serialize>(app_handle: &AppHandle, card: Option<&T>) {
    let value = card.and_then(|c| serde_json::to_value(c).ok());
    let had = std::mem::replace(
        &mut *AGENT_CARD.lock().unwrap_or_else(|e| e.into_inner()),
        value.clone(),
    );
    if had == value {
        return;
    }
    let on_bar = SHOWN.lock().unwrap_or_else(|e| e.into_inner()).as_deref() == Some("agent");
    match value {
        Some(card) => {
            let quiet = matches!(
                SHOWN.lock().unwrap_or_else(|e| e.into_inner()).as_deref(),
                None | Some("rest") | Some("notes") | Some("agent")
            );
            if call_card().is_none() && !dictating(app_handle) && quiet {
                show_bar(app_handle, "agent", "bar-agent", &card, None);
            }
        }
        None if on_bar => hide_recording_overlay(app_handle),
        None => {}
    }
}

/// A short word that an agent finished, when the bar is free.
pub fn show_agent_done(app_handle: &AppHandle, agent: &str, title: &str) {
    let quiet = matches!(
        SHOWN.lock().unwrap_or_else(|e| e.into_inner()).as_deref(),
        None | Some("rest")
    );
    if quiet && call_card().is_none() && !dictating(app_handle) {
        show_overlay_notice(
            app_handle,
            &serde_json::json!({ "kind": "agent_done", "agent": agent, "title": title }),
        );
    }
}

/// Keeps the bar out of screen sharing and recordings while it shows the
/// call card.
fn exclude_from_capture(overlay_window: &tauri::webview::WebviewWindow, exclude: bool) {
    if EXCLUDED_FROM_CAPTURE.swap(exclude, Ordering::Relaxed) == exclude {
        return;
    }
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
    };
    if let Ok(hwnd) = overlay_window.hwnd() {
        let affinity = if exclude {
            WDA_EXCLUDEFROMCAPTURE
        } else {
            WDA_NONE
        };
        // SAFETY: the overlay window is ours and alive.
        if let Err(e) = unsafe { SetWindowDisplayAffinity(hwnd, affinity) } {
            log::warn!("Couldn't change the bar's screen capture setting: {e}");
        }
    }
}

/// Hides the bar with its fade-out; while a call card or an agent's
/// question is up it comes back, and during a Notes recording the bar goes
/// back to its pill. A dictation
/// ends with `end_dictation`, which also frees the bubble.
pub fn hide_recording_overlay(app_handle: &AppHandle) {
    if let Some(card) = call_card() {
        show_bar(app_handle, "call", "bar-call", &card, None);
        return;
    }
    if let Some(card) = agent_card() {
        show_bar(app_handle, "agent", "bar-agent", &card, None);
        return;
    }
    if NOTES_PILL.load(Ordering::SeqCst) {
        show_overlay_state(app_handle, "notes");
        return;
    }
    // The island goes back to its small resting shape instead of away,
    // unless a fullscreen window has the screen.
    if island_rests(&settings::get_settings(app_handle)) && !crate::bubble::screen_covered() {
        show_overlay_state(app_handle, "rest");
        return;
    }
    // Always hide the overlay regardless of settings - if setting was changed while recording,
    // we still want to hide it properly
    if let Some(overlay_window) = app_handle.get_webview_window("recording_overlay") {
        *SHOWN.lock().unwrap_or_else(|e| e.into_inner()) = None;
        // Snapshot before doing anything observable, so any show that lands
        // after this point invalidates the delayed hide below.
        let scheduled_at = OVERLAY_SHOW_GENERATION.load(Ordering::SeqCst);
        // Emit event to trigger fade-out animation
        let _ = app_handle.emit_to("recording_overlay", "hide-overlay", ());
        // Hide the window after a short delay to allow animation to complete,
        // unless a newer session has shown the overlay again by then.
        let window_clone = overlay_window.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(300));
            if OVERLAY_SHOW_GENERATION.load(Ordering::SeqCst) != scheduled_at {
                log::debug!("Skipping stale overlay hide: a newer session is showing the overlay");
                return;
            }
            island_hit::disarm(&window_clone);
            let _ = window_clone.hide();
        });
    }
}

// Cached "overlay is enabled" flag, kept in sync with overlay_style. Avoids
// reading the Tauri store on every audio callback (~24 Hz during recording).
// Defaults to false so the audio path doesn't emit until lib.rs::setup
// populates the cache from initial settings.
static OVERLAY_ENABLED: AtomicBool = AtomicBool::new(false);

/// Update the cached overlay-enabled flag. Called from `lib.rs` at
/// startup after settings load, and from `change_overlay_style_setting`
/// whenever the user changes whether the overlay is shown.
pub fn update_overlay_enabled_cache(enabled: bool) {
    OVERLAY_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn emit_levels(app_handle: &AppHandle, levels: &[f32]) {
    // Skip emission when the overlay is disabled. The recording_overlay
    // window is created at boot regardless of overlay_style, so without this
    // guard a hidden overlay's webview would still process every event. The
    // bubble's meter still wants them with the bar off.
    let overlay = OVERLAY_ENABLED.load(Ordering::Relaxed);
    if !overlay && !crate::bubble::wants_levels() {
        return;
    }

    // Throttle to ~30 FPS. Even with the overlay enabled, the raw audio
    // callback fires far faster than the UI needs; capping emission rate
    // cuts the per-frame `eval_script`/IPC volume that drives the wry
    // memory growth in issue #1279 (upstream tauri-apps/wry#1489).
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let last = LAST_MIC_LEVEL_EMIT.load(Ordering::Relaxed);
    if now.saturating_sub(last) < EMIT_THROTTLE_MS {
        return;
    }
    LAST_MIC_LEVEL_EMIT.store(now, Ordering::Relaxed);

    // Target only the overlay window. In Tauri 2 both `AppHandle::emit`
    // and `WebviewWindow::emit` broadcast to all webviews; Tauri's
    // listener filter then skips webviews with no registered listener
    // for the event, so the settings webview never received `mic-level`.
    // But the previous dual-call pattern still produced two `eval_script`
    // calls to the overlay per audio callback (one from each .emit()).
    // `emit_to` with the overlay's window label produces a single
    // eval_script call per callback, cutting the per-callback webview
    // dispatch work in half.
    if MAIN_WANTS_MIC_LEVEL.load(Ordering::Relaxed) {
        let _ = app_handle.emit_to("main", "mic-level", levels);
    }
    if overlay {
        let _ = app_handle.emit_to("recording_overlay", "mic-level", levels);
    }
    if crate::bubble::wants_levels() {
        let _ = app_handle.emit_to(crate::bubble::WINDOW, "mic-level", levels);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monitor_hit_test_uses_half_open_physical_bounds() {
        let position = PhysicalPosition::new(-2560, -200);
        let size = PhysicalSize::new(2560, 1440);

        assert!(is_mouse_within_monitor((-2560, -200), &position, &size));
        assert!(is_mouse_within_monitor((-1, 1239), &position, &size));
        assert!(!is_mouse_within_monitor((0, 0), &position, &size));
        assert!(!is_mouse_within_monitor((-1, 1240), &position, &size));
    }

    #[test]
    fn windows_cursor_hit_test_does_not_scale_physical_monitor_bounds() {
        let position = PhysicalPosition::new(1920, 0);
        let size = PhysicalSize::new(3840, 2160);
        let cursor = (5000, 1000);

        assert!(is_mouse_within_monitor(cursor, &position, &size));

        // This is the old mixed-coordinate comparison. It excludes a cursor
        // that is visibly inside a secondary display running at 150%.
        let scale = 1.5;
        let logical_position = PhysicalPosition::new(
            (position.x as f64 / scale) as i32,
            (position.y as f64 / scale) as i32,
        );
        let logical_size = PhysicalSize::new(
            (size.width as f64 / scale) as u32,
            (size.height as f64 / scale) as u32,
        );
        assert!(!is_mouse_within_monitor(
            cursor,
            &logical_position,
            &logical_size
        ));
    }

    #[test]
    fn windows_overlay_bounds_use_destination_monitor_scale() {
        let monitor_position = PhysicalPosition::new(1920, 0);
        let monitor_size = PhysicalSize::new(3840, 2160);

        assert_eq!(
            windows_overlay_bounds(
                monitor_position,
                monitor_size,
                1.5,
                1.0,
                OVERLAY_WIDTH,
                OVERLAY_HEIGHT,
                OverlayPosition::Bottom,
            ),
            (3648, 2025, 384, 75)
        );
        assert_eq!(
            windows_overlay_bounds(
                monitor_position,
                monitor_size,
                1.5,
                1.0,
                OVERLAY_WIDTH,
                OVERLAY_HEIGHT,
                OverlayPosition::Top,
            ),
            (3648, 6, 384, 75)
        );
    }

    #[test]
    fn windows_overlay_bounds_support_negative_monitor_origins() {
        assert_eq!(
            windows_overlay_bounds(
                PhysicalPosition::new(-2560, -200),
                PhysicalSize::new(2560, 1440),
                1.25,
                1.0,
                OVERLAY_STREAM_WIDTH,
                OVERLAY_STREAM_HEIGHT,
                OverlayPosition::Bottom,
            ),
            (-1530, 1040, 500, 150)
        );
    }

    #[test]
    fn windows_overlay_bounds_grow_with_text_scale_without_moving_the_anchored_edge() {
        let monitor_position = PhysicalPosition::new(-2560, -200);
        let monitor_size = PhysicalSize::new(2560, 1440);

        let (x, y, width, height) = windows_overlay_bounds(
            monitor_position,
            monitor_size,
            1.25,
            1.1,
            OVERLAY_STREAM_WIDTH,
            OVERLAY_STREAM_HEIGHT,
            OverlayPosition::Bottom,
        );
        // 400x120 logical at 1.25 DPI x 1.1 text, still centered horizontally.
        assert_eq!((x, y, width, height), (-1555, 1025, 550, 165));
        // Bottom edge unchanged from the 1.0 case above (1040 + 150).
        assert_eq!(y + height, 1190);

        let (_, top_y, _, _) = windows_overlay_bounds(
            monitor_position,
            monitor_size,
            1.25,
            1.1,
            OVERLAY_STREAM_WIDTH,
            OVERLAY_STREAM_HEIGHT,
            OverlayPosition::Top,
        );
        // Top offset rides the DPI scale alone, so the top edge doesn't move.
        assert_eq!(top_y, -195);
    }
}

/// What the bar's window shows now (a state name), or None while hidden.
static SHOWN: Mutex<Option<String>> = Mutex::new(None);

/// Whether the bar takes the island's look: picked in Settings › Bar, and
/// the bar isn't turned off.
pub fn island_on(settings: &AppSettings) -> bool {
    settings.bar_look == BarLook::Island && settings.overlay_style != OverlayStyle::None
}

/// Whether the island stays on screen, small, with nothing to show.
fn island_rests(settings: &AppSettings) -> bool {
    island_on(settings) && settings.island_idle == IslandIdle::Small
}

/// Settings › Bar changed the look or what the island does at rest: the
/// resting island appears or goes now, and the bubble, which the island
/// stands in for, goes or comes back; anything else shown keeps going and
/// takes the new look next time.
pub fn look_changed(app_handle: &AppHandle) {
    let shown = SHOWN.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let settings = settings::get_settings(app_handle);
    match shown.as_deref() {
        None if island_rests(&settings) => show_overlay_state(app_handle, "rest"),
        Some("rest") if !island_rests(&settings) => hide_recording_overlay(app_handle),
        _ => {}
    }
    crate::bubble::bar_look_changed(app_handle);
    crate::notes::refresh_pill(app_handle);
}

/// A window started or stopped filling the screen: the resting island steps
/// aside for it, as the bubble does. A dictation or a call still shows.
pub fn island_cover_changed(app_handle: &AppHandle, covered: bool) {
    let settings = settings::get_settings(app_handle);
    if !island_rests(&settings) {
        return;
    }
    let shown = SHOWN.lock().unwrap_or_else(|e| e.into_inner()).clone();
    match (covered, shown.as_deref()) {
        (true, Some("rest")) => {
            if let Some(window) = app_handle.get_webview_window("recording_overlay") {
                *SHOWN.lock().unwrap_or_else(|e| e.into_inner()) = None;
                island_hit::disarm(&window);
                let _ = window.hide();
            }
        }
        (false, None) => show_overlay_state(app_handle, "rest"),
        _ => {}
    }
}

/// The island's window covers more of the screen than its shape, so clicks
/// pass through it except over the shape, which the page reports. A thread
/// checks the cursor against the shape 30 times a second while the island
/// shows, and sleeps while it doesn't.
pub mod island_hit {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Condvar, Mutex, Once};
    use std::time::Duration;

    /// The shape in the window, in physical pixels from its top-left corner:
    /// x, y, width, height. The page reports it with a little slack around
    /// the edge so it's easy to hit.
    static SHAPE: Mutex<(f64, f64, f64, f64)> = Mutex::new((0.0, 0.0, 0.0, 0.0));
    static ARMED: Mutex<Option<tauri::webview::WebviewWindow>> = Mutex::new(None);
    static WAKE: Condvar = Condvar::new();
    static STARTED: Once = Once::new();
    /// Whether the window takes clicks now.
    static TAKING: AtomicBool = AtomicBool::new(true);

    /// The page's report of where the shape is.
    pub fn set_shape(x: f64, y: f64, w: f64, h: f64) {
        *SHAPE.lock().unwrap_or_else(|e| e.into_inner()) = (x, y, w, h);
    }

    fn take_clicks(window: &tauri::webview::WebviewWindow, take: bool) {
        if TAKING.swap(take, Ordering::SeqCst) != take {
            let _ = window.set_ignore_cursor_events(!take);
        }
    }

    /// The island shows: clicks pass through until the cursor is on it.
    pub fn arm(window: &tauri::webview::WebviewWindow) {
        STARTED.call_once(|| {
            if let Err(e) = std::thread::Builder::new()
                .name("island-hit".into())
                .spawn(run)
            {
                log::warn!("The island's click check didn't start: {e}");
            }
        });
        take_clicks(window, false);
        *ARMED.lock().unwrap_or_else(|e| e.into_inner()) = Some(window.clone());
        WAKE.notify_all();
    }

    /// The island went, or the bar is back: the window takes clicks again
    /// (the bar's own window is all card) and the thread sleeps.
    pub fn disarm(window: &tauri::webview::WebviewWindow) {
        let was = ARMED.lock().unwrap_or_else(|e| e.into_inner()).take();
        if was.is_some() || !TAKING.load(Ordering::SeqCst) {
            take_clicks(window, true);
        }
    }

    fn run() {
        loop {
            let window = {
                let mut armed = ARMED.lock().unwrap_or_else(|e| e.into_inner());
                while armed.is_none() {
                    armed = WAKE.wait(armed).unwrap_or_else(|e| e.into_inner());
                }
                armed.clone()
            };
            let mut near = true;
            if let Some(window) = window {
                let (inside, close) = cursor_on_shape(&window);
                near = close;
                // The window may have been disarmed while we looked.
                if ARMED.lock().unwrap_or_else(|e| e.into_inner()).is_some()
                    && TAKING.load(Ordering::SeqCst) != inside
                {
                    take_clicks(&window, inside);
                    // The page can't see the pointer leave once clicks pass
                    // through, so it hears both ways from here.
                    use tauri::{Emitter, Manager};
                    let _ =
                        window
                            .app_handle()
                            .emit_to("recording_overlay", "island-pointer", inside);
                }
            }
            // Quick only while the pointer is near: from across the screen
            // it can't reach the island before the next look.
            std::thread::sleep(Duration::from_millis(if near { 33 } else { 120 }));
        }
    }

    /// Whether the cursor is on the island's shape, and whether it's within
    /// a short reach of its window.
    fn cursor_on_shape(window: &tauri::webview::WebviewWindow) -> (bool, bool) {
        use windows::Win32::Foundation::{POINT, RECT};
        use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetWindowRect};
        let Ok(hwnd) = window.hwnd() else {
            return (false, false);
        };
        let mut cursor = POINT::default();
        let mut rect = RECT::default();
        // SAFETY: plain queries into stack values on our own window.
        let ok =
            unsafe { GetCursorPos(&mut cursor).is_ok() && GetWindowRect(hwnd, &mut rect).is_ok() };
        if !ok {
            return (false, true);
        }
        const REACH: i32 = 200;
        let near = cursor.x > rect.left - REACH
            && cursor.x < rect.right + REACH
            && cursor.y > rect.top - REACH
            && cursor.y < rect.bottom + REACH;
        let (x, y, w, h) = *SHAPE.lock().unwrap_or_else(|e| e.into_inner());
        let (cx, cy) = ((cursor.x - rect.left) as f64, (cursor.y - rect.top) as f64);
        (cx >= x && cx < x + w && cy >= y && cy < y + h, near)
    }
}
