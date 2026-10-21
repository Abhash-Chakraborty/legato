//! Add selection to Dictionary (Win+Alt+W by default): copies what is
//! selected in the app in front, puts the clipboard back as it was, and adds
//! the text as a Dictionary word, spelled exactly as selected.

use crate::input::EnigoState;
use crate::managers::audio::AudioRecordingManager;
use crate::managers::dictionary::DictionaryManager;
use crate::managers::dictionary_store::{clean_word, DictionarySource, MAX_WORD_CHARS};
use enigo::{Direction, Enigo, Key, Keyboard};
use log::{error, info};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

static BUSY: AtomicBool = AtomicBool::new(false);

/// A selection with more words than this isn't a word to learn.
const MAX_WORDS: usize = 4;

/// How long to wait for the app to put the selection on the clipboard.
const COPY_TIMEOUT: Duration = Duration::from_millis(600);

/// Unassigned virtual key (AutoHotkey's menu mask). Pressed while Win or Alt
/// is still down, so their release doesn't open Start or a menu bar.
const MENU_MASK_VK: u32 = 0xE8;

/// What the overlay says afterwards; it words each kind itself.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum SelectionNotice {
    WordAdded { word: String },
    WordExists { word: String },
    NoSelection,
    NotAWord,
    Failed,
}

/// Runs on the shortcut's press. The work happens on its own thread once the
/// shortcut's keys are up.
pub fn add_selection_to_dictionary(app: &AppHandle) {
    if BUSY.swap(true, Ordering::SeqCst) {
        return;
    }
    mask_menu_key(app);
    let app = app.clone();
    std::thread::spawn(move || {
        let notice = add(&app).unwrap_or_else(|e| {
            error!("Add selection to Dictionary failed: {}", e);
            SelectionNotice::Failed
        });
        // Never cover a dictation that started meanwhile.
        let recording = app
            .try_state::<Arc<AudioRecordingManager>>()
            .is_some_and(|rm| rm.is_recording());
        if !recording {
            crate::overlay::show_overlay_notice(&app, &notice);
        }
        BUSY.store(false, Ordering::SeqCst);
    });
}

fn add(app: &AppHandle) -> Result<SelectionNotice, String> {
    wait_for_keys_up();
    let Some(selected) = copy_selection(app)? else {
        return Ok(SelectionNotice::NoSelection);
    };
    let Some(word) = word_from_selection(&selected) else {
        return Ok(SelectionNotice::NotAWord);
    };
    let dictionary = app
        .try_state::<Arc<DictionaryManager>>()
        .ok_or("The Dictionary isn't ready")?;
    let learned = dictionary
        .learn(&word, None, DictionarySource::Selection, false, None)
        .map_err(|e| e.to_string())?;
    info!("Added a selected word to the Dictionary");
    Ok(if learned.created {
        SelectionNotice::WordAdded {
            word: learned.entry.text,
        }
    } else {
        SelectionNotice::WordExists {
            word: learned.entry.text,
        }
    })
}

/// The selection as a word: outer spaces and sentence punctuation trimmed,
/// everything else kept as selected. None if it's empty or reads as more
/// than a word or a name.
pub fn word_from_selection(selected: &str) -> Option<String> {
    if selected.trim().contains('\n') {
        return None;
    }
    let trimmed = selected.trim_matches(|c: char| {
        c.is_whitespace()
            || matches!(
                c,
                '.' | ','
                    | ';'
                    | ':'
                    | '!'
                    | '?'
                    | '"'
                    | '\''
                    | '('
                    | ')'
                    | '['
                    | ']'
                    | '{'
                    | '}'
                    | '<'
                    | '>'
                    | '\u{201c}'
                    | '\u{201d}'
                    | '\u{2018}'
                    | '\u{2019}'
                    | '\u{2026}'
            )
    });
    let word = clean_word(trimmed);
    if word.is_empty()
        || word.chars().count() > MAX_WORD_CHARS
        || word.split(' ').count() > MAX_WORDS
    {
        return None;
    }
    Some(word)
}

/// Taps an unassigned key while the shortcut's Win or Alt is still down, so
/// letting go of them doesn't open Start or focus a menu bar.
pub(crate) fn mask_menu_key(app: &AppHandle) {
    let _ = with_enigo(app, |enigo| {
        enigo
            .key(Key::Other(MENU_MASK_VK), Direction::Click)
            .map_err(|e| e.to_string())
    });
}

