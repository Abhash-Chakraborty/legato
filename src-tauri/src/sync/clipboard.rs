//! The shared clipboard on the PC: noticing copies to send to the phone, and
//! putting the phone's clips on the clipboard. Private copies (password
//! managers mark theirs) and apps on the never-share list stay here.

use std::io::Cursor;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use tauri::{AppHandle, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

use super::identity::uuid;
use super::protocol::{Clip, ClipKind};
use super::SyncManager;
use crate::settings::{get_settings, SyncSettings};

/// A clip older than this on arrival goes to Recent clips, not the
/// clipboard.
pub const LATE_MS: u64 = 2 * 60 * 1000;
/// Text longer than this isn't shared.
const MAX_TEXT_BYTES: usize = 4 * 1024 * 1024;
const POLL: Duration = Duration::from_millis(400);
/// With no phone connected nothing is sent, so the watcher only glances.
const IDLE_POLL: Duration = Duration::from_secs(5);

/// The clipboard sequence number right after Legato wrote a phone's clip,
/// so the watcher doesn't send it back.
static WROTE: AtomicU32 = AtomicU32::new(0);

/// Whether `exe` (a path or file name) is on the never-share list.
pub fn never_shares(settings: &SyncSettings, exe: &str) -> bool {
    let name = exe.rsplit(['\\', '/']).next().unwrap_or(exe);
    settings
        .clipboard_never
        .iter()
        .any(|n| n.eq_ignore_ascii_case(name))
}

/// Watches for copies while clipboard sync is on and a phone is connected.
pub fn watch(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last = win::sequence();
        loop {
            let online = app
                .try_state::<Arc<SyncManager>>()
                .is_some_and(|s| s.any_online());
            std::thread::sleep(if online { POLL } else { IDLE_POLL });
            let seq = win::sequence();
            if seq == last {
                continue;
            }
            last = seq;
            let seen = Instant::now();
            if seq == WROTE.load(Ordering::SeqCst) {
                continue;
            }
            let Some(sync) = app.try_state::<Arc<SyncManager>>() else {
                continue;
            };
            if !sync.clipboard_live() {
                continue;
            }
            match read(&app, seen) {
                Ok(Some(clip)) => sync.send_clip(clip),
                Ok(None) => {}
                Err(e) => log::debug!("Didn't share a copy: {e:#}"),
            }
        }
    });
}

/// What was just copied, as a clip, unless it must stay on this PC.
pub fn read(app: &AppHandle, seen: Instant) -> Result<Option<Clip>> {
    let settings = get_settings(app).sync;
    let owner = win::owner();
    if owner.as_ref().is_some_and(|o| o.pid == std::process::id()) {
        return Ok(None);
    }
    if win::is_private() {
        log::debug!("Not sharing a copy marked private");
        return Ok(None);
    }
    let from_app = owner
        .as_ref()
        .and_then(|o| crate::context::app_for_process(o.pid));
    if let Some(app_info) = &from_app {
        if never_shares(&settings, &app_info.exe_path) {
            log::debug!("Not sharing a copy from {}", app_info.name);
            return Ok(None);
        }
    }
    let from_name = from_app.map(|a| a.name);
    let clipboard = app.clipboard();
    if let Ok(text) = clipboard.read_text() {
        if !text.is_empty() {
            if text.len() > MAX_TEXT_BYTES {
                bail!("the text is too long to share");
            }
            return Ok(Some(Clip {
                id: uuid(),
                kind: ClipKind::Text,
                text: Some(text),
                png: None,
                age_ms: seen.elapsed().as_millis() as u64,
                from_app: from_name,
            }));
        }
    }
    if settings.clipboard_images_mb == 0 {
        return Ok(None);
    }
    let Ok(image) = clipboard.read_image() else {
        return Ok(None);
    };
    let png = encode_png(image.rgba(), image.width(), image.height())?;
    if png.len() as u64 > settings.clipboard_images_mb as u64 * 1024 * 1024 {
        bail!("the image is over {} MB", settings.clipboard_images_mb);
    }
    Ok(Some(Clip {
        id: uuid(),
        kind: ClipKind::Image,
        text: None,
        png: Some(STANDARD.encode(png)),
        age_ms: seen.elapsed().as_millis() as u64,
        from_app: from_name,
    }))
}

