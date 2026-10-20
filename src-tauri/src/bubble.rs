//! The desktop bubble: a small Legato dot in the middle of a screen edge.
//! Hovering it opens Dictate, Notes and the Scratchpad; while you talk it
//! shows a level meter, upright on a side and lying down at the top or
//! bottom. It can be dragged anywhere and settles in the middle of the
//! nearest edge. It never takes focus, so the app
//! being dictated into keeps it, and it steps aside while a window fills
//! the screen. Placement math lives in `bubble_rules`.

use crate::bubble_rules::{self, Edge, Rect};
use crate::settings::{get_settings, write_settings, BubbleSpot};
use log::{debug, warn};
use serde::Serialize;
use specta::Type;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, WebviewWindowBuilder};

pub const WINDOW: &str = "bubble";

/// The dot's centre in logical screen pixels, and the monitor's work area
/// it's on.
struct Spot {
    center: (f64, f64),
    area: Rect,
    /// The edge it settled on; None while it's being dragged.
    edge: Option<Edge>,
}

static SPOT: Mutex<Option<Spot>> = Mutex::new(None);
static EXPANDED: AtomicBool = AtomicBool::new(false);
/// Hidden by a window filling the screen.
static COVERED: AtomicBool = AtomicBool::new(false);
/// "Hide for an hour": until this many seconds since 1970.
static SNOOZED_UNTIL: AtomicU64 = AtomicU64::new(0);
/// The window is on screen, so it wants the microphone's levels.
static VISIBLE: AtomicBool = AtomicBool::new(false);
/// A dictation is running; the bubble shows it even over a fullscreen
/// window, since the bar leaves it to the bubble.
static DICTATING: AtomicBool = AtomicBool::new(false);

/// Whether the bubble shows, and so wants `mic-level` while you talk.
pub fn wants_levels() -> bool {
    VISIBLE.load(Ordering::Relaxed)
}

/// What the bubble's page needs to lay itself out.
#[derive(Clone, Serialize, Type)]
pub struct BubbleLayout {
    pub expanded: bool,
    /// The dot's centre inside the window.
    pub dot_x: f64,
    pub dot_y: f64,
    pub edge: Edge,
}

fn now_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn logical_area(monitor: &tauri::Monitor) -> Rect {
    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    Rect {
        x: f64::from(area.position.x) / scale,
        y: f64::from(area.position.y) / scale,
        w: f64::from(area.size.width) / scale,
        h: f64::from(area.size.height) / scale,
    }
}

/// The monitor a logical point is on, or the primary one.
pub(crate) fn monitor_at(app: &AppHandle, point: Option<(f64, f64)>) -> Option<tauri::Monitor> {
    if let Some((x, y)) = point {
        if let Ok(monitors) = app.available_monitors() {
            for m in monitors {
                let scale = m.scale_factor();
                let pos = m.position();
                let size = m.size();
                let (mx, my) = (f64::from(pos.x) / scale, f64::from(pos.y) / scale);
                let (mw, mh) = (
                    f64::from(size.width) / scale,
                    f64::from(size.height) / scale,
                );
                if x >= mx && x < mx + mw && y >= my && y < my + mh {
                    return Some(m);
                }
            }
        }
    }
    app.primary_monitor().ok().flatten()
}

/// Where the saved spot is now, on the monitor it was saved on when that
/// one is still connected.
fn saved_spot(app: &AppHandle) -> Option<Spot> {
    let settings = get_settings(app);
    let monitors = app.available_monitors().ok()?;
    let saved = settings.bubble_spot.as_ref();
    let monitor = saved
        .and_then(|s| {
            monitors
                .iter()
                .find(|m| m.name().is_some_and(|n| *n == s.monitor))
                .cloned()
        })
        .or_else(|| app.primary_monitor().ok().flatten())
        .or_else(|| monitors.first().cloned())?;
    let area = logical_area(&monitor);
    // Spots saved before edges were a thing pick the nearest edge.
    let edge = saved.map_or(Edge::default(), |s| {
        s.edge.unwrap_or_else(|| {
            bubble_rules::nearest_edge((area.x + s.x * area.w, area.y + s.y * area.h), area)
        })
    });
    Some(Spot {
        center: bubble_rules::edge_center(edge, area),
        area,
        edge: Some(edge),
    })
}

