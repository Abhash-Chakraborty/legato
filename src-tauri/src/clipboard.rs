use crate::input::{self, EnigoState};
use crate::settings::{
    get_settings, AfterPaste, AppSettings, AutoSubmitKey, ClipboardHandling, PasteMethod,
};
use enigo::{Direction, Enigo, Key, Keyboard};
use log::info;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

fn with_enigo<T>(
    app_handle: &AppHandle,
    f: impl FnOnce(&mut Enigo) -> Result<T, String>,
) -> Result<T, String> {
    let enigo_state = app_handle
        .try_state::<EnigoState>()
        .ok_or("Enigo state not initialized")?;
    let mut enigo = enigo_state
        .0
        .lock()
        .map_err(|e| format!("Failed to lock Enigo: {}", e))?;
    f(&mut enigo)
}

fn write_text_to_clipboard(app_handle: &AppHandle, text: &str) -> Result<(), String> {
    // Another app (a clipboard manager, a browser reading a copy) can hold
    // the clipboard for a moment; giving up at once sent the dictation to
    // the Scratchpad instead of pasting it.
    let mut error = String::new();
    for attempt in 1..=6 {
        match crate::sync::clipboard::paste_write(|| app_handle.clipboard().write_text(text)) {
            Ok(()) => return Ok(()),
            Err(e) => error = e.to_string(),
        }
        std::thread::sleep(Duration::from_millis(40 * attempt));
    }
    Err(format!("Failed to write to clipboard: {}", error))
}

fn finish_clipboard_paste(
    paste_result: Result<(), String>,
    paste_delay_after_ms: u64,
    restore_clipboard: impl FnOnce(),
) -> Result<(), String> {
    std::thread::sleep(Duration::from_millis(paste_delay_after_ms));
    restore_clipboard();
    paste_result
}

/// Pastes text using the clipboard: saves current content, writes text, sends paste keystroke, restores clipboard.
fn paste_via_clipboard(
    text: &str,
    app_handle: &AppHandle,
    paste_method: &PasteMethod,
    paste_delay_ms: u64,
    paste_delay_after_ms: u64,
) -> Result<(), String> {
    let clipboard = app_handle.clipboard();
    let saved_text = clipboard.read_text().ok().filter(|t| !t.is_empty());
    // Only probe for an image when there is no text to restore. Text is by far the
    // common case, and reading an image decodes the full bitmap, so this keeps the
    // text path exactly as cheap as it was before.
    let saved_image = if saved_text.is_none() {
        clipboard.read_image().ok().map(|image| image.to_owned())
    } else {
        None
    };
    // Neither: files copied in Explorer, say. Their data is kept as it is.
    let saved_other = if saved_text.is_none() && saved_image.is_none() {
        raw::snapshot()
    } else {
        None
    };

    // Write text to clipboard first
    write_text_to_clipboard(app_handle, text)?;

    std::thread::sleep(Duration::from_millis(paste_delay_ms));

    // Capture key injection errors so the original clipboard is restored before
    // propagating them to the caller.
    let paste_result = with_enigo(app_handle, |enigo| match paste_method {
        // The legacy path cannot detect a mistimed chord, so it keeps the
        // conservative 100ms modifier hold.
        PasteMethod::CtrlV => input::send_paste_ctrl_v(enigo, 100),
        PasteMethod::CtrlShiftV => input::send_paste_ctrl_shift_v(enigo, 100),
        PasteMethod::ShiftInsert => input::send_paste_shift_insert(enigo, 100),
        _ => Err("Invalid paste method for clipboard paste".into()),
    });

    finish_clipboard_paste(paste_result, paste_delay_after_ms, || {
        // Restore original clipboard content even when key injection failed.
        // Text takes priority so this path stays identical to the previous behavior;
        // an image is only restored when the clipboard held no text at all, which is
        // the case that used to silently wipe screenshots.
        if let Some(clipboard_content) = saved_text {
            let _ = write_text_to_clipboard(app_handle, &clipboard_content);
        } else if let Some(image) = saved_image {
            info!("Restoring image to clipboard");
            let _ = crate::sync::clipboard::paste_write(|| clipboard.write_image(&image));
        } else if let Some(other) = saved_other
            .filter(|other| crate::sync::clipboard::paste_write(|| raw::restore(app_handle, other)))
        {
            info!("Restored {} clipboard format(s)", other.formats());
        } else {
            // Nothing was there to begin with — don't leave the transcription behind.
            let _ = clipboard.clear();
        }
    })
}

/// Types text directly by simulating individual key presses.
fn paste_direct(text: &str, app_handle: &AppHandle) -> Result<(), String> {
    with_enigo(app_handle, |enigo| input::paste_text_direct(enigo, text))
}