/// The widest or tallest image a clip from the phone may unpack to.
const MAX_SIDE: u32 = 8192;

/// Puts a clip on the clipboard, without sending it back.
pub fn write(app: &AppHandle, clip: &Clip) -> Result<()> {
    let clipboard = app.clipboard();
    match clip.kind {
        ClipKind::Text => clipboard.write_text(clip.text.clone().unwrap_or_default())?,
        ClipKind::Image => {
            let png = STANDARD.decode(clip.png.as_deref().unwrap_or_default())?;
            // A small PNG can unpack to gigabytes; cap what decoding may use.
            let mut reader =
                image::ImageReader::with_format(Cursor::new(&png), image::ImageFormat::Png);
            let mut limits = image::Limits::default();
            limits.max_image_width = Some(MAX_SIDE);
            limits.max_image_height = Some(MAX_SIDE);
            limits.max_alloc = Some(256 * 1024 * 1024);
            reader.limits(limits);
            let rgba = reader.decode()?.to_rgba8();
            let (w, h) = rgba.dimensions();
            clipboard.write_image(&tauri::image::Image::new_owned(rgba.into_raw(), w, h))?;
        }
    }
    WROTE.store(win::sequence(), Ordering::SeqCst);
    Ok(())
}

fn encode_png(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let buffer = image::RgbaImage::from_raw(width, height, rgba.to_vec())
        .ok_or_else(|| anyhow!("the image's size doesn't match its pixels"))?;
    let mut out = Cursor::new(Vec::new());
    buffer.write_to(&mut out, image::ImageFormat::Png)?;
    Ok(out.into_inner())
}

pub struct Owner {
    pub pid: u32,
}

/// Copies text that mustn't linger (a pairing link holds its secret): kept
/// out of clipboard history and the cloud clipboard, and never sent to a
/// phone.
#[cfg(windows)]
pub fn copy_private(app: &AppHandle, text: &str) -> Result<()> {
    use tauri::Manager;
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| anyhow!("the main window isn't open"))?;
    let hwnd = window.hwnd()?;
    win::write_private_text(hwnd, text).map_err(|e| anyhow!(e))?;
    WROTE.store(win::sequence(), Ordering::SeqCst);
    Ok(())
}

#[cfg(not(windows))]
pub fn copy_private(app: &AppHandle, text: &str) -> Result<()> {
    app.clipboard().write_text(text.to_string())?;
    Ok(())
}