/// Moves and sizes the window for the dot and tells the page.
fn apply(app: &AppHandle) {
    let Some(window) = app.get_webview_window(WINDOW) else {
        return;
    };
    let expanded = EXPANDED.load(Ordering::SeqCst);
    let placement = {
        let guard = SPOT.lock().unwrap_or_else(|e| e.into_inner());
        let Some(spot) = guard.as_ref() else {
            return;
        };
        let edge = spot
            .edge
            .unwrap_or_else(|| bubble_rules::nearest_edge(spot.center, spot.area));
        bubble_rules::place(spot.center, edge, spot.area, expanded)
    };
    let r = placement.window;
    // Size first when growing and position first when shrinking, so the
    // dot doesn't jump for a frame.
    let size = tauri::LogicalSize::new(r.w, r.h);
    let pos = tauri::LogicalPosition::new(r.x, r.y);
    if expanded {
        let _ = window.set_size(size);
        let _ = window.set_position(pos);
    } else {
        let _ = window.set_position(pos);
        let _ = window.set_size(size);
    }
    let _ = window.emit(
        "bubble-layout",
        BubbleLayout {
            expanded,
            dot_x: placement.dot.0,
            dot_y: placement.dot.1,
            edge: placement.edge,
        },
    );
}

fn should_show(app: &AppHandle) -> bool {
    takes_dictation(app) && (!COVERED.load(Ordering::SeqCst) || DICTATING.load(Ordering::SeqCst))
}

/// Whether the bubble is on (and not hidden for an hour), so a dictation
/// shows on it rather than on the bar.
pub fn takes_dictation(app: &AppHandle) -> bool {
    get_settings(app).bubble_enabled && now_s() >= SNOOZED_UNTIL.load(Ordering::SeqCst)
}

/// A dictation started or ended: over a fullscreen window the bubble comes
/// out for it and steps aside again after.
pub fn set_dictating(app: &AppHandle, on: bool) {
    if DICTATING.swap(on, Ordering::SeqCst) == on || !COVERED.load(Ordering::SeqCst) {
        return;
    }
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || refresh_visibility(&handle));
}

/// Shows or hides the window to match the setting, the snooze and what's
/// on screen, without taking focus.
fn refresh_visibility(app: &AppHandle) {
    let Some(window) = app.get_webview_window(WINDOW) else {
        return;
    };
    if should_show(app) {
        if !window.is_visible().unwrap_or(false) {
            apply(app);
            show_topmost(&window);
            VISIBLE.store(true, Ordering::Relaxed);
        }
    } else if window.is_visible().unwrap_or(false) {
        EXPANDED.store(false, Ordering::SeqCst);
        VISIBLE.store(false, Ordering::Relaxed);
        let _ = window.hide();
    }
}

fn show_topmost(window: &tauri::WebviewWindow) {
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
    };
    let w = window.clone();
    let _ = window.run_on_main_thread(move || {
        if let Ok(hwnd) = w.hwnd() {
            // SAFETY: the window is ours and alive; no size, move or focus.
            unsafe {
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

/// Creates the bubble's window at startup and starts watching for windows
/// that fill the screen.
pub fn init(app: &AppHandle) {
    *SPOT.lock().unwrap_or_else(|e| e.into_inner()) = saved_spot(app);
    let mut builder = WebviewWindowBuilder::new(
        app,
        WINDOW,
        tauri::WebviewUrl::App("src/bubble/index.html".into()),
    )
    .title("Legato bubble")
    .resizable(false)
    .inner_size(bubble_rules::COLLAPSED.0, bubble_rules::COLLAPSED.1)
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
    if let Err(e) = builder.build() {
        warn!("Couldn't create the bubble: {e}");
        return;
    }
    apply(app);
    refresh_visibility(app);

    let app = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(1500));
        let covered = foreground_fills_screen();
        let was = COVERED.swap(covered, Ordering::SeqCst);
        let snooze = SNOOZED_UNTIL.load(Ordering::SeqCst);
        if was != covered || (snooze != 0 && now_s() >= snooze) {
            if snooze != 0 && now_s() >= snooze {
                SNOOZED_UNTIL.store(0, Ordering::SeqCst);
                crate::notes::refresh_pill(&app);
            }
            let app2 = app.clone();
            let _ = app.run_on_main_thread(move || refresh_visibility(&app2));
        }
    });
}

/// Whether the window in front fills its monitor (a video, a game, a
/// slideshow). The desktop and the taskbar don't count.
fn foreground_fills_screen() -> bool {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetForegroundWindow, GetWindowLongW, GetWindowRect, IsZoomed, GWL_STYLE,
        WS_CAPTION,
    };
    // SAFETY: plain queries on the foreground window handle, which may be
    // null; every call tolerates that and reports failure.
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return false;
        }
        let mut class = [0u16; 64];
        let n = GetClassNameW(hwnd, &mut class);
        let class = String::from_utf16_lossy(&class[..n.max(0) as usize]);
        if matches!(
            class.as_str(),
            "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
        ) {
            return false;
        }
        // A maximized window with a title bar fills the screen too when the
        // taskbar hides itself, but it isn't a video or a game.
        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
        if IsZoomed(hwnd).as_bool() || style & WS_CAPTION.0 == WS_CAPTION.0 {
            return false;
        }
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return false;
        }
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return false;
        }
        let m = info.rcMonitor;
        bubble_rules::covers(
            (rect.left, rect.top, rect.right, rect.bottom),
            (m.left, m.top, m.right, m.bottom),
        )
    }
}

