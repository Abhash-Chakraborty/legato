//! Looking at the screen: grabbing a screen or a window as pixels, and
//! reading its words with Windows' own text recognition, all on this PC.
//! Notes use it for screenshots. A dictation uses it to know the names,
//! products and code in the window it goes to (Settings › Privacy › Use
//! words on screen): the window is read once while you speak, only the
//! terms worth knowing are kept, until the dictation is pasted, and the
//! pixels are dropped at once.

use crate::screen_rules;
use anyhow::{anyhow, Result};
use log::debug;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// A screen or window as grabbed: 32-bit pixels, blue first, top row first.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

/// What the window in front showed when a dictation began.
struct Seen {
    window: isize,
    at: Instant,
    terms: Vec<String>,
}

static SEEN: Mutex<Option<Seen>> = Mutex::new(None);

/// How long what was seen stays good: a dictation and its transform.
const GOOD_FOR: Duration = Duration::from_secs(120);
/// A window read this recently isn't read again for the next dictation.
const FRESH_FOR: Duration = Duration::from_secs(8);

/// Reads the words in `window` on a thread of its own, at low priority, and
/// keeps its terms for the dictation that just began. `is_word` is the
/// spell checker's answer, which sets ordinary words apart.
pub fn look(window: isize, is_word: crate::audio_toolkit::text::IsWord) {
    {
        // Just read, for a dictation a moment ago: still good. Anything
        // else was another window, or is old.
        let mut seen = lock();
        if seen
            .as_ref()
            .is_some_and(|s| s.window == window && s.at.elapsed() < FRESH_FOR)
        {
            return;
        }
        *seen = None;
    }
    let spawned = std::thread::Builder::new()
        .name("screen-look".into())
        .spawn(move || {
            lower_priority();
            let started = Instant::now();
            let text = match grab_window(window).and_then(|frame| recognise(&frame)) {
                Ok(text) => text,
                Err(e) => {
                    debug!("The window in front wasn't read: {e}");
                    return;
                }
            };
            let terms = screen_rules::terms(&text, &*is_word);
            debug!(
                "Read the window in front in {:?}: {} term(s)",
                started.elapsed(),
                terms.len()
            );
            *lock() = Some(Seen {
                window,
                at: Instant::now(),
                terms,
            });
        });
    if let Err(e) = spawned {
        debug!("Couldn't start reading the window in front: {e}");
    }
}

/// The terms seen in the window of the dictation under way; empty when it
/// wasn't read, or isn't read yet.
pub fn seen() -> Vec<String> {
    lock()
        .as_ref()
        .filter(|s| s.at.elapsed() < GOOD_FOR)
        .map(|s| s.terms.clone())
        .unwrap_or_default()
}

/// Forgets what was seen, when the setting is turned off.
pub fn forget() {
    *lock() = None;
}

fn lock() -> std::sync::MutexGuard<'static, Option<Seen>> {
    SEEN.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(not(windows))]
fn lower_priority() {}

#[cfg(not(windows))]
pub fn grab_pointer_screen() -> Result<Frame> {
    Err(anyhow!("Screenshots need Windows"))
}

#[cfg(not(windows))]
pub fn grab_window(_window: isize) -> Result<Frame> {
    Err(anyhow!("Screenshots need Windows"))
}

#[cfg(not(windows))]
pub fn recognise(_frame: &Frame) -> Result<String> {
    Err(anyhow!("Text recognition needs Windows"))
}

/// Reading a window never competes with the dictation it's for.
#[cfg(windows)]
fn lower_priority() {
    use windows::Win32::System::Threading::{
        GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL,
    };
    unsafe {
        let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL);
    }
}

/// The whole screen the pointer is on, as it shows now. Legato's own bar,
/// bubble and Notes panel stay out of it while they're hidden from screen
/// capture (Settings › Legato bar).
#[cfg(windows)]
pub fn grab_pointer_screen() -> Result<Frame> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTOPRIMARY,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    unsafe {
        let mut pointer = POINT::default();
        // Without the pointer's place, the primary screen is taken.
        let _ = GetCursorPos(&mut pointer);
        let monitor = MonitorFromPoint(pointer, MONITOR_DEFAULTTOPRIMARY);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return Err(anyhow!("Windows didn't say where the screen is"));
        }
        grab_area(info.rcMonitor)
    }
}