#[cfg(windows)]
mod win {
    use super::Owner;
    use windows::core::w;
    use windows::Win32::Foundation::{HANDLE, HGLOBAL};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, GetClipboardOwner, GetClipboardSequenceNumber,
        IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW,
    };
    use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
    use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

    pub fn sequence() -> u32 {
        unsafe { GetClipboardSequenceNumber() }
    }

    pub fn owner() -> Option<Owner> {
        unsafe {
            let window = GetClipboardOwner().ok()?;
            let mut pid = 0u32;
            GetWindowThreadProcessId(window, Some(&mut pid));
            (pid != 0).then_some(Owner { pid })
        }
    }

    /// Whether the copy is marked private: excluded from clipboard monitors,
    /// or kept out of clipboard history or the cloud clipboard.
    pub fn is_private() -> bool {
        unsafe {
            let exclude =
                RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing"));
            if exclude != 0 && IsClipboardFormatAvailable(exclude).is_ok() {
                return true;
            }
            let flags = [
                RegisterClipboardFormatW(w!("CanIncludeInClipboardHistory")),
                RegisterClipboardFormatW(w!("CanUploadToCloudClipboard")),
            ];
            let marked: Vec<u32> = flags
                .into_iter()
                .filter(|f| *f != 0 && IsClipboardFormatAvailable(*f).is_ok())
                .collect();
            if marked.is_empty() {
                return false;
            }
            // The flag is a DWORD; 0 means no. When it can't be read, treat
            // the copy as private.
            let mut opened = false;
            for _ in 0..5 {
                if OpenClipboard(None).is_ok() {
                    opened = true;
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            if !opened {
                return true;
            }
            let private = marked.iter().any(|f| match GetClipboardData(*f) {
                Ok(handle) => read_dword(handle).is_none_or(|v| v == 0),
                Err(_) => true,
            });
            let _ = CloseClipboard();
            private
        }
    }

    /// Puts text on the clipboard marked private, in one go, so clipboard
    /// history, the cloud clipboard and Legato's own watcher never see it.
    /// `owner` must be one of this app's windows: with no owner, Windows
    /// refuses the data after emptying the clipboard.
    pub fn write_private_text(
        owner: windows::Win32::Foundation::HWND,
        text: &str,
    ) -> Result<(), String> {
        use windows::Win32::Foundation::GlobalFree;
        use windows::Win32::System::DataExchange::{EmptyClipboard, SetClipboardData};
        use windows::Win32::System::Memory::{GlobalAlloc, GMEM_MOVEABLE};
        use windows::Win32::System::Ole::CF_UNICODETEXT;

        unsafe fn put(format: u32, bytes: &[u8]) -> bool {
            let Ok(global) = GlobalAlloc(GMEM_MOVEABLE, bytes.len()) else {
                return false;
            };
            let ptr = GlobalLock(global) as *mut u8;
            if ptr.is_null() {
                let _ = GlobalFree(Some(global));
                return false;
            }
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
            let _ = GlobalUnlock(global);
            // On success the clipboard owns the memory.
            if SetClipboardData(format, Some(HANDLE(global.0))).is_err() {
                let _ = GlobalFree(Some(global));
                return false;
            }
            true
        }

        // SAFETY: the clipboard is opened, filled and closed on this thread;
        // each buffer is handed to it or freed.
        unsafe {
            let mut opened = false;
            for _ in 0..10 {
                if OpenClipboard(Some(owner)).is_ok() {
                    opened = true;
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            if !opened {
                return Err("Another app is using the clipboard; try again".into());
            }
            let result = (|| {
                EmptyClipboard().map_err(|e| e.to_string())?;
                for (name, value) in [
                    (w!("ExcludeClipboardContentFromMonitorProcessing"), 1u32),
                    (w!("CanIncludeInClipboardHistory"), 0u32),
                    (w!("CanUploadToCloudClipboard"), 0u32),
                ] {
                    let format = RegisterClipboardFormatW(name);
                    if format != 0 {
                        put(format, &value.to_le_bytes());
                    }
                }
                let wide: Vec<u8> = text
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .flat_map(u16::to_le_bytes)
                    .collect();
                if put(CF_UNICODETEXT.0 as u32, &wide) {
                    Ok(())
                } else {
                    Err("Couldn't copy".to_string())
                }
            })();
            let _ = CloseClipboard();
            result
        }
    }

    unsafe fn read_dword(handle: HANDLE) -> Option<u32> {
        let global = HGLOBAL(handle.0);
        if GlobalSize(global) < 4 {
            return None;
        }
        let ptr = GlobalLock(global) as *const u32;
        if ptr.is_null() {
            return None;
        }
        let value = ptr.read_unaligned();
        let _ = GlobalUnlock(global);
        Some(value)
    }
}

#[cfg(not(windows))]
mod win {
    use super::Owner;

    pub fn sequence() -> u32 {
        0
    }

    pub fn owner() -> Option<Owner> {
        None
    }

    pub fn is_private() -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_never_share_list_matches_file_names_in_any_case() {
        let settings = SyncSettings::default();
        assert!(never_shares(
            &settings,
            r"C:\Program Files\KeePassXC\keepassxc.exe"
        ));
        assert!(never_shares(&settings, "Bitwarden.exe"));
        assert!(!never_shares(&settings, r"C:\Windows\notepad.exe"));
    }

    #[test]
    fn images_round_trip_through_png() {
        let rgba = vec![255u8, 0, 0, 255, 0, 255, 0, 255];
        let png = encode_png(&rgba, 2, 1).unwrap();
        let back = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(back.into_raw(), rgba);
        assert!(encode_png(&rgba, 3, 1).is_err());
    }
}
