//! Commands behind the Dictionary page and Fix a word on Home.

use crate::learn_rules::Spelling;
use crate::managers::dictionary::{applied_fixes, DictionaryManager, FixUndo};
use crate::managers::dictionary_store::{
    self as store, DictionaryEntry, DictionarySource, EntryInput, ImportSummary, PendingWord,
    HINT_LIMIT, MAX_WORD_CHARS, PIN_LIMIT,
};
use crate::managers::history::HistoryManager;
use crate::managers::history_store::{AppliedFix, FixSource};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use tauri::{AppHandle, State};

/// Everything the Dictionary page shows.
#[derive(Clone, Debug, Serialize, Type)]
pub struct DictionaryView {
    pub entries: Vec<DictionaryEntry>,
    /// What the speech model gets before it listens, in order.
    pub hint: Vec<String>,
    pub hint_limit: u32,
    pub pin_limit: u32,
    pub max_word_chars: u32,
    /// Words learned from edits, waiting under Needs review.
    pub pending: Vec<PendingWord>,
    /// Words Learn from my edits never proposes.
    pub never: Vec<String>,
    /// Terms the model wrote more than one way lately, also under Needs
    /// review.
    pub spellings: Vec<Spelling>,
}

#[tauri::command]
#[specta::specta]
pub fn get_dictionary(
    dictionary: State<'_, Arc<DictionaryManager>>,
) -> Result<DictionaryView, String> {
    let entries = dictionary.list().map_err(|e| e.to_string())?;
    Ok(DictionaryView {
        hint: store::hint(&entries),
        entries,
        hint_limit: HINT_LIMIT as u32,
        pin_limit: PIN_LIMIT as u32,
        max_word_chars: MAX_WORD_CHARS as u32,
        pending: dictionary.pending().map_err(|e| e.to_string())?,
        never: dictionary.never().map_err(|e| e.to_string())?,
        spellings: dictionary.spellings().map_err(|e| e.to_string())?,
    })
}

