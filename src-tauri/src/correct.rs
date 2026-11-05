//! Correct last dictation: the bar opens what Legato last pasted for editing,
//! puts the corrected text in its place in the app, fixes it on Home, and
//! teaches the Dictionary the words that changed.

use crate::context::AppInfo;
use crate::managers::dictionary::DictionaryManager;
use crate::managers::dictionary_store::{DictionarySource, Learned};
use crate::managers::history::HistoryManager;
use crate::managers::history_store::{AppliedFix, FixSource};
use crate::settings::{get_settings, LearnMode};
use crate::transforms::{self, BarNotice};
use crate::{clipboard, context, learn_rules, overlay, selection};
use log::{error, info, warn};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

/// How long the corrected words' notice stays.
const NOTICE_MS: u64 = 5000;
/// Time for the app to take the focus back before its text is selected.
const REFOCUS_MS: u64 = 150;

/// What Correct is working on while the bar shows it.
struct Session {
    window: isize,
    pasted: String,
    target: Option<AppInfo>,
}

static OPEN: Mutex<Option<Session>> = Mutex::new(None);

fn open() -> std::sync::MutexGuard<'static, Option<Session>> {
    OPEN.lock().unwrap_or_else(|e| e.into_inner())
}

/// What the bar's editor starts with.
#[derive(Serialize, Clone)]
struct CorrectCard {
    text: String,
    app: Option<String>,
}

/// Opens the last paste in the bar, from its Correct button or the Correct
/// last dictation shortcut.
pub fn begin(app: &AppHandle) {
    let Some((window, pasted)) = clipboard::last_paste(transforms::RESELECT_WITHIN) else {
        overlay::show_overlay_notice(app, &BarNotice::NothingToCorrect);
        return;
    };
    // A paste ends with the space or line break Style added; the editor
    // shows the words, and the ending goes back on when it's replaced.
    let target = context::window_info(window);
    let card = CorrectCard {
        text: pasted.trim_end().to_string(),
        app: target.as_ref().map(|t| t.name.clone()),
    };
    *open() = Some(Session {
        window,
        pasted,
        target,
    });
    overlay::show_bar(app, "correct", "bar-correct", &card, None);
    overlay::set_bar_focus(app, true);
}

/// Correct last dictation (Alt+Shift+C): opens the editor once the keys
/// are up, so Alt doesn't open the app's menu.
pub fn shortcut(app: &AppHandle) {
    selection::mask_menu_key(app);
    let app = app.clone();
    std::thread::spawn(move || {
        selection::wait_for_keys_up();
        begin(&app);
    });
}

/// Closes the editor without changing anything. With `refocus` the app
/// gets its focus back; without, you clicked somewhere else.
pub fn cancel(app: &AppHandle, refocus: bool) {
    // Already closed, by Fix or by the bar moving on.
    let Some(session) = open().take() else {
        return;
    };
    overlay::set_bar_focus(app, false);
    overlay::hide_recording_overlay(app);
    if refocus {
        context::focus_window(session.window);
    }
}

