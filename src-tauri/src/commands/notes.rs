//! Commands behind the Notes page and its recorder.

use crate::loopback::{self, AudioApp};
use crate::managers::dictionary_store::WordKind;
use crate::managers::notes_store::Speaker;
use crate::managers::notes_store::{DeletedRecording, Note, NoteLine, NoteMatch, Voice};
use crate::notes::{
    self, LineEdit, NotesManager, SessionInfo, StartNotes, SuggestedFix, WordFix, WordFixed,
};
use crate::settings::{get_settings, write_settings};
use crate::speakers::{self, SpeakerModel};
use std::sync::Arc;
use tauri::{AppHandle, State};

type Notes<'a> = State<'a, Arc<NotesManager>>;

fn text<T>(result: anyhow::Result<T>) -> Result<T, String> {
    result.map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn get_notes(notes: Notes<'_>) -> Result<Vec<Note>, String> {
    text(notes.list())
}

#[tauri::command]
#[specta::specta]
pub fn get_note_lines(notes: Notes<'_>, id: i64) -> Result<Vec<NoteLine>, String> {
    text(notes.lines(id))
}

#[tauri::command]
#[specta::specta]
pub fn search_notes(notes: Notes<'_>, query: String) -> Result<Vec<NoteMatch>, String> {
    text(notes.search(&query))
}

#[tauri::command]
#[specta::specta]
pub fn get_note_tags(notes: Notes<'_>) -> Result<Vec<String>, String> {
    text(notes.tags())
}

#[tauri::command]
#[specta::specta]
pub fn rename_note(notes: Notes<'_>, id: i64, title: String) -> Result<(), String> {
    text(notes.rename(id, &title))
}

/// Corrects a line's words, or who said it, during or after the recording.
#[tauri::command]
#[specta::specta]
pub fn edit_note_line(
    notes: Notes<'_>,
    id: i64,
    who: Speaker,
    speaker: Option<i64>,
    text: String,
) -> Result<LineEdit, String> {
    self::text(notes.edit_line(id, who, speaker, &text))
}

/// Fixes a corrected word in the note's other lines, and adds it to the
/// Dictionary when `learn` is on.
#[tauri::command]
#[specta::specta]
pub fn fix_note_word(notes: Notes<'_>, fix: WordFix, learn: bool) -> Result<WordFixed, String> {
    text(notes.fix_word(&fix, learn))
}

/// Adds words selected in a transcript to the Dictionary and writes them
/// that way in every line of the note.
#[tauri::command]
#[specta::specta]
pub fn add_note_word(
    notes: Notes<'_>,
    id: i64,
    heard: String,
    word: String,
    kind: Option<WordKind>,
) -> Result<WordFixed, String> {
    text(notes.add_word(id, &heard, &word, kind))
}

#[tauri::command]
#[specta::specta]
pub fn undo_note_word_fix(notes: Notes<'_>, undo_token: u32) -> Result<(), String> {
    text(notes.undo_fix_word(undo_token))
}

#[tauri::command]
#[specta::specta]
pub fn delete_note_line(notes: Notes<'_>, id: i64) -> Result<NoteLine, String> {
    text(notes.delete_line(id))
}

#[tauri::command]
#[specta::specta]
pub fn restore_note_line(notes: Notes<'_>, line: NoteLine) -> Result<(), String> {
    text(notes.restore_line(&line))
}

#[tauri::command]
#[specta::specta]
pub fn save_note_my_notes(notes: Notes<'_>, id: i64, text: String) -> Result<(), String> {
    self::text(notes.set_my_notes(id, &text))
}

#[tauri::command]
#[specta::specta]
pub fn add_note_tag(notes: Notes<'_>, id: i64, tag: String) -> Result<(), String> {
    text(notes.add_tag(id, &tag))
}

#[tauri::command]
#[specta::specta]
pub fn remove_note_tag(notes: Notes<'_>, id: i64, tag: String) -> Result<(), String> {
    text(notes.remove_tag(id, &tag))
}

#[tauri::command]
#[specta::specta]
pub fn set_note_action_done(notes: Notes<'_>, action_id: i64, done: bool) -> Result<(), String> {
    text(notes.set_action_done(action_id, done))
}

/// The bar's Keep or Discard for a note that looked started by mistake.
#[tauri::command]
#[specta::specta]
pub fn answer_short_note(app: AppHandle, id: i64, keep: bool) -> Result<(), String> {
    text(notes::answer_unsure(&app, id, keep))
}

/// The bar's Keep recording, shortly before a recording's length limit.
#[tauri::command]
#[specta::specta]
pub fn keep_notes_recording() -> Result<(), String> {
    text(notes::keep_recording())
}

/// Deletes a note and returns it for Undo.
#[tauri::command]
#[specta::specta]
pub fn delete_note(notes: Notes<'_>, id: i64) -> Result<Option<DeletedRecording>, String> {
    text(notes.delete(id))
}

#[tauri::command]
#[specta::specta]
pub fn restore_note(notes: Notes<'_>, deleted: DeletedRecording) -> Result<(), String> {
    text(notes.restore(&deleted))
}

/// The note's audio file, for the player.
#[tauri::command]
#[specta::specta]
pub fn get_note_audio(notes: Notes<'_>, id: i64) -> Result<Option<String>, String> {
    text(notes.audio_file(id)).map(|p| p.map(|p| p.to_string_lossy().into_owned()))
}

/// The note as Markdown, for Copy.
#[tauri::command]
#[specta::specta]
pub fn get_note_markdown(notes: Notes<'_>, id: i64) -> Result<String, String> {
    text(notes.markdown(id))
}

/// Puts the note on the clipboard formatted, so pasting into Google Docs or
/// Word keeps its headings, lists and bold titles.
#[tauri::command]
#[specta::specta]
pub fn copy_note_formatted(app: AppHandle, notes: Notes<'_>, id: i64) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    let (html, markdown) = text(notes.formatted(id))?;
    app.clipboard()
        .write_html(html, Some(markdown))
        .map_err(|e| e.to_string())
}

/// Saves the note where the save dialog said: a Word file for a .docx
/// name, Markdown otherwise.
#[tauri::command]
#[specta::specta]
pub fn export_note(notes: Notes<'_>, id: i64, path: String) -> Result<(), String> {
    let word = std::path::Path::new(&path)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("docx"));
    let bytes = if word {
        text(notes.docx(id))?
    } else {
        text(notes.markdown(id))?.into_bytes()
    };
    std::fs::write(&path, bytes).map_err(|e| e.to_string())
}