/// Sends Ctrl plus a virtual key to the app in front.
pub(crate) fn send_ctrl(app: &AppHandle, vk: u32) -> Result<(), String> {
    with_enigo(app, |enigo| {
        enigo
            .key(Key::Control, Direction::Press)
            .map_err(|e| format!("Failed to press Ctrl: {}", e))?;
        let click = enigo
            .key(Key::Other(vk), Direction::Click)
            .map_err(|e| format!("Failed to press the key: {}", e));
        enigo
            .key(Key::Control, Direction::Release)
            .map_err(|e| format!("Failed to release Ctrl: {}", e))?;
        click
    })
}

/// Selects `count` characters before the cursor with Shift+Left.
pub(crate) fn select_back(app: &AppHandle, count: usize) -> Result<(), String> {
    with_enigo(app, |enigo| {
        enigo
            .key(Key::Shift, Direction::Press)
            .map_err(|e| format!("Failed to press Shift: {}", e))?;
        let mut moved = Ok(());
        for _ in 0..count {
            if let Err(e) = enigo.key(Key::LeftArrow, Direction::Click) {
                moved = Err(format!("Failed to press Left: {}", e));
                break;
            }
        }
        enigo
            .key(Key::Shift, Direction::Release)
            .map_err(|e| format!("Failed to release Shift: {}", e))?;
        moved
    })
}

/// Presses one virtual key in the app in front.
pub(crate) fn send_key(app: &AppHandle, vk: u32) -> Result<(), String> {
    with_enigo(app, |enigo| {
        enigo
            .key(Key::Other(vk), Direction::Click)
            .map_err(|e| format!("Failed to press the key: {}", e))
    })
}

fn with_enigo<T>(
    app: &AppHandle,
    f: impl FnOnce(&mut Enigo) -> Result<T, String>,
) -> Result<T, String> {
    let state = app
        .try_state::<EnigoState>()
        .ok_or("Keyboard input isn't ready yet")?;
    let mut enigo = state
        .0
        .lock()
        .map_err(|e| format!("Failed to lock Enigo: {}", e))?;
    f(&mut enigo)
}

/// Waits until Win, Alt, Ctrl and Shift are up, so the Ctrl+C below isn't
/// read as Win+Alt+Ctrl+C.
#[cfg(windows)]
pub(crate) fn wait_for_keys_up() {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline && modifiers_down() {
        std::thread::sleep(Duration::from_millis(15));
    }
    std::thread::sleep(Duration::from_millis(40));
}

/// Whether Win, Alt, Ctrl or Shift is held.
#[cfg(windows)]
pub(crate) fn modifiers_down() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
    };
    [VK_LWIN, VK_RWIN, VK_MENU, VK_CONTROL, VK_SHIFT]
        .iter()
        .any(|key| unsafe { GetAsyncKeyState(i32::from(key.0)) } as u16 & 0x8000 != 0)
}

#[cfg(not(windows))]
pub(crate) fn modifiers_down() -> bool {
    false
}

#[cfg(not(windows))]
pub(crate) fn wait_for_keys_up() {
    std::thread::sleep(Duration::from_millis(200));
}

/// Sends Ctrl+C and reads what the app copied, then puts back whatever the
/// clipboard held before.
pub(crate) fn copy_selection(app: &AppHandle) -> Result<Option<String>, String> {
    let clipboard = app.clipboard();
    let saved_text = clipboard.read_text().ok().filter(|t| !t.is_empty());
    let saved_image = if saved_text.is_none() {
        clipboard.read_image().ok().map(|image| image.to_owned())
    } else {
        None
    };
    // Empty first, so an app with nothing selected can't hand back the old text.
    let _ = clipboard.clear();

    let sent = send_ctrl(app, 0x43); // VK_C

    let mut copied = None;
    if sent.is_ok() {
        let deadline = Instant::now() + COPY_TIMEOUT;
        while Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(30));
            if let Ok(text) = clipboard.read_text() {
                if !text.is_empty() {
                    copied = Some(text);
                    break;
                }
            }
        }
    }

    if let Some(text) = saved_text {
        let _ = clipboard.write_text(text);
    } else if let Some(image) = saved_image {
        let _ = clipboard.write_image(&image);
    } else {
        let _ = clipboard.clear();
    }
    sent?;
    Ok(copied)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selections_become_words() {
        assert_eq!(
            word_from_selection(" Dokploy, ").as_deref(),
            Some("Dokploy")
        );
        assert_eq!(
            word_from_selection("\u{201c}Netbird\u{201d}").as_deref(),
            Some("Netbird")
        );
        assert_eq!(word_from_selection("C++").as_deref(), Some("C++"));
        assert_eq!(
            word_from_selection("Wispr  Flow").as_deref(),
            Some("Wispr Flow")
        );
        assert_eq!(word_from_selection("..."), None);
        assert_eq!(word_from_selection("one two three four five"), None);
        assert_eq!(word_from_selection("line one\nline two"), None);
    }
}