pub(crate) fn send_return_key(enigo: &mut Enigo, key_type: AutoSubmitKey) -> Result<(), String> {
    match key_type {
        AutoSubmitKey::Enter => {
            enigo
                .key(Key::Return, Direction::Press)
                .map_err(|e| format!("Failed to press Return key: {}", e))?;
            enigo
                .key(Key::Return, Direction::Release)
                .map_err(|e| format!("Failed to release Return key: {}", e))?;
        }
        AutoSubmitKey::CtrlEnter => {
            enigo
                .key(Key::Control, Direction::Press)
                .map_err(|e| format!("Failed to press Control key: {}", e))?;
            enigo
                .key(Key::Return, Direction::Press)
                .map_err(|e| format!("Failed to press Return key: {}", e))?;
            enigo
                .key(Key::Return, Direction::Release)
                .map_err(|e| format!("Failed to release Return key: {}", e))?;
            enigo
                .key(Key::Control, Direction::Release)
                .map_err(|e| format!("Failed to release Control key: {}", e))?;
        }
        AutoSubmitKey::CmdEnter => {
            enigo
                .key(Key::Meta, Direction::Press)
                .map_err(|e| format!("Failed to press Meta key: {}", e))?;
            enigo
                .key(Key::Return, Direction::Press)
                .map_err(|e| format!("Failed to press Return key: {}", e))?;
            enigo
                .key(Key::Return, Direction::Release)
                .map_err(|e| format!("Failed to release Return key: {}", e))?;
            enigo
                .key(Key::Meta, Direction::Release)
                .map_err(|e| format!("Failed to release Meta key: {}", e))?;
        }
    }

    Ok(())
}

fn should_send_auto_submit(auto_submit: bool, paste_method: PasteMethod) -> bool {
    auto_submit && paste_method != PasteMethod::None
}

/// Paste settings for one kind of app, from Style. `None` fields use
/// Settings › Output.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PasteOverrides {
    pub paste_method: Option<PasteMethod>,
    pub after_paste: Option<AfterPaste>,
    pub trailing_space: Option<bool>,
}

impl PasteOverrides {
    /// What Style says for the app the text goes to.
    pub fn for_category(settings: &AppSettings, category: Option<&str>) -> Self {
        match category {
            Some(category) if settings.style_enabled => {
                let style = settings.styles.get(category);
                Self {
                    paste_method: style.paste_method,
                    after_paste: style.after_paste,
                    trailing_space: style.trailing_space,
                }
            }
            _ => Self::default(),
        }
    }

    pub fn apply(&self, settings: &mut AppSettings) {
        if let Some(method) = self.paste_method {
            settings.paste_method = method;
        }
        match self.after_paste {
            Some(AfterPaste::Nothing) => settings.auto_submit = false,
            Some(AfterPaste::Enter) => {
                settings.auto_submit = true;
                settings.auto_submit_key = AutoSubmitKey::Enter;
            }
            Some(AfterPaste::CtrlEnter) => {
                settings.auto_submit = true;
                settings.auto_submit_key = AutoSubmitKey::CtrlEnter;
            }
            None => {}
        }
        if let Some(space) = self.trailing_space {
            settings.append_trailing_space = space;
        }
    }
}

/// Pastes a dictation with the paste settings of the app it goes to.
/// The last text Legato pasted, the window it went into and when, so a
/// transform with nothing selected can select it again.
static LAST_PASTE: Mutex<Option<(isize, String, Instant)>> = Mutex::new(None);

fn remember_paste(text: &str) {
    let last = crate::context::foreground_handle().map(|h| (h, text.to_string(), Instant::now()));
    *LAST_PASTE.lock().unwrap_or_else(|e| e.into_inner()) = last;
}

/// What Legato last pasted, when it went into the window in front within
/// `within`.
pub fn last_paste_here(within: Duration) -> Option<String> {
    let front = crate::context::foreground_handle()?;
    LAST_PASTE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .filter(|(handle, _, at)| *handle == front && at.elapsed() < within)
        .map(|(_, text, _)| text.clone())
}

pub fn paste_dictation(
    text: String,
    app_handle: AppHandle,
    overrides: PasteOverrides,
) -> Result<(), String> {
    paste_with(text, app_handle, true, overrides)
}

/// Stacked messages: pastes and sends each part, then the last one with the
/// paste settings of the app. Stops at the first part that fails.
pub fn paste_stacked(
    parts: Vec<String>,
    app_handle: AppHandle,
    overrides: PasteOverrides,
) -> Result<(), String> {
    let last = parts.len().saturating_sub(1);
    for (i, part) in parts.into_iter().enumerate() {
        let send = PasteOverrides {
            after_paste: Some(AfterPaste::Enter),
            trailing_space: Some(false),
            ..overrides
        };
        paste_with(
            part,
            app_handle.clone(),
            true,
            if i == last { overrides } else { send },
        )?;
        if i != last {
            // The chat app clears its box after sending.
            std::thread::sleep(Duration::from_millis(250));
        }
    }
    Ok(())
}