/// Apps a meeting can record, and the output device "Everything" records.
#[tauri::command]
#[specta::specta]
pub fn get_notes_audio_sources() -> (Vec<AudioApp>, Option<String>) {
    (loopback::audio_apps(), loopback::default_output_name())
}

#[tauri::command]
#[specta::specta]
pub fn start_notes(app: AppHandle, options: StartNotes) -> Result<Note, String> {
    text(notes::start(&app, options))
}

#[tauri::command]
#[specta::specta]
pub fn stop_notes(app: AppHandle) -> Result<(), String> {
    text(notes::stop(&app))
}

#[tauri::command]
#[specta::specta]
pub fn pause_notes(app: AppHandle, paused: bool) -> Result<(), String> {
    text(notes::set_paused(&app, paused))
}

#[tauri::command]
#[specta::specta]
pub fn get_notes_session() -> Option<SessionInfo> {
    notes::session()
}

/// Writes the summary again as the kind of conversation you picked.
#[tauri::command]
#[specta::specta]
pub async fn summarise_note_as(app: AppHandle, id: i64, kind: String) -> Result<(), String> {
    notes::resummarise_as(&app, id, &kind).await
}

/// Writes the note's summary again. Errors are codes the page words.
#[tauri::command]
#[specta::specta]
pub async fn summarise_note(app: AppHandle, id: i64) -> Result<(), String> {
    notes::resummarise(&app, id).await
}

/// Tries a note's failed lines again from its audio. Each line waits for
/// the speech model, so it runs off the main thread.
#[tauri::command]
#[specta::specta]
pub async fn retry_note_lines(app: AppHandle, id: i64) -> Result<notes::RetryResult, String> {
    tauri::async_runtime::spawn_blocking(move || notes::retry_failed_lines(&app, id))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// Ask about this note. Errors are codes the page words.
#[tauri::command]
#[specta::specta]
pub async fn ask_note(app: AppHandle, id: i64, question: String) -> Result<String, String> {
    notes::ask(&app, id, &question).await
}

/// Ask across notes, from the Notes page. Errors are codes, as for Ask;
/// "nosummaries" when no note has a summary yet.
#[tauri::command]
#[specta::specta]
pub async fn ask_all_notes(app: AppHandle, question: String) -> Result<String, String> {
    notes::ask_all(&app, &question).await
}

/// What did I miss? in the Notes side panel: the last few minutes of the
/// recording, recapped. Errors are codes, as for Ask.
#[tauri::command]
#[specta::specta]
pub async fn notes_what_did_i_miss(app: AppHandle, id: i64) -> Result<String, String> {
    notes::what_did_i_miss(&app, id).await
}

/// Misheard words your AI found in the note, for review. Errors are codes
/// the page words.
#[tauri::command]
#[specta::specta]
pub async fn suggest_note_fixes(app: AppHandle, id: i64) -> Result<Vec<SuggestedFix>, String> {
    notes::suggest_fixes(&app, id).await
}

/// Applies the fixes that were accepted; returns a token for Undo.
#[tauri::command]
#[specta::specta]
pub fn apply_note_fixes(
    notes: Notes<'_>,
    id: i64,
    fixes: Vec<SuggestedFix>,
) -> Result<WordFixed, String> {
    text(notes.apply_fixes(id, &fixes))
}

/// Drafts a follow-up email from the summary and opens it in the
/// Scratchpad. Errors are codes the page words.
#[tauri::command]
#[specta::specta]
pub async fn draft_note_follow_up(app: AppHandle, id: i64) -> Result<(), String> {
    notes::follow_up(&app, id).await
}

/// Dictate here in My notes: starts a dictation into the note, or stops it.
#[tauri::command]
#[specta::specta]
pub fn notes_dictate(app: AppHandle, note_id: i64) {
    crate::scratchpad::dictate_into_notes(&app, note_id);
}

/// The bar's Open after a note is ready.
#[tauri::command]
#[specta::specta]
pub fn open_note(app: AppHandle, id: i64) {
    crate::open_page(&app, "notes", Some(&id.to_string()));
}

#[tauri::command]
#[specta::specta]
pub fn change_notes_live_transcript_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.notes_live_transcript = enabled;
    write_settings(&app, settings);
    Ok(())
}