/// Puts `corrected` in place of the paste and learns from it.
pub fn apply(app: &AppHandle, corrected: String) {
    let Some(session) = open().take() else {
        return;
    };
    overlay::set_bar_focus(app, false);
    overlay::hide_recording_overlay(app);
    let ending = &session.pasted[session.pasted.trim_end().len()..];
    let corrected = format!("{}{ending}", corrected.trim_end());
    if corrected.trim() == session.pasted.trim() {
        context::focus_window(session.window);
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        context::focus_window(session.window);
        std::thread::sleep(Duration::from_millis(REFOCUS_MS));
        let replaced = transforms::reselect_last_paste(&app, session.target.as_ref()).is_some()
            && match transforms::paste_on_main(&app, corrected.clone()) {
                Ok(()) => true,
                Err(e) => {
                    error!("Couldn't paste the correction: {e}");
                    false
                }
            };
        if !replaced {
            if let Err(e) = app.clipboard().write_text(corrected.clone()) {
                error!("Couldn't put the correction on the clipboard: {e}");
            }
        }
        let edits = learn_rules::find_edits(
            &session.pasted,
            &corrected,
            get_settings(&app).word_correction_threshold,
        );
        fix_history(&app, &session.pasted, &corrected, &edits);
        let review = get_settings(&app).learn_mode == LearnMode::Ask;
        let words = if review {
            ask(&app, &edits, session.target.as_ref())
        } else {
            let learned = learn(&app, &edits, session.target.as_ref());
            let words = learned.iter().map(|l| l.entry.text.clone()).collect();
            if !learned.is_empty() {
                transforms::offer_unlearn(learned);
            }
            words
        };
        info!(
            "Corrected the last dictation ({}, {} word(s) {})",
            if replaced { "replaced" } else { "copied" },
            words.len(),
            if review { "for review" } else { "learned" }
        );
        let word = words.first().cloned();
        let more = words.len().saturating_sub(1);
        let notice = if replaced {
            BarNotice::Corrected { word, more, review }
        } else {
            BarNotice::CorrectionCopied { word, more, review }
        };
        overlay::show_overlay_notice_for(&app, &notice, NOTICE_MS);
    });
}

/// Fixes the latest dictation on Home when it is the one that was pasted.
fn fix_history(app: &AppHandle, pasted: &str, corrected: &str, edits: &[learn_rules::Edit]) {
    let Some(history) = app.try_state::<Arc<HistoryManager>>() else {
        return;
    };
    let Ok(Some(entry)) = history.get_latest_completed_entry() else {
        return;
    };
    let shown = entry
        .post_processed_text
        .as_deref()
        .unwrap_or(&entry.transcription_text);
    if shown.trim() != pasted.trim() {
        warn!("The latest dictation isn't the one pasted; Home keeps it as it was");
        return;
    }
    let fixes: Vec<AppliedFix> = edits
        .iter()
        .map(|e| AppliedFix {
            from: e.heard.clone(),
            to: e.word.clone(),
            source: FixSource::Manual,
        })
        .collect();
    let shown = shown.to_string();
    if let Err(e) = history.correct_row(entry.id, &shown, corrected.trim(), &fixes) {
        error!("Couldn't correct the dictation on Home: {e}");
    }
}

/// Puts the corrected words under Needs review, as Learn from my edits does
/// when it asks first.
fn ask(app: &AppHandle, edits: &[learn_rules::Edit], target: Option<&AppInfo>) -> Vec<String> {
    let Some(dictionary) = app.try_state::<Arc<DictionaryManager>>() else {
        return Vec::new();
    };
    let fix_app = target.map(|t| (t.name.as_str(), Some(t.exe_path.as_str())));
    let words: Vec<(String, String, String)> = edits
        .iter()
        .map(|e| (e.word.clone(), e.heard.clone(), e.context.clone()))
        .collect();
    match dictionary.add_pending(&words, fix_app) {
        Ok(added) => added.into_iter().map(|p| p.word).collect(),
        Err(e) => {
            error!("Couldn't save corrected words for review: {e}");
            Vec::new()
        }
    }
}

/// Adds the corrected words to the Dictionary, as Fix a word does.
fn learn(app: &AppHandle, edits: &[learn_rules::Edit], target: Option<&AppInfo>) -> Vec<Learned> {
    let Some(dictionary) = app.try_state::<Arc<DictionaryManager>>() else {
        return Vec::new();
    };
    let fix_app = target.map(|t| (t.name.as_str(), Some(t.exe_path.as_str())));
    let mut learned = Vec::new();
    for edit in edits {
        if dictionary.is_known(&edit.word, &edit.heard).unwrap_or(true) {
            continue;
        }
        match dictionary.learn(
            &edit.word,
            Some(&edit.heard),
            DictionarySource::Fixed,
            true,
            fix_app,
        ) {
            Ok(l) => learned.push(l),
            Err(e) => error!("Couldn't learn a corrected word: {e}"),
        }
    }
    learned
}