/// Presses Enter in the app in front: "press enter" said on its own.
pub fn press_enter(app_handle: &AppHandle) -> Result<(), String> {
    with_enigo(app_handle, |enigo| {
        send_return_key(enigo, AutoSubmitKey::Enter)
    })
}

/// Pastes a transform's result: no trailing space and no auto-submit, since
/// it replaces text the person is still working on.
pub fn paste_plain(text: String, app_handle: AppHandle) -> Result<(), String> {
    paste_with(text, app_handle, false, PasteOverrides::default())
}

fn paste_with(
    text: String,
    app_handle: AppHandle,
    dictation: bool,
    overrides: PasteOverrides,
) -> Result<(), String> {
    let mut settings = get_settings(&app_handle);
    overrides.apply(&mut settings);
    if !dictation {
        settings.append_trailing_space = false;
        settings.auto_submit = false;
    }
    let paste_method = settings.paste_method;
    let paste_delay_ms = settings.paste_delay_ms;
    let paste_delay_after_ms = settings.paste_delay_after_ms;

    // Append trailing space if setting is enabled
    let text = if settings.append_trailing_space {
        format!("{} ", text)
    } else {
        text
    };

    info!(
        "Using paste method: {:?}, delay before: {}ms, delay after: {}ms",
        paste_method, paste_delay_ms, paste_delay_after_ms
    );
    // Enter after the paste moves the cursor off the text.
    let reselectable = paste_method != PasteMethod::None && !settings.auto_submit;

    // Perform the paste operation
    match paste_method {
        PasteMethod::None => {
            info!("PasteMethod::None selected - skipping paste action");
        }
        PasteMethod::Direct => {
            paste_direct(&text, &app_handle)?;
        }
        PasteMethod::CtrlV | PasteMethod::CtrlShiftV | PasteMethod::ShiftInsert => {
            // Debug-gated receipt-sequenced paste (#502): restore the clipboard
            // after the target actually reads the transcript, not on a timer.
            // On success it fully handles the paste (including auto-submit and
            // clipboard handling) asynchronously; on failure fall through to
            // the legacy path untouched.
            if settings.reliable_paste {
                let reliable_result = with_enigo(&app_handle, |enigo| {
                    crate::paste_tx::try_reliable_paste(
                        &text,
                        &app_handle,
                        &paste_method,
                        enigo,
                        settings.auto_submit,
                        settings.auto_submit_key,
                        settings.clipboard_handling,
                    )
                });
                match reliable_result {
                    Ok(()) => {
                        if reselectable {
                            remember_paste(&text);
                        }
                        return Ok(());
                    }
                    Err(e) => {
                        log::warn!("Reliable paste unavailable ({e}); falling back to legacy paste")
                    }
                }
            }
            paste_via_clipboard(
                &text,
                &app_handle,
                &paste_method,
                paste_delay_ms,
                paste_delay_after_ms,
            )?
        }
    }

    if should_send_auto_submit(settings.auto_submit, paste_method) {
        std::thread::sleep(Duration::from_millis(50));
        if let Err(error) = with_enigo(&app_handle, |enigo| {
            send_return_key(enigo, settings.auto_submit_key)
        }) {
            log::warn!("Paste succeeded, but auto-submit failed: {error}");
        }
    }

    if reselectable {
        remember_paste(&text);
    }

    // After pasting, optionally copy to clipboard based on settings
    if settings.clipboard_handling == ClipboardHandling::CopyToClipboard {
        write_text_to_clipboard(&app_handle, &text)?;
    }

    Ok(())
}

/// What the clipboard held when it was neither text nor an image (files
/// copied in Explorer, a format only one app reads), copied byte for byte so
/// a paste can put it back.
mod raw {
    use tauri::{AppHandle, Manager};

    /// Formats whose data is a GDI handle rather than memory, and the
    /// private and GDI ranges, which can't be copied as bytes. Windows makes
    /// the bitmap formats again from CF_DIB.
    const NOT_MEMORY: [u32; 8] = [2, 3, 9, 14, 0x80, 0x82, 0x83, 0x8E];
    const NOT_MEMORY_RANGE: std::ops::RangeInclusive<u32> = 0x200..=0x3FF;
    /// More than this isn't kept: the clipboard is cleared as before.
    const MAX_BYTES: usize = 64 * 1024 * 1024;