/// Tells the voices apart in a finished note from its audio. Takes a while
/// for a long meeting. Returns how many voices were found.
#[tauri::command]
#[specta::specta]
pub async fn find_note_speakers(app: AppHandle, id: i64) -> Result<u32, String> {
    tauri::async_runtime::spawn_blocking(move || notes::find_speakers(&app, id))
        .await
        .map_err(|e| e.to_string())?
        .map(|count| count as u32)
        .map_err(|e| e.to_string())
}

/// Names one of a note's voices; an empty name goes back to "Speaker 2".
#[tauri::command]
#[specta::specta]
pub fn name_note_speaker(
    notes: Notes<'_>,
    id: i64,
    speaker: i64,
    name: String,
) -> Result<(), String> {
    text(notes.name_speaker(id, speaker, &name))
}

#[tauri::command]
#[specta::specta]
pub fn get_speaker_model(app: AppHandle) -> SpeakerModel {
    speakers::state(&app)
}

/// Downloads the speaker model; progress comes as `SpeakerModelChanged`.
#[tauri::command]
#[specta::specta]
pub async fn download_speaker_model(app: AppHandle) -> Result<(), String> {
    speakers::download(&app).await
}

#[tauri::command]
#[specta::specta]
pub fn cancel_speaker_model_download() {
    speakers::cancel();
}

#[tauri::command]
#[specta::specta]
pub fn delete_speaker_model(app: AppHandle) -> Result<(), String> {
    text(speakers::delete(&app))
}

/// The voices saved when you named someone, the most recently heard first.
#[tauri::command]
#[specta::specta]
pub fn list_voices(app: AppHandle) -> Result<Vec<Voice>, String> {
    text(speakers::list(&app))
}

/// Forgets a saved voice: later meetings no longer recognise it.
#[tauri::command]
#[specta::specta]
pub fn forget_voice(app: AppHandle, id: i64) -> Result<(), String> {
    text(speakers::forget(&app, id))
}

#[tauri::command]
#[specta::specta]
pub fn change_notes_remember_voices_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.notes_remember_voices = enabled;
    write_settings(&app, settings);
    Ok(())
}

/// How many days notes keep their audio; 0 keeps it for good. Shortening
/// it removes older audio at once.
#[tauri::command]
#[specta::specta]
pub fn change_notes_audio_keep_days_setting(
    app: AppHandle,
    notes: Notes<'_>,
    days: u32,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.notes_audio_keep_days = days;
    write_settings(&app, settings);
    notes.expire_audio();
    Ok(())
}

/// Where a Word copy of every note is kept, or None to stop. The folder is
/// made if it isn't there; copies already written stay.
#[tauri::command]
#[specta::specta]
pub fn change_notes_folder_setting(app: AppHandle, folder: Option<String>) -> Result<(), String> {
    let folder = folder
        .map(|f| f.trim().to_string())
        .filter(|f| !f.is_empty());
    if let Some(f) = &folder {
        let path = std::path::Path::new(f);
        if !path.is_absolute() {
            return Err("Pick a folder on this PC".into());
        }
        std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
    }
    let mut settings = get_settings(&app);
    settings.notes_folder = folder;
    write_settings(&app, settings);
    crate::note_copies::nudge(&app);
    Ok(())
}

/// Opens the folder the notes' Word copies go to.
#[tauri::command]
#[specta::specta]
pub fn open_notes_folder(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let folder = get_settings(&app)
        .notes_folder
        .ok_or("No notes folder is set")?;
    app.opener()
        .open_path(folder, None::<String>)
        .map_err(|e| e.to_string())
}

/// Google Drive's and OneDrive's folders on this PC, offered for the copies.
#[tauri::command]
#[specta::specta]
pub async fn note_folder_suggestions() -> Vec<crate::note_copies::FolderSuggestion> {
    tauri::async_runtime::spawn_blocking(crate::note_copies::suggestions)
        .await
        .unwrap_or_default()
}

#[tauri::command]
#[specta::specta]
pub fn change_notes_speakers_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.notes_speakers = enabled;
    write_settings(&app, settings);
    Ok(())
}

/// Hides the "Let people know you're recording" reminder for good.
#[tauri::command]
#[specta::specta]
pub fn dismiss_notes_reminder(app: AppHandle) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.notes_reminder = false;
    write_settings(&app, settings);
    Ok(())
}