/// A window as it shows now, by its handle: the part of it on its screen.
#[cfg(windows)]
pub fn grab_window(window: isize) -> Result<Frame> {
    use std::ffi::c_void;
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowRect, IsIconic, IsWindowVisible};

    let hwnd = HWND(window as *mut c_void);
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
            return Err(anyhow!("The window isn't showing"));
        }
        // The frame as drawn, without the invisible resize border.
        let mut area = RECT::default();
        let drawn = DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut area as *mut RECT as *mut c_void,
            std::mem::size_of::<RECT>() as u32,
        );
        if drawn.is_err() {
            GetWindowRect(hwnd, &mut area)?;
        }
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(monitor, &mut info).as_bool() {
            let screen = info.rcMonitor;
            area.left = area.left.max(screen.left);
            area.top = area.top.max(screen.top);
            area.right = area.right.min(screen.right);
            area.bottom = area.bottom.min(screen.bottom);
        }
        grab_area(area)
    }
}

/// The pixels of an area of the desktop.
#[cfg(windows)]
unsafe fn grab_area(area: windows::Win32::Foundation::RECT) -> Result<Frame> {
    use std::ffi::c_void;
    use windows::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
        GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CAPTUREBLT,
        DIB_RGB_COLORS, SRCCOPY,
    };

    let (width, height) = (area.right - area.left, area.bottom - area.top);
    if width <= 0 || height <= 0 {
        return Err(anyhow!("There's nothing there to copy"));
    }
    let screen = GetDC(None);
    if screen.is_invalid() {
        return Err(anyhow!("Windows gave no screen to copy from"));
    }
    let memory = CreateCompatibleDC(Some(screen));
    let bitmap = CreateCompatibleBitmap(screen, width, height);
    let before = SelectObject(memory, bitmap.into());
    let copied = BitBlt(
        memory,
        0,
        0,
        width,
        height,
        Some(screen),
        area.left,
        area.top,
        SRCCOPY | CAPTUREBLT,
    );
    let mut header = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            // Top row first.
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bgra = vec![0u8; width as usize * height as usize * 4];
    // The bitmap must be out of the memory DC before its bits are read.
    SelectObject(memory, before);
    let lines = if copied.is_ok() {
        GetDIBits(
            screen,
            bitmap,
            0,
            height as u32,
            Some(bgra.as_mut_ptr() as *mut c_void),
            &mut header,
            DIB_RGB_COLORS,
        )
    } else {
        0
    };
    let _ = DeleteObject(bitmap.into());
    let _ = DeleteDC(memory);
    ReleaseDC(None, screen);
    if let Err(e) = copied {
        return Err(anyhow!("Windows didn't copy the screen: {e}"));
    }
    if lines <= 0 {
        return Err(anyhow!("Windows didn't hand over the screen's pixels"));
    }
    Ok(Frame {
        width: width as u32,
        height: height as u32,
        bgra,
    })
}

/// The words on the screen, read by Windows' own text recognition in the
/// languages Windows is set up for. Nothing leaves the PC.
#[cfg(windows)]
pub fn recognise(frame: &Frame) -> Result<String> {
    use windows::Graphics::Imaging::{BitmapAlphaMode, BitmapPixelFormat, SoftwareBitmap};
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::Streams::DataWriter;

    let max = OcrEngine::MaxImageDimension()?;
    if frame.width > max || frame.height > max {
        return Err(anyhow!("The screen is larger than Windows reads"));
    }
    let engine = OcrEngine::TryCreateFromUserProfileLanguages()?;
    let writer = DataWriter::new()?;
    writer.WriteBytes(&frame.bgra)?;
    let buffer = writer.DetachBuffer()?;
    let bitmap = SoftwareBitmap::CreateCopyWithAlphaFromBuffer(
        &buffer,
        BitmapPixelFormat::Bgra8,
        frame.width as i32,
        frame.height as i32,
        BitmapAlphaMode::Ignore,
    )?;
    let result = engine.RecognizeAsync(&bitmap)?.join()?;
    let mut text = String::new();
    for line in result.Lines()? {
        text.push_str(&line.Text()?.to_string());
        text.push('\n');
    }
    Ok(text)
}