    pub struct Snapshot(Vec<(u32, Vec<u8>)>);

    impl Snapshot {
        pub fn formats(&self) -> usize {
            self.0.len()
        }
    }

    /// The clipboard's formats as bytes, or None when it's empty, too big,
    /// or can't be opened.
    pub fn snapshot() -> Option<Snapshot> {
        use windows::Win32::Foundation::HGLOBAL;
        use windows::Win32::System::DataExchange::{
            CloseClipboard, EnumClipboardFormats, GetClipboardData,
        };
        use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};

        // SAFETY: the clipboard is opened and closed on this thread; each
        // handle is read under GlobalLock and copied before it's unlocked.
        unsafe {
            if !open(None) {
                return None;
            }
            let mut out = Vec::new();
            let mut total = 0usize;
            let mut format = 0u32;
            let mut fits = true;
            loop {
                format = EnumClipboardFormats(format);
                if format == 0 {
                    break;
                }
                if NOT_MEMORY.contains(&format) || NOT_MEMORY_RANGE.contains(&format) {
                    continue;
                }
                let Ok(handle) = GetClipboardData(format) else {
                    continue;
                };
                let global = HGLOBAL(handle.0);
                let size = GlobalSize(global);
                if size == 0 {
                    continue;
                }
                total += size;
                if total > MAX_BYTES {
                    fits = false;
                    break;
                }
                let ptr = GlobalLock(global) as *const u8;
                if ptr.is_null() {
                    continue;
                }
                out.push((format, std::slice::from_raw_parts(ptr, size).to_vec()));
                let _ = GlobalUnlock(global);
            }
            let _ = CloseClipboard();
            (fits && !out.is_empty()).then_some(Snapshot(out))
        }
    }

    /// Puts a snapshot back. Windows only keeps data set by an owner window
    /// after emptying the clipboard, so one of Legato's windows owns it.
    pub fn restore(app: &AppHandle, snapshot: &Snapshot) -> bool {
        use windows::Win32::Foundation::{GlobalFree, HANDLE};
        use windows::Win32::System::DataExchange::{
            CloseClipboard, EmptyClipboard, SetClipboardData,
        };
        use windows::Win32::System::Memory::{
            GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE,
        };

        let Some(owner) = app.webview_windows().values().find_map(|w| w.hwnd().ok()) else {
            return false;
        };
        // SAFETY: as above; each buffer is handed to the clipboard or freed.
        unsafe {
            if !open(Some(owner)) {
                return false;
            }
            let mut put = 0;
            if EmptyClipboard().is_ok() {
                for (format, bytes) in &snapshot.0 {
                    let Ok(global) = GlobalAlloc(GMEM_MOVEABLE, bytes.len()) else {
                        continue;
                    };
                    let ptr = GlobalLock(global) as *mut u8;
                    if ptr.is_null() {
                        let _ = GlobalFree(Some(global));
                        continue;
                    }
                    std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
                    let _ = GlobalUnlock(global);
                    // On success the clipboard owns the memory.
                    if SetClipboardData(*format, Some(HANDLE(global.0))).is_ok() {
                        put += 1;
                    } else {
                        let _ = GlobalFree(Some(global));
                    }
                }
            }
            let _ = CloseClipboard();
            put > 0
        }
    }

    /// Opens the clipboard, waiting a moment for another app to let go.
    unsafe fn open(owner: Option<windows::Win32::Foundation::HWND>) -> bool {
        use windows::Win32::System::DataExchange::OpenClipboard;
        for _ in 0..10 {
            if OpenClipboard(owner).is_ok() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn auto_submit_requires_setting_enabled() {
        assert!(!should_send_auto_submit(false, PasteMethod::CtrlV));
        assert!(!should_send_auto_submit(false, PasteMethod::Direct));
    }

    #[test]
    fn auto_submit_skips_none_paste_method() {
        assert!(!should_send_auto_submit(true, PasteMethod::None));
    }

    #[test]
    fn auto_submit_runs_for_active_paste_methods() {
        assert!(should_send_auto_submit(true, PasteMethod::CtrlV));
        assert!(should_send_auto_submit(true, PasteMethod::Direct));
        assert!(should_send_auto_submit(true, PasteMethod::CtrlShiftV));
        assert!(should_send_auto_submit(true, PasteMethod::ShiftInsert));
    }

    #[test]
    fn clipboard_is_restored_before_key_injection_error_is_returned() {
        let restored = Cell::new(false);
        let result = finish_clipboard_paste(Err("input failed".into()), 0, || {
            restored.set(true);
        });

        assert_eq!(result.unwrap_err(), "input failed");
        assert!(restored.get());
    }
}
