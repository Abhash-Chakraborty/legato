//! Commands behind the Snippets page.

use crate::managers::dictionary::DictionaryManager;
use crate::managers::dictionary_store;
use crate::managers::snippet_store::{self as store, Snippet, SnippetImport, SnippetInput};
use crate::managers::snippets::SnippetManager;
use crate::snippet_rules::{self as rules, SnippetTest, MAX_TEXT_CHARS, MAX_TRIGGER_CHARS};
use serde::Serialize;
use specta::Type;
use std::sync::Arc;
use tauri::{AppHandle, State};

/// The Dictionary's pass over a dictation, which runs before snippets are
/// matched: a trigger it changes is never heard as typed.
fn dictionary_pass(
    app: &AppHandle,
    dictionary: &DictionaryManager,
) -> Result<impl Fn(&str) -> String, String> {
    let entries = dictionary.list().map_err(|e| e.to_string())?;
    let (_, corrector) = dictionary_store::compile(&entries);
    let corrector = corrector.with_word_check(crate::spelling::checker());
    let threshold = crate::settings::get_settings(app).word_correction_threshold;
    Ok(move |text: &str| corrector.apply(text, false, threshold).0)
}

/// What each trigger becomes after the Dictionary's pass, in order.
#[tauri::command]
#[specta::specta]
pub fn snippet_triggers_heard(
    app: AppHandle,
    dictionary: State<'_, Arc<DictionaryManager>>,
    triggers: Vec<String>,
) -> Result<Vec<String>, String> {
    let pass = dictionary_pass(&app, &dictionary)?;
    Ok(triggers.iter().map(|t| pass(t)).collect())
}

/// Everything the Snippets page shows.
#[derive(Clone, Debug, Serialize, Type)]
pub struct SnippetsView {
    pub snippets: Vec<Snippet>,
    pub max_trigger_chars: u32,
    pub max_text_chars: u32,
    /// The variables Insert variable offers, as written in the text.
    pub variables: Vec<String>,
}

#[tauri::command]
#[specta::specta]
pub fn get_snippets(snippets: State<'_, Arc<SnippetManager>>) -> Result<SnippetsView, String> {
    Ok(SnippetsView {
        snippets: snippets.list().map_err(|e| e.to_string())?,
        max_trigger_chars: MAX_TRIGGER_CHARS as u32,
        max_text_chars: MAX_TEXT_CHARS as u32,
        variables: rules::VARIABLES.iter().map(|v| v.to_string()).collect(),
    })
}

#[tauri::command]
#[specta::specta]
pub fn add_snippet(
    snippets: State<'_, Arc<SnippetManager>>,
    input: SnippetInput,
) -> Result<Snippet, String> {
    snippets.add(&input).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn update_snippet(
    snippets: State<'_, Arc<SnippetManager>>,
    id: i64,
    input: SnippetInput,
) -> Result<Snippet, String> {
    snippets.update(id, &input).map_err(|e| e.to_string())
}

/// Deletes snippets and returns them for Undo.
#[tauri::command]
#[specta::specta]
pub fn delete_snippets(
    snippets: State<'_, Arc<SnippetManager>>,
    ids: Vec<i64>,
) -> Result<Vec<Snippet>, String> {
    snippets.delete(&ids).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn restore_snippets(
    snippets: State<'_, Arc<SnippetManager>>,
    removed: Vec<Snippet>,
) -> Result<(), String> {
    snippets.restore(&removed).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn import_snippets(
    snippets: State<'_, Arc<SnippetManager>>,
    path: String,
) -> Result<SnippetImport, String> {
    let content =
        std::fs::read_to_string(&path).map_err(|e| format!("Couldn't read the file: {}", e))?;
    let rows = store::parse_import(&content).map_err(|e| e.to_string())?;
    if rows.is_empty() {
        return Err("No snippets found in that file".to_string());
    }
    snippets.import(&rows).map_err(|e| e.to_string())
}

/// Saves the given snippets, or all of them, as JSON.
#[tauri::command]
#[specta::specta]
pub fn export_snippets(
    snippets: State<'_, Arc<SnippetManager>>,
    path: String,
    ids: Option<Vec<i64>>,
) -> Result<u32, String> {
    let mut list = snippets.list().map_err(|e| e.to_string())?;
    if let Some(ids) = ids {
        list.retain(|s| ids.contains(&s.id));
    }
    let json = store::export_json(&list).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("Couldn't save the file: {}", e))?;
    Ok(list.len() as u32)
}

/// What a phrase expands to with the saved snippets and the one being edited.
/// Dates and times are filled in; {{clipboard}} and {{selection}} stay, as
/// they only have a value at paste time.
#[tauri::command]
#[specta::specta]
pub fn test_snippets(
    app: AppHandle,
    snippets: State<'_, Arc<SnippetManager>>,
    dictionary: State<'_, Arc<DictionaryManager>>,
    text: String,
    draft: Option<SnippetInput>,
    draft_id: Option<i64>,
) -> Result<SnippetTest, String> {
    let mut list = snippets.list().map_err(|e| e.to_string())?;
    if let Some(draft) = draft {
        let id = draft_id.unwrap_or(-1);
        list.retain(|s| s.id != id);
        list.push(Snippet {
            id,
            trigger: rules::clean_trigger(&draft.trigger),
            also: draft.also.iter().map(|a| rules::clean_trigger(a)).collect(),
            text: draft.text,
            alone: draft.alone,
            uses: 0,
            added_at: 0,
            last_used_at: None,
            last_app: None,
            last_app_path: None,
        });
    }
    let text = dictionary_pass(&app, &dictionary)?(&text);
    let marked = rules::mark(&text, &store::rules_of(&list));
    let vars = rules::Variables {
        now: chrono::Local::now(),
        clipboard: Some("{{clipboard}}".into()),
        selection: Some("{{selection}}".into()),
    };
    let expanded = rules::unmark(&marked.text, &marked.hits, |id| {
        list.iter()
            .find(|s| s.id == id)
            .map(|s| rules::fill(&s.text, &vars))
            .unwrap_or_default()
    });
    Ok(SnippetTest {
        text: expanded,
        matched: marked.hits,
    })
}