/// Turns the bubble on or off (Settings, or its own menu).
pub fn set_enabled(app: &AppHandle, enabled: bool) {
    let mut settings = get_settings(app);
    settings.bubble_enabled = enabled;
    write_settings(app, settings);
    if enabled {
        SNOOZED_UNTIL.store(0, Ordering::SeqCst);
    }
    refresh_visibility(app);
    // Notes in progress move between the bubble and the bar's pill.
    crate::notes::refresh_pill(app);
}

/// Opens or closes the bubble's buttons as the pointer comes and goes.
pub fn expand(app: &AppHandle, expanded: bool) {
    if DRAGGING.load(Ordering::SeqCst) {
        return;
    }
    if EXPANDED.swap(expanded, Ordering::SeqCst) != expanded {
        apply(app);
        if expanded {
            watch_pointer(app);
        }
    }
}

/// Watching whether the pointer left the open bubble.
static WATCHING: AtomicBool = AtomicBool::new(false);

/// Closes the open bubble once the pointer has been off it for a moment.
/// The page hears the pointer leave only when it moves out in steps; one
/// that jumps to another window (a click that opens the Notes panel or the
/// Scratchpad) leaves it open otherwise.
fn watch_pointer(app: &AppHandle) {
    if WATCHING.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let mut away = 0;
        while EXPANDED.load(Ordering::SeqCst) && away < 3 {
            std::thread::sleep(Duration::from_millis(150));
            let inside = (|| {
                let window = app.get_webview_window(WINDOW)?;
                let (pos, size) = (window.outer_position().ok()?, window.outer_size().ok()?);
                let p = app.cursor_position().ok()?;
                Some(
                    p.x >= f64::from(pos.x)
                        && p.x < f64::from(pos.x) + f64::from(size.width)
                        && p.y >= f64::from(pos.y)
                        && p.y < f64::from(pos.y) + f64::from(size.height),
                )
            })()
            .unwrap_or(true);
            away = if inside || DRAGGING.load(Ordering::SeqCst) {
                0
            } else {
                away + 1
            };
        }
        WATCHING.store(false, Ordering::SeqCst);
        if away >= 3 {
            expand(&app, false);
        } else if EXPANDED.load(Ordering::SeqCst) {
            // Opened again just as this one stopped watching.
            watch_pointer(&app);
        }
    });
}

/// Follows a drag: the dot's centre in logical screen pixels.
fn move_to(app: &AppHandle, x: f64, y: f64) {
    let monitor = monitor_at(app, Some((x, y)));
    {
        let mut guard = SPOT.lock().unwrap_or_else(|e| e.into_inner());
        let Some(m) = monitor else {
            return;
        };
        let area = logical_area(&m);
        *guard = Some(Spot {
            center: bubble_rules::clamp_center((x, y), area),
            area,
            edge: None,
        });
    }
    EXPANDED.store(false, Ordering::SeqCst);
    apply(app);
}

/// A drag in progress, followed from the system cursor.
static DRAGGING: AtomicBool = AtomicBool::new(false);

