//! The desktop bubble: a small Legato dot that stays on screen. Hovering it
//! opens Dictate, Notes and the Scratchpad; it can be dragged anywhere and
//! snaps to a side when dropped near one. It never takes focus, so the app
//! being dictated into keeps it, and it steps aside while a window fills
//! the screen. Placement math lives in `bubble_rules`.

use crate::bubble_rules::{self, LabelSide, Rect};
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
}

static SPOT: Mutex<Option<Spot>> = Mutex::new(None);
static EXPANDED: AtomicBool = AtomicBool::new(false);
/// Hidden by a window filling the screen.
static COVERED: AtomicBool = AtomicBool::new(false);
/// "Hide for an hour": until this many seconds since 1970.
static SNOOZED_UNTIL: AtomicU64 = AtomicU64::new(0);

/// What the bubble's page needs to lay itself out.
#[derive(Clone, Serialize, Type)]
pub struct BubbleLayout {
    pub expanded: bool,
    /// The dot's centre inside the window.
    pub dot_x: f64,
    pub dot_y: f64,
    pub labels: LabelSide,
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
fn monitor_at(app: &AppHandle, point: Option<(f64, f64)>) -> Option<tauri::Monitor> {
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
    let fraction = saved
        .map(|s| (s.x, s.y))
        .unwrap_or(bubble_rules::DEFAULT_FRACTION);
    Some(Spot {
        center: bubble_rules::from_fraction(fraction, area),
        area,
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
        bubble_rules::place(spot.center, spot.area, expanded)
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
            labels: placement.labels,
        },
    );
}

fn should_show(app: &AppHandle) -> bool {
    get_settings(app).bubble_enabled
        && !COVERED.load(Ordering::SeqCst)
        && now_s() >= SNOOZED_UNTIL.load(Ordering::SeqCst)
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
        }
    } else if window.is_visible().unwrap_or(false) {
        EXPANDED.store(false, Ordering::SeqCst);
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
        GetClassNameW, GetForegroundWindow, GetWindowRect,
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
}

/// Opens or closes the bubble's buttons as the pointer comes and goes.
pub fn expand(app: &AppHandle, expanded: bool) {
    if EXPANDED.swap(expanded, Ordering::SeqCst) != expanded {
        apply(app);
    }
}

/// Follows a drag: the dot's centre in logical screen pixels.
pub fn move_to(app: &AppHandle, x: f64, y: f64) {
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
        });
    }
    EXPANDED.store(false, Ordering::SeqCst);
    apply(app);
}

/// Ends a drag: snaps to a side when near one and remembers the spot.
pub fn drop_here(app: &AppHandle) {
    let saved = {
        let mut guard = SPOT.lock().unwrap_or_else(|e| e.into_inner());
        let Some(spot) = guard.as_mut() else {
            return;
        };
        spot.center = bubble_rules::snap(spot.center, spot.area);
        bubble_rules::to_fraction(spot.center, spot.area)
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
    });
    write_settings(app, settings);
}

/// Hides the bubble for an hour.
pub fn snooze(app: &AppHandle) {
    SNOOZED_UNTIL.store(now_s() + 3600, Ordering::SeqCst);
    refresh_visibility(app);
    debug!("Bubble hidden for an hour");
}

/// Puts the bubble back after the screens change (a monitor plugged in or
/// out, scaling changed).
pub fn screens_changed(app: &AppHandle) {
    *SPOT.lock().unwrap_or_else(|e| e.into_inner()) = saved_spot(app);
    apply(app);
}