#[tauri::command]
#[specta::specta]
pub fn add_dictionary_entry(
    dictionary: State<'_, Arc<DictionaryManager>>,
    input: EntryInput,
) -> Result<DictionaryEntry, String> {
    dictionary
        .add(&input, DictionarySource::Manual)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn update_dictionary_entry(
    dictionary: State<'_, Arc<DictionaryManager>>,
    id: i64,
    input: EntryInput,
) -> Result<DictionaryEntry, String> {
    dictionary.update(id, &input).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn set_dictionary_entry_pinned(
    dictionary: State<'_, Arc<DictionaryManager>>,
    id: i64,
    pinned: bool,
) -> Result<DictionaryEntry, String> {
    dictionary.set_pinned(id, pinned).map_err(|e| e.to_string())
}

/// Deletes words and returns them, for Undo.
#[tauri::command]
#[specta::specta]
pub fn delete_dictionary_entries(
    dictionary: State<'_, Arc<DictionaryManager>>,
    ids: Vec<i64>,
) -> Result<Vec<DictionaryEntry>, String> {
    dictionary.delete(&ids).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn restore_dictionary_entries(
    dictionary: State<'_, Arc<DictionaryManager>>,
    entries: Vec<DictionaryEntry>,
) -> Result<(), String> {
    dictionary.restore(&entries).map_err(|e| e.to_string())
}

/// A sentence after the Dictionary corrected it.
#[derive(Clone, Debug, Serialize, Type)]
pub struct DictionaryTest {
    pub text: String,
    pub fixes: Vec<AppliedFix>,
}

/// Runs the same correction the pipeline uses on a typed sentence. `draft` is
/// a word being edited (replacing the entry `draft_id`, or added), so the
/// editor can test before saving.
#[tauri::command]
#[specta::specta]
pub fn test_dictionary(
    app: AppHandle,
    dictionary: State<'_, Arc<DictionaryManager>>,
    text: String,
    draft: Option<EntryInput>,
    draft_id: Option<i64>,
) -> Result<DictionaryTest, String> {
    let mut entries = dictionary.list().map_err(|e| e.to_string())?;
    if let Some(draft) = draft.filter(|d| !store::clean_word(&d.text).is_empty()) {
        let word = DictionaryEntry {
            id: draft_id.unwrap_or(-1),
            text: store::clean_word(&draft.text),
            heard: draft.heard.iter().map(|h| store::clean_heard(h)).collect(),
            source: DictionarySource::Manual,
            pinned: draft.pinned,
            case_sensitive: draft.case_sensitive,
            added_at: chrono::Utc::now().timestamp(),
            last_fix_at: None,
            last_app: None,
            last_app_path: None,
            hits: 0,
            kind: draft.kind,
        };
        entries.retain(|e| {
            Some(e.id) != draft_id && e.text.to_lowercase() != word.text.to_lowercase()
        });
        entries.push(word);
    }
    let (_, corrector) = store::compile(&entries);
    let threshold = crate::settings::get_settings(&app).word_correction_threshold;
    // As if no word went to the model, so every word is matched.
    let (text, fixes) = corrector.apply(&text, false, threshold);
    Ok(DictionaryTest {
        text,
        fixes: applied_fixes(&fixes),
    })
}

#[tauri::command]
#[specta::specta]
pub fn import_dictionary(
    dictionary: State<'_, Arc<DictionaryManager>>,
    path: String,
) -> Result<ImportSummary, String> {
    let content =
        std::fs::read_to_string(&path).map_err(|e| format!("Couldn't read the file: {}", e))?;
    let rows = store::parse_import(&content).map_err(|e| e.to_string())?;
    if rows.is_empty() {
        return Err("No words found in that file".to_string());
    }
    dictionary.import(&rows).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn export_dictionary(
    dictionary: State<'_, Arc<DictionaryManager>>,
    path: String,
) -> Result<u32, String> {
    let entries = dictionary.list().map_err(|e| e.to_string())?;
    let json = store::export_json(&entries).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("Couldn't save the file: {}", e))?;
    Ok(entries.len() as u32)
}

/// Other kept transcripts the same miss could be fixed in.
#[tauri::command]
#[specta::specta]
pub fn find_fix_candidates(
    history: State<'_, Arc<HistoryManager>>,
    from: String,
    to: String,
    except: i64,
) -> Result<Vec<i64>, String> {
    history
        .rows_with(&from, &to, except)
        .map_err(|e| e.to_string())
}

/// Fix a word: what was heard, what it should be, and where.
#[derive(Clone, Debug, Deserialize, Type)]
pub struct FixWordRequest {
    pub history_id: i64,
    /// The transcript's text as Home showed it.
    pub shown_before: String,
    /// The same text with the fix in place.
    pub shown_after: String,
    pub from: String,
    pub to: String,
    /// Add the word to the Dictionary (Fix and learn).
    pub learn: bool,
    /// Keep `from` as a way the model mishears the word.
    pub remember_heard: bool,
    /// Other transcripts to fix the same way.
    pub also: Vec<i64>,
}

#[derive(Clone, Debug, Serialize, Type)]
pub struct FixWordResult {
    /// Pass to `undo_fix_word`.
    pub undo_token: u32,
    pub entry: Option<DictionaryEntry>,
    pub created: bool,
    pub extended: bool,
    /// Other transcripts that were fixed.
    pub others: u32,
}

#[tauri::command]
#[specta::specta]
pub fn fix_word(
    history: State<'_, Arc<HistoryManager>>,
    dictionary: State<'_, Arc<DictionaryManager>>,
    request: FixWordRequest,
) -> Result<FixWordResult, String> {
    let from = request.from.trim().to_string();
    let to = store::clean_word(&request.to);
    if to.is_empty() || to == from {
        return Err("Type how the word should be spelled".to_string());
    }
    if request.learn && to.chars().count() > MAX_WORD_CHARS {
        return Err(format!(
            "Dictionary words can be up to {} characters",
            MAX_WORD_CHARS
        ));
    }
    let fix = AppliedFix {
        from: from.clone(),
        to: to.clone(),
        source: FixSource::Manual,
    };
    let (snapshot, entry) = history
        .fix_row(
            request.history_id,
            &request.shown_before,
            &request.shown_after,
            &fix,
        )
        .map_err(|e| e.to_string())?;
    let mut rows = vec![snapshot];
    let others = if request.also.is_empty() {
        Vec::new()
    } else {
        history
            .fix_rows(&request.also, &fix)
            .map_err(|e| e.to_string())?
    };
    let others_fixed = others.len() as u32;
    rows.extend(others);

    let learned = if request.learn {
        let app = entry
            .app_name
            .as_deref()
            .map(|name| (name, entry.app_path.as_deref()));
        let heard = request.remember_heard.then_some(from.as_str());
        match dictionary.learn(&to, heard, DictionarySource::Fixed, true, app) {
            Ok(learned) => Some(learned),
            Err(e) => {
                // Leave nothing half done.
                let _ = history.restore_texts(&rows);
                return Err(e.to_string());
            }
        }
    } else {
        None
    };

    let result = FixWordResult {
        undo_token: 0,
        entry: learned.as_ref().map(|l| l.entry.clone()),
        created: learned.as_ref().is_some_and(|l| l.created),
        extended: learned.as_ref().is_some_and(|l| l.extended),
        others: others_fixed,
    };
    let undo_token = dictionary.stash_undo(FixUndo {
        rows,
        learned: learned.into_iter().collect(),
        pending: Vec::new(),
    });
    Ok(FixWordResult {
        undo_token,
        ..result
    })
}

#[tauri::command]
#[specta::specta]
pub fn undo_fix_word(
    history: State<'_, Arc<HistoryManager>>,
    dictionary: State<'_, Arc<DictionaryManager>>,
    undo_token: u32,
) -> Result<(), String> {
    let undo = dictionary
        .take_undo(undo_token)
        .ok_or("That fix can no longer be undone")?;
    history
        .restore_texts(&undo.rows)
        .map_err(|e| e.to_string())?;
    undo_learned(&dictionary, &undo)
}

fn undo_learned(dictionary: &DictionaryManager, undo: &FixUndo) -> Result<(), String> {
    for learned in undo.learned.iter().rev() {
        dictionary.unlearn(learned).map_err(|e| e.to_string())?;
    }
    if !undo.pending.is_empty() {
        dictionary
            .restore_pending(&undo.pending)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Adds words from Needs review to the Dictionary, each with the way it was
/// pasted as a heard form. Returns a token for Undo.
#[tauri::command]
#[specta::specta]
pub fn accept_pending_words(
    dictionary: State<'_, Arc<DictionaryManager>>,
    ids: Vec<i64>,
) -> Result<u32, String> {
    let pending = dictionary.remove_pending(&ids).map_err(|e| e.to_string())?;
    let mut learned = Vec::new();
    for p in &pending {
        let app = p.app.as_deref().map(|name| (name, p.app_path.as_deref()));
        match dictionary.learn(
            &p.word,
            Some(&p.heard),
            DictionarySource::Edited,
            false,
            app,
        ) {
            Ok(l) => learned.push(l),
            Err(e) => {
                // Leave nothing half done.
                let undo = FixUndo {
                    rows: Vec::new(),
                    learned,
                    pending,
                };
                let _ = undo_learned(&dictionary, &undo);
                return Err(e.to_string());
            }
        }
    }
    Ok(dictionary.stash_undo(FixUndo {
        rows: Vec::new(),
        learned,
        pending,
    }))
}

#[tauri::command]
#[specta::specta]
pub fn undo_accept_pending_words(
    dictionary: State<'_, Arc<DictionaryManager>>,
    undo_token: u32,
) -> Result<(), String> {
    let undo = dictionary
        .take_undo(undo_token)
        .ok_or("Those words can no longer be undone")?;
    undo_learned(&dictionary, &undo)
}

/// Takes words off Needs review without adding them; returns them for Undo.
#[tauri::command]
#[specta::specta]
pub fn dismiss_pending_words(
    dictionary: State<'_, Arc<DictionaryManager>>,
    ids: Vec<i64>,
) -> Result<Vec<PendingWord>, String> {
    dictionary.remove_pending(&ids).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn restore_pending_words(
    dictionary: State<'_, Arc<DictionaryManager>>,
    words: Vec<PendingWord>,
) -> Result<(), String> {
    dictionary
        .restore_pending(&words)
        .map_err(|e| e.to_string())
}

/// Adds a term the model wrote more than one way, with its other forms as
/// ways it's heard. Undo deletes the entry.
#[tauri::command]
#[specta::specta]
pub fn add_spelling(
    dictionary: State<'_, Arc<DictionaryManager>>,
    word: String,
    forms: Vec<String>,
) -> Result<DictionaryEntry, String> {
    dictionary
        .add_spelling(&word, &forms)
        .map_err(|e| e.to_string())
}

/// Stops Learn from my edits proposing a word.
#[tauri::command]
#[specta::specta]
pub fn add_never_learn_word(
    dictionary: State<'_, Arc<DictionaryManager>>,
    word: String,
) -> Result<(), String> {
    dictionary.add_never(&word).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn remove_never_learn_word(
    dictionary: State<'_, Arc<DictionaryManager>>,
    word: String,
) -> Result<(), String> {
    dictionary.remove_never(&word).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn change_learn_from_edits_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = crate::settings::get_settings(&app);
    settings.learn_from_edits = enabled;
    crate::settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_learn_mode_setting(
    app: AppHandle,
    mode: crate::settings::LearnMode,
) -> Result<(), String> {
    let mut settings = crate::settings::get_settings(&app);
    settings.learn_mode = mode;
    crate::settings::write_settings(&app, settings);
    Ok(())
}