/// The cursor in logical pixels of the monitor it's on.
fn cursor_logical(app: &AppHandle) -> Option<(f64, f64)> {
    let p = app.cursor_position().ok()?;
    let monitors = app.available_monitors().ok()?;
    let m = monitors.iter().find(|m| {
        let (pos, size) = (m.position(), m.size());
        p.x >= f64::from(pos.x)
            && p.x < f64::from(pos.x) + f64::from(size.width)
            && p.y >= f64::from(pos.y)
            && p.y < f64::from(pos.y) + f64::from(size.height)
    })?;
    let scale = m.scale_factor();
    Some((p.x / scale, p.y / scale))
}

/// Whether the left mouse button is down now.
fn button_down() -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
        (unsafe { GetAsyncKeyState(i32::from(VK_LBUTTON.0)) } as u16) & 0x8000 != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// A press on the bubble being watched, until the button comes up.
static PRESSED: AtomicBool = AtomicBool::new(false);
/// How far, in logical pixels, the cursor moves before a press is a drag.
const DRAG_PX: f64 = 5.0;

/// Watches a press on the bubble. Once the cursor moves a few pixels with
/// the button held, the dot follows the system cursor until the button
/// comes up, then settles on the nearest edge. Watching the cursor here
/// rather than the page's pointer events keeps up with a quick flick, which
/// leaves the small window behind, and with a press that lands just as the
/// bubble opens and swaps the dot for its buttons; either loses the page's
/// pointer capture. `grab` is the dot's centre less the cursor, in logical
/// pixels.
pub fn press(app: &AppHandle, grab: (f64, f64)) {
    if PRESSED.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let start = cursor_logical(&app);
        let mut last = None;
        while button_down() {
            if let Some((x, y)) = cursor_logical(&app) {
                let moved = start.is_some_and(|(sx, sy)| (x - sx).hypot(y - sy) >= DRAG_PX);
                if moved && !DRAGGING.swap(true, Ordering::SeqCst) {
                    EXPANDED.store(false, Ordering::SeqCst);
                }
                if DRAGGING.load(Ordering::SeqCst) {
                    let center = (x + grab.0, y + grab.1);
                    if last != Some(center) {
                        move_to(&app, center.0, center.1);
                        last = Some(center);
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(8));
        }
        if DRAGGING.load(Ordering::SeqCst) {
            drop_here(&app);
            DRAGGING.store(false, Ordering::SeqCst);
            // The page may never hear the button come up; tell it the drag
            // ended.
            let _ = app.emit_to(WINDOW, "bubble-dropped", ());
        }
        PRESSED.store(false, Ordering::SeqCst);
    });
}

/// Ends a drag: settles in the middle of the nearest edge and remembers it.
fn drop_here(app: &AppHandle) {
    let (edge, saved) = {
        let mut guard = SPOT.lock().unwrap_or_else(|e| e.into_inner());
        let Some(spot) = guard.as_mut() else {
            return;
        };
        let (edge, center) = bubble_rules::snap(spot.center, spot.area);
        spot.center = center;
        spot.edge = Some(edge);
        let a = spot.area;
        (edge, ((center.0 - a.x) / a.w, (center.1 - a.y) / a.h))
    };
    apply(app);
    let name = monitor_at(app, {
        let guard = SPOT.lock().unwrap_or_else(|e| e.into_inner());
        guard.as_ref().map(|s| s.center)
    })
    .and_then(|m| m.name().cloned())
    .unwrap_or_default();
    let mut settings = get_settings(app);
    settings.bubble_spot = Some(BubbleSpot {
        monitor: name,
        x: saved.0,
        y: saved.1,
        edge: Some(edge),
    });
    write_settings(app, settings);
}

/// Hides the bubble for an hour.
pub fn snooze(app: &AppHandle) {
    SNOOZED_UNTIL.store(now_s() + 3600, Ordering::SeqCst);
    refresh_visibility(app);
    crate::notes::refresh_pill(app);
    debug!("Bubble hidden for an hour");
}

/// Puts the bubble back after the screens change (a monitor plugged in or
/// out, scaling changed).
pub fn screens_changed(app: &AppHandle) {
    *SPOT.lock().unwrap_or_else(|e| e.into_inner()) = saved_spot(app);
    apply(app);
}
