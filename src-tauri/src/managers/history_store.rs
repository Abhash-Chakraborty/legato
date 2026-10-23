//! The history database: its schema, the queries Home runs and the daily
//! statistics. Plain rusqlite with no app state, so it can be tested on its
//! own; `HistoryManager` wraps it with files, settings and events.

use anyhow::{anyhow, Result};
use regex::Regex;
use rusqlite::types::ToSql;
use rusqlite::{params, Connection, OptionalExtension, Row};
use rusqlite_migration::M;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::Path;

/// Database migrations for transcription history.
/// Each migration is applied in order. The library tracks which migrations
/// have been applied using SQLite's user_version pragma.
pub static MIGRATIONS: &[M] = &[
    M::up(
        "CREATE TABLE IF NOT EXISTS transcription_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_name TEXT NOT NULL,
            timestamp INTEGER NOT NULL,
            saved BOOLEAN NOT NULL DEFAULT 0,
            title TEXT NOT NULL,
            transcription_text TEXT NOT NULL
        );",
    ),
    M::up("ALTER TABLE transcription_history ADD COLUMN post_processed_text TEXT;"),
    M::up("ALTER TABLE transcription_history ADD COLUMN post_process_prompt TEXT;"),
    M::up("ALTER TABLE transcription_history ADD COLUMN post_process_requested BOOLEAN NOT NULL DEFAULT 0;"),
    // Where each dictation went and how it went. `paste_app_*` is set only
    // when the app in front at paste differs from the one at the start.
    M::up(
        "ALTER TABLE transcription_history ADD COLUMN app_exe TEXT;
         ALTER TABLE transcription_history ADD COLUMN app_path TEXT;
         ALTER TABLE transcription_history ADD COLUMN app_name TEXT;
         ALTER TABLE transcription_history ADD COLUMN app_title TEXT;
         ALTER TABLE transcription_history ADD COLUMN app_site TEXT;
         ALTER TABLE transcription_history ADD COLUMN category TEXT;
         ALTER TABLE transcription_history ADD COLUMN paste_app_name TEXT;
         ALTER TABLE transcription_history ADD COLUMN paste_app_title TEXT;
         ALTER TABLE transcription_history ADD COLUMN speech_ms INTEGER;
         ALTER TABLE transcription_history ADD COLUMN word_count INTEGER;
         ALTER TABLE transcription_history ADD COLUMN model_id TEXT;
         ALTER TABLE transcription_history ADD COLUMN model_name TEXT;
         ALTER TABLE transcription_history ADD COLUMN transform TEXT;
         ALTER TABLE transcription_history ADD COLUMN fixes TEXT;
         ALTER TABLE transcription_history ADD COLUMN paste_ms INTEGER;
         ALTER TABLE transcription_history ADD COLUMN status TEXT;
         ALTER TABLE transcription_history ADD COLUMN error TEXT;",
    ),
    // Full-text search over what was said, what was pasted and where.
    M::up(
        "CREATE VIRTUAL TABLE history_fts USING fts5(
            transcription_text, post_processed_text, app_name, app_title,
            content='transcription_history', content_rowid='id',
            tokenize='unicode61 remove_diacritics 2'
         );
         CREATE TRIGGER history_fts_insert AFTER INSERT ON transcription_history BEGIN
            INSERT INTO history_fts(rowid, transcription_text, post_processed_text, app_name, app_title)
            VALUES (new.id, new.transcription_text, new.post_processed_text, new.app_name, new.app_title);
         END;
         CREATE TRIGGER history_fts_delete AFTER DELETE ON transcription_history BEGIN
            INSERT INTO history_fts(history_fts, rowid, transcription_text, post_processed_text, app_name, app_title)
            VALUES ('delete', old.id, old.transcription_text, old.post_processed_text, old.app_name, old.app_title);
         END;
         CREATE TRIGGER history_fts_update
         AFTER UPDATE OF transcription_text, post_processed_text, app_name, app_title ON transcription_history BEGIN
            INSERT INTO history_fts(history_fts, rowid, transcription_text, post_processed_text, app_name, app_title)
            VALUES ('delete', old.id, old.transcription_text, old.post_processed_text, old.app_name, old.app_title);
            INSERT INTO history_fts(rowid, transcription_text, post_processed_text, app_name, app_title)
            VALUES (new.id, new.transcription_text, new.post_processed_text, new.app_name, new.app_title);
         END;
         INSERT INTO history_fts(history_fts) VALUES ('rebuild');",
    ),
    // Per-day totals, kept as dictations happen so Insights survives deleted
    // transcripts. `backfill` seeds them from the rows that exist.
    M::up(
        "CREATE TABLE daily_stats (
            day TEXT PRIMARY KEY,
            dictations INTEGER NOT NULL DEFAULT 0,
            words INTEGER NOT NULL DEFAULT 0,
            speech_ms INTEGER NOT NULL DEFAULT 0,
            transformed INTEGER NOT NULL DEFAULT 0,
            fixes INTEGER NOT NULL DEFAULT 0
         );
         CREATE TABLE daily_app_stats (
            day TEXT NOT NULL,
            app TEXT NOT NULL,
            category TEXT,
            dictations INTEGER NOT NULL DEFAULT 0,
            words INTEGER NOT NULL DEFAULT 0,
            speech_ms INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (day, app)
         );
         CREATE TABLE daily_model_stats (
            day TEXT NOT NULL,
            model TEXT NOT NULL,
            dictations INTEGER NOT NULL DEFAULT 0,
            words INTEGER NOT NULL DEFAULT 0,
            speech_ms INTEGER NOT NULL DEFAULT 0,
            pastes INTEGER NOT NULL DEFAULT 0,
            paste_ms INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (day, model)
         );",
    ),
    // The Dictionary: words spelled the user's way, and the ways the model
    // mishears each one. A heard form belongs to one word.
    M::up(
        "CREATE TABLE dictionary_entry (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            text TEXT NOT NULL,
            source TEXT NOT NULL DEFAULT 'manual',
            pinned INTEGER NOT NULL DEFAULT 0,
            case_sensitive INTEGER NOT NULL DEFAULT 0,
            added_at INTEGER NOT NULL,
            last_fix_at INTEGER,
            last_app TEXT,
            last_app_path TEXT,
            hits INTEGER NOT NULL DEFAULT 0
         );
         CREATE UNIQUE INDEX dictionary_entry_text ON dictionary_entry (text COLLATE NOCASE);
         CREATE TABLE dictionary_heard (
            entry_id INTEGER NOT NULL REFERENCES dictionary_entry (id) ON DELETE CASCADE,
            heard TEXT NOT NULL
         );
         CREATE UNIQUE INDEX dictionary_heard_form ON dictionary_heard (heard COLLATE NOCASE);
         CREATE INDEX dictionary_heard_entry ON dictionary_heard (entry_id);",
    ),
    // Transform runs, for the Transforms page's counts and Recent runs.
    M::up(
        "CREATE TABLE transform_run (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            at INTEGER NOT NULL,
            transform_id TEXT NOT NULL,
            name TEXT NOT NULL,
            source TEXT NOT NULL,
            app_name TEXT,
            app_path TEXT,
            in_words INTEGER NOT NULL DEFAULT 0,
            out_words INTEGER NOT NULL DEFAULT 0,
            ms INTEGER NOT NULL DEFAULT 0,
            result TEXT NOT NULL,
            error TEXT,
            history_id INTEGER
         );
         CREATE INDEX transform_run_at ON transform_run (at);",
    ),
    // Snippets: a trigger phrase and the text it pastes.
    M::up(
        "CREATE TABLE snippet (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            trigger TEXT NOT NULL,
            text TEXT NOT NULL,
            alone INTEGER NOT NULL DEFAULT 0,
            uses INTEGER NOT NULL DEFAULT 0,
            added_at INTEGER NOT NULL,
            last_used_at INTEGER,
            last_app TEXT,
            last_app_path TEXT
         );",
    ),
    // What the model heard, for Style's Heard and Pasted example.
    M::up("ALTER TABLE transcription_history ADD COLUMN heard_text TEXT;"),
    // The Scratchpad: notes, which of them are open as tabs, and versions.
    M::up(
        "CREATE TABLE scratch_note (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL DEFAULT '',
            auto_title INTEGER NOT NULL DEFAULT 1,
            text TEXT NOT NULL DEFAULT '',
            pinned INTEGER NOT NULL DEFAULT 0,
            tab INTEGER,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            source TEXT,
            source_app TEXT,
            source_app_path TEXT,
            reason TEXT
         );
         CREATE TABLE scratch_version (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            note_id INTEGER NOT NULL,
            text TEXT NOT NULL,
            at INTEGER NOT NULL
         );
         CREATE INDEX scratch_version_note ON scratch_version (note_id, at);",
    ),
    // Learn from my edits: words waiting under Needs review, and words never
    // to propose again.
    M::up(
        "CREATE TABLE dictionary_pending (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            word TEXT NOT NULL,
            heard TEXT NOT NULL,
            context TEXT NOT NULL DEFAULT '',
            app TEXT,
            app_path TEXT,
            at INTEGER NOT NULL
         );
         CREATE UNIQUE INDEX dictionary_pending_pair
            ON dictionary_pending (word COLLATE NOCASE, heard COLLATE NOCASE);
         CREATE TABLE dictionary_never (
            word TEXT PRIMARY KEY COLLATE NOCASE,
            added_at INTEGER NOT NULL
         );",
    ),
    // Notes: recordings with their transcript, tags, summary and action
    // items.
    M::up(
        "CREATE TABLE note (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            kind TEXT NOT NULL,
            status TEXT NOT NULL,
            app TEXT,
            app_path TEXT,
            microphone TEXT,
            system_audio TEXT,
            started_at INTEGER NOT NULL,
            duration_ms INTEGER NOT NULL DEFAULT 0,
            audio_file TEXT,
            my_notes TEXT NOT NULL DEFAULT '',
            summary TEXT NOT NULL DEFAULT '',
            decisions TEXT NOT NULL DEFAULT '[]',
            summary_model TEXT,
            summary_at INTEGER,
            summary_error TEXT,
            failed_lines INTEGER NOT NULL DEFAULT 0
         );
         CREATE TABLE note_line (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            note_id INTEGER NOT NULL,
            at_ms INTEGER NOT NULL,
            end_ms INTEGER NOT NULL,
            who TEXT NOT NULL,
            text TEXT NOT NULL
         );
         CREATE INDEX note_line_note ON note_line (note_id, at_ms);
         CREATE TABLE note_tag (
            note_id INTEGER NOT NULL,
            tag TEXT NOT NULL COLLATE NOCASE,
            PRIMARY KEY (note_id, tag)
         );
         CREATE TABLE note_action (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            note_id INTEGER NOT NULL,
            owner TEXT NOT NULL,
            text TEXT NOT NULL,
            done INTEGER NOT NULL DEFAULT 0,
            position INTEGER NOT NULL DEFAULT 0
         );
         CREATE INDEX note_action_note ON note_action (note_id, position);",
    ),
    // Insights: snippets pasted and fixes per model, dictations by hour, and
    // filler words removed, which are counted from this version on.
    M::up(
        "ALTER TABLE daily_stats ADD COLUMN snippets INTEGER NOT NULL DEFAULT 0;
         ALTER TABLE daily_model_stats ADD COLUMN fixes INTEGER NOT NULL DEFAULT 0;
         CREATE TABLE daily_hour_stats (
            day TEXT NOT NULL,
            hour INTEGER NOT NULL,
            dictations INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (day, hour)
         );
         CREATE TABLE daily_filler_stats (
            day TEXT NOT NULL,
            word TEXT NOT NULL,
            count INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (day, word)
         );
         CREATE TABLE insights_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
         INSERT INTO insights_meta (key, value) VALUES ('counted_since', date('now', 'localtime'));
         INSERT INTO daily_hour_stats (day, hour, dictations)
            SELECT date(timestamp, 'unixepoch', 'localtime'),
                   CAST(strftime('%H', timestamp, 'unixepoch', 'localtime') AS INTEGER), COUNT(*)
            FROM transcription_history WHERE transcription_text != '' GROUP BY 1, 2;
         UPDATE daily_model_stats SET fixes = COALESCE((
            SELECT SUM(json_array_length(h.fixes)) FROM transcription_history h
            WHERE h.model_id = daily_model_stats.model AND h.fixes IS NOT NULL
              AND date(h.timestamp, 'unixepoch', 'localtime') = daily_model_stats.day), 0);",
    ),
    // Notes: the stretches of a recording the speech model couldn't
    // transcribe, so they can be tried again from the note's audio.
    M::up(
        "CREATE TABLE note_failed_line (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            note_id INTEGER NOT NULL,
            at_ms INTEGER NOT NULL,
            end_ms INTEGER NOT NULL,
            who TEXT NOT NULL
         );
         CREATE INDEX note_failed_line_note ON note_failed_line (note_id, at_ms);",
    ),
    // Accuracy in Insights: how long each model took once you stopped
    // speaking, and how often it ran on the GPU. Counted from this version.
    M::up(
        "ALTER TABLE transcription_history ADD COLUMN transcribe_ms INTEGER;
         ALTER TABLE transcription_history ADD COLUMN on_gpu INTEGER;
         ALTER TABLE daily_model_stats ADD COLUMN timed INTEGER NOT NULL DEFAULT 0;
         ALTER TABLE daily_model_stats ADD COLUMN transcribe_ms INTEGER NOT NULL DEFAULT 0;
         ALTER TABLE daily_model_stats ADD COLUMN on_gpu INTEGER NOT NULL DEFAULT 0;",
    ),
    // Phone sync (sync/ledger.rs): the last known version of every item that
    // syncs, rows that came from another device, deletes to pass on, paired
    // devices, merges and recent clips.
    M::up(
        "CREATE TABLE sync_item (
            kind TEXT NOT NULL,
            key TEXT NOT NULL,
            hash TEXT NOT NULL,
            body TEXT,
            changed_at INTEGER NOT NULL,
            origin TEXT NOT NULL,
            seq INTEGER NOT NULL,
            PRIMARY KEY (kind, key)
         );
         CREATE INDEX sync_item_seq ON sync_item (seq);
         CREATE TABLE sync_local (
            kind TEXT NOT NULL,
            local_id INTEGER NOT NULL,
            key TEXT NOT NULL,
            device TEXT NOT NULL,
            PRIMARY KEY (kind, local_id)
         );
         CREATE UNIQUE INDEX sync_local_key ON sync_local (kind, key);
         CREATE TABLE sync_gone (
            kind TEXT NOT NULL,
            local_id INTEGER NOT NULL,
            at INTEGER NOT NULL,
            PRIMARY KEY (kind, local_id)
         );
         CREATE TABLE sync_peer (
            device_id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            platform TEXT NOT NULL,
            app_version TEXT NOT NULL DEFAULT '',
            fingerprint TEXT NOT NULL,
            paired_at INTEGER NOT NULL,
            last_seen INTEGER,
            last_sync INTEGER,
            sent_upto INTEGER NOT NULL DEFAULT 0,
            recv_upto INTEGER NOT NULL DEFAULT 0
         );
         CREATE TABLE sync_merge (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            at INTEGER NOT NULL,
            peer TEXT NOT NULL,
            kind TEXT NOT NULL,
            key TEXT NOT NULL,
            label TEXT NOT NULL,
            kept_theirs INTEGER NOT NULL
         );
         CREATE TABLE sync_clip (
            id TEXT PRIMARY KEY,
            at INTEGER NOT NULL,
            peer TEXT NOT NULL,
            incoming INTEGER NOT NULL,
            kind TEXT NOT NULL,
            text TEXT,
            image TEXT,
            bytes INTEGER NOT NULL,
            late INTEGER NOT NULL DEFAULT 0,
            from_app TEXT
         );",
    ),
    // Notes: when the transcript was last corrected, so a summary written
    // before that can say it's out of date.
    M::up("ALTER TABLE note ADD COLUMN edited_at INTEGER;"),
    // Notes: who said each line when the speaker model told voices apart
    // (1, 2, … within the note; fixed once you choose it by hand), the
    // names given to them or recognised from a saved voice, and the voices
    // saved when you name someone, as clips in the voices folder.
    M::up(
        "ALTER TABLE note_line ADD COLUMN speaker INTEGER;
         ALTER TABLE note_line ADD COLUMN speaker_fixed INTEGER NOT NULL DEFAULT 0;
         CREATE TABLE note_speaker (
            note_id INTEGER NOT NULL,
            speaker INTEGER NOT NULL,
            name TEXT NOT NULL,
            recognised INTEGER NOT NULL DEFAULT 0,
            voice_id INTEGER,
            PRIMARY KEY (note_id, speaker)
         );
         CREATE TABLE voice (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE COLLATE NOCASE,
            file TEXT NOT NULL,
            ms INTEGER NOT NULL,
            created_at INTEGER NOT NULL,
            used_at INTEGER NOT NULL
         );",
    ),
    // Insights: each app's executable, for its icon, kept with the totals
    // because the history rows it came from are trimmed or deleted.
    M::up("ALTER TABLE daily_app_stats ADD COLUMN app_path TEXT;"),
    // Windows Terminal describes itself as "Windows Terminal Host"; the
    // dictations saved under that name move to the one shown now.
    M::up(
        "UPDATE transcription_history SET app_name = 'Windows Terminal'
            WHERE app_name = 'Windows Terminal Host';
         UPDATE daily_app_stats AS old SET app = 'Windows Terminal'
            WHERE app = 'Windows Terminal Host' AND NOT EXISTS
               (SELECT 1 FROM daily_app_stats n
                WHERE n.day = old.day AND n.app = 'Windows Terminal');",
    ),
    // Notes: the summary's topics, details and people (JSON), and the
    // people invited when the note came from a calendar meeting.
    M::up(
        "ALTER TABLE note ADD COLUMN outline TEXT;
         ALTER TABLE note ADD COLUMN attendees TEXT;",
    ),
    // Recently deleted: a deleted note or Scratchpad note, whole, for 30
    // days.
    M::up(
        "CREATE TABLE trash (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            kind TEXT NOT NULL,
            item_id INTEGER NOT NULL,
            title TEXT NOT NULL,
            preview TEXT NOT NULL DEFAULT '',
            duration_ms INTEGER,
            deleted_at INTEGER NOT NULL,
            payload TEXT NOT NULL
        );
        CREATE INDEX idx_trash_deleted_at ON trash(deleted_at);",
    ),
];

/// The schema version that added the columns and tables `backfill` fills in.
pub const BACKFILL_VERSION: i32 = 7;

/// The schema version that added the Dictionary; custom words from the
/// settings move into it when a database first reaches it.
pub const DICTIONARY_VERSION: i32 = 8;

/// What happened to a dictation after it was transcribed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum DictationStatus {
    Pasted,
    /// The paste itself failed; `error` says why.
    PasteFailed,
    /// Paste method is None, so the text was only kept here.
    NotPasted,
    /// Transcription failed; the audio is kept so it can be tried again.
    Failed,
}

impl DictationStatus {
    fn as_str(self) -> &'static str {
        match self {
            DictationStatus::Pasted => "pasted",
            DictationStatus::PasteFailed => "paste_failed",
            DictationStatus::NotPasted => "not_pasted",
            DictationStatus::Failed => "failed",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "pasted" => Some(DictationStatus::Pasted),
            "paste_failed" => Some(DictationStatus::PasteFailed),
            "not_pasted" => Some(DictationStatus::NotPasted),
            "failed" => Some(DictationStatus::Failed),
            _ => None,
        }
    }
}

/// What made a fix.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum FixSource {
    /// A heard-as form in the Dictionary.
    Heard,
    /// A close match to a Dictionary word.
    #[default]
    Match,
    /// Fixed by hand with Fix a word.
    Manual,
    /// Fixed by the AI correction pass.
    Ai,
}

/// A fix made on one dictation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct AppliedFix {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub source: FixSource,
}

fn fixes_json(fixes: &[AppliedFix]) -> Option<String> {
    if fixes.is_empty() {
        None
    } else {
        serde_json::to_string(fixes).ok()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Type)]
pub struct HistoryEntry {
    pub id: i64,
    pub file_name: String,
    pub timestamp: i64,
    pub saved: bool,
    pub title: String,
    pub transcription_text: String,
    pub post_processed_text: Option<String>,
    pub post_process_prompt: Option<String>,
    pub post_process_requested: bool,
    pub app_exe: Option<String>,
    pub app_path: Option<String>,
    pub app_name: Option<String>,
    pub app_title: Option<String>,
    pub app_site: Option<String>,
    pub category: Option<String>,
    pub paste_app_name: Option<String>,
    pub speech_ms: Option<i64>,
    pub word_count: Option<i64>,
    pub model_id: Option<String>,
    pub model_name: Option<String>,
    /// Name of the prompt that rewrote the text.
    pub transform: Option<String>,
    pub fixes: Vec<AppliedFix>,
    /// From the end of recording to the paste.
    pub paste_ms: Option<i64>,
    pub status: Option<DictationStatus>,
    pub error: Option<String>,
    /// What the model heard, before the Dictionary, cleanup and Style.
    pub heard_text: Option<String>,
    /// From the end of recording to the model's text.
    pub transcribe_ms: Option<i64>,
    /// Whether the model ran on the GPU.
    pub on_gpu: Option<bool>,
    /// The paired phone it was dictated on; None for this PC.
    pub device: Option<String>,
}

pub const COLUMNS: &str = "id, file_name, timestamp, saved, title, transcription_text,
    post_processed_text, post_process_prompt, post_process_requested, app_exe, app_path,
    app_name, app_title, app_site, category, paste_app_name, speech_ms, word_count,
    model_id, model_name, transform, fixes, paste_ms, status, error, heard_text,
    transcribe_ms, on_gpu,
    (SELECT device FROM sync_local
     WHERE kind = 'history' AND local_id = transcription_history.id) AS device";

pub fn map_entry(row: &Row<'_>) -> rusqlite::Result<HistoryEntry> {
    let fixes: Option<String> = row.get("fixes")?;
    let status: Option<String> = row.get("status")?;
    Ok(HistoryEntry {
        id: row.get("id")?,
        file_name: row.get("file_name")?,
        timestamp: row.get("timestamp")?,
        saved: row.get("saved")?,
        title: row.get("title")?,
        transcription_text: row.get("transcription_text")?,
        post_processed_text: row.get("post_processed_text")?,
        post_process_prompt: row.get("post_process_prompt")?,
        post_process_requested: row.get("post_process_requested")?,
        app_exe: row.get("app_exe")?,
        app_path: row.get("app_path")?,
        app_name: row.get("app_name")?,
        app_title: row.get("app_title")?,
        app_site: row.get("app_site")?,
        category: row.get("category")?,
        paste_app_name: row.get("paste_app_name")?,
        speech_ms: row.get("speech_ms")?,
        word_count: row.get("word_count")?,
        model_id: row.get("model_id")?,
        model_name: row.get("model_name")?,
        transform: row.get("transform")?,
        fixes: fixes
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default(),
        paste_ms: row.get("paste_ms")?,
        status: status.as_deref().and_then(DictationStatus::parse),
        error: row.get("error")?,
        heard_text: row.get("heard_text")?,
        transcribe_ms: row.get("transcribe_ms")?,
        on_gpu: row.get("on_gpu")?,
        device: row.get("device")?,
    })
}

/// The app a dictation went into.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AppFields {
    pub exe: String,
    pub path: String,
    pub name: String,
    pub title: String,
    pub site: Option<String>,
    pub category: String,
}

/// Everything known about a dictation when it is first saved.
#[derive(Clone, Debug, Default)]
pub struct NewEntry {
    pub file_name: String,
    pub timestamp: i64,
    pub title: String,
    pub transcription_text: String,
    pub post_process_requested: bool,
    pub post_processed_text: Option<String>,
    pub post_process_prompt: Option<String>,
    pub transform: Option<String>,
    pub app: Option<AppFields>,
    pub speech_ms: Option<i64>,
    pub model_id: Option<String>,
    pub model_name: Option<String>,
    pub status: Option<DictationStatus>,
    pub fixes: Vec<AppliedFix>,
    pub heard_text: Option<String>,
    /// Filler words removed, with how many times each.
    pub fillers: Vec<(String, i64)>,
    /// Snippets the dictation pasted.
    pub snippets: i64,
    pub transcribe_ms: Option<i64>,
    pub on_gpu: Option<bool>,
}

/// How the paste went.
#[derive(Clone, Debug)]
pub struct PasteRecord {
    pub status: DictationStatus,
    pub error: Option<String>,
    pub paste_ms: Option<i64>,
    /// Set when the app in front at paste differs from the one at the start.
    pub paste_app: Option<(String, String)>,
}

/// Words in a transcript. Chinese, Japanese and Korean characters count one
/// each, since those scripts don't put spaces between words.
pub fn word_count(text: &str) -> i64 {
    let mut count = 0i64;
    for token in text.split_whitespace() {
        let cjk = token.chars().filter(|c| is_cjk(*c)).count() as i64;
        let rest = token
            .chars()
            .map(|c| if is_cjk(c) { ' ' } else { c })
            .collect::<String>();
        count += cjk
            + rest
                .split_whitespace()
                .filter(|w| w.chars().any(char::is_alphanumeric))
                .count() as i64;
    }
    count
}

fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF)
}

fn final_text(text: &str, processed: Option<&str>) -> String {
    processed.unwrap_or(text).to_string()
}

const DAY_OF: &str = "date(?1, 'unixepoch', 'localtime')";

/// Adds one dictation (or removes one, with `sign` -1) to the daily tables.
fn add_to_stats(conn: &Connection, entry: &HistoryEntry, sign: i64) -> Result<()> {
    if entry.transcription_text.is_empty() {
        return Ok(());
    }
    let words = sign * entry.word_count.unwrap_or(0);
    let speech = sign * entry.speech_ms.unwrap_or(0);
    let transformed = sign * i64::from(entry.post_processed_text.is_some());
    let fixes = sign * entry.fixes.len() as i64;
    conn.execute(
        &format!(
            "INSERT INTO daily_stats (day, dictations, words, speech_ms, transformed, fixes)
             VALUES ({DAY_OF}, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(day) DO UPDATE SET
                dictations = dictations + excluded.dictations,
                words = words + excluded.words,
                speech_ms = speech_ms + excluded.speech_ms,
                transformed = transformed + excluded.transformed,
                fixes = fixes + excluded.fixes"
        ),
        params![entry.timestamp, sign, words, speech, transformed, fixes],
    )?;
    if let Some(app) = entry.app_name.as_deref().filter(|a| !a.is_empty()) {
        conn.execute(
            &format!(
                "INSERT INTO daily_app_stats
                    (day, app, category, dictations, words, speech_ms, app_path)
                 VALUES ({DAY_OF}, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(day, app) DO UPDATE SET
                    category = excluded.category,
                    dictations = dictations + excluded.dictations,
                    words = words + excluded.words,
                    speech_ms = speech_ms + excluded.speech_ms,
                    app_path = COALESCE(excluded.app_path, app_path)"
            ),
            params![
                entry.timestamp,
                app,
                entry.category,
                sign,
                words,
                speech,
                entry.app_path.as_deref().filter(|p| !p.is_empty())
            ],
        )?;
    }
    if let Some(model) = entry.model_id.as_deref().filter(|m| !m.is_empty()) {
        let timed = sign * i64::from(entry.transcribe_ms.is_some());
        let transcribe = sign * entry.transcribe_ms.unwrap_or(0);
        let on_gpu = sign * i64::from(entry.transcribe_ms.is_some() && entry.on_gpu == Some(true));
        conn.execute(
            &format!(
                "INSERT INTO daily_model_stats
                    (day, model, dictations, words, speech_ms, fixes, timed, transcribe_ms, on_gpu)
                 VALUES ({DAY_OF}, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(day, model) DO UPDATE SET
                    dictations = dictations + excluded.dictations,
                    words = words + excluded.words,
                    speech_ms = speech_ms + excluded.speech_ms,
                    fixes = fixes + excluded.fixes,
                    timed = timed + excluded.timed,
                    transcribe_ms = transcribe_ms + excluded.transcribe_ms,
                    on_gpu = on_gpu + excluded.on_gpu"
            ),
            params![
                entry.timestamp,
                model,
                sign,
                words,
                speech,
                fixes,
                timed,
                transcribe,
                on_gpu
            ],
        )?;
    }
    conn.execute(
        &format!(
            "INSERT INTO daily_hour_stats (day, hour, dictations)
             VALUES ({DAY_OF}, CAST(strftime('%H', ?1, 'unixepoch', 'localtime') AS INTEGER), ?2)
             ON CONFLICT(day, hour) DO UPDATE SET dictations = dictations + excluded.dictations"
        ),
        params![entry.timestamp, sign],
    )?;
    Ok(())
}

/// Counts what only a new dictation knows: the fillers removed and the
/// snippets pasted. Text fixed later doesn't change them.
fn add_cleanup_stats(conn: &Connection, new: &NewEntry) -> Result<()> {
    if new.transcription_text.is_empty() {
        return Ok(());
    }
    for (word, count) in &new.fillers {
        conn.execute(
            &format!(
                "INSERT INTO daily_filler_stats (day, word, count) VALUES ({DAY_OF}, ?2, ?3)
                 ON CONFLICT(day, word) DO UPDATE SET count = count + excluded.count"
            ),
            params![new.timestamp, word, count],
        )?;
    }
    if new.snippets > 0 {
        conn.execute(
            &format!(
                "INSERT INTO daily_stats (day, snippets) VALUES ({DAY_OF}, ?2)
                 ON CONFLICT(day) DO UPDATE SET snippets = snippets + excluded.snippets"
            ),
            params![new.timestamp, new.snippets],
        )?;
    }
    Ok(())
}

pub fn get(conn: &Connection, id: i64) -> Result<Option<HistoryEntry>> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM transcription_history WHERE id = ?1"),
            params![id],
            map_entry,
        )
        .optional()?)
}

/// Saves a dictation and counts it in the daily tables.
pub fn insert(conn: &mut Connection, new: &NewEntry) -> Result<HistoryEntry> {
    let tx = conn.transaction()?;
    let words = word_count(&final_text(
        &new.transcription_text,
        new.post_processed_text.as_deref(),
    ));
    let app = new.app.as_ref();
    tx.execute(
        "INSERT INTO transcription_history (
            file_name, timestamp, saved, title, transcription_text, post_processed_text,
            post_process_prompt, post_process_requested, app_exe, app_path, app_name,
            app_title, app_site, category, speech_ms, word_count, model_id, model_name,
            transform, status, fixes, heard_text, transcribe_ms, on_gpu
         ) VALUES (?1, ?2, 0, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)",
        params![
            new.file_name,
            new.timestamp,
            new.title,
            new.transcription_text,
            new.post_processed_text,
            new.post_process_prompt,
            new.post_process_requested,
            app.map(|a| &a.exe),
            app.map(|a| &a.path),
            app.map(|a| &a.name),
            app.map(|a| &a.title),
            app.and_then(|a| a.site.as_ref()),
            app.map(|a| &a.category),
            new.speech_ms,
            words,
            new.model_id,
            new.model_name,
            new.transform,
            new.status.map(DictationStatus::as_str),
            fixes_json(&new.fixes),
            new.heard_text,
            new.transcribe_ms,
            new.on_gpu,
        ],
    )?;
    let entry = get(&tx, tx.last_insert_rowid())?.ok_or_else(|| anyhow!("Saved row is missing"))?;
    add_to_stats(&tx, &entry, 1)?;
    add_cleanup_stats(&tx, new)?;
    tx.commit()?;
    Ok(entry)
}

/// Records how the paste went.
pub fn record_paste(conn: &mut Connection, id: i64, paste: &PasteRecord) -> Result<HistoryEntry> {
    let tx = conn.transaction()?;
    let updated = tx.execute(
        "UPDATE transcription_history
         SET status = ?1, error = ?2, paste_ms = ?3, paste_app_name = ?4, paste_app_title = ?5
         WHERE id = ?6",
        params![
            paste.status.as_str(),
            paste.error,
            paste.paste_ms,
            paste.paste_app.as_ref().map(|(name, _)| name),
            paste.paste_app.as_ref().map(|(_, title)| title),
            id
        ],
    )?;
    if updated == 0 {
        return Err(anyhow!("History entry {} not found", id));
    }
    let entry = get(&tx, id)?.ok_or_else(|| anyhow!("History entry {} not found", id))?;
    if let (Some(model), Some(ms), DictationStatus::Pasted) =
        (entry.model_id.as_deref(), entry.paste_ms, paste.status)
    {
        tx.execute(
            &format!(
                "INSERT INTO daily_model_stats (day, model, pastes, paste_ms)
                 VALUES ({DAY_OF}, ?2, 1, ?3)
                 ON CONFLICT(day, model) DO UPDATE SET
                    pastes = pastes + 1, paste_ms = paste_ms + excluded.paste_ms"
            ),
            params![entry.timestamp, model, ms],
        )?;
    }
    tx.commit()?;
    Ok(entry)
}

/// Replaces a dictation's text after it was transcribed again, keeping the
/// daily tables in step.
#[allow(clippy::too_many_arguments)]
pub fn update_transcription(
    conn: &mut Connection,
    id: i64,
    transcription_text: &str,
    post_processed_text: Option<&str>,
    post_process_prompt: Option<&str>,
    transform: Option<&str>,
    model: Option<(&str, &str)>,
    fixes: &[AppliedFix],
    heard_text: Option<&str>,
) -> Result<HistoryEntry> {
    let tx = conn.transaction()?;
    let before = get(&tx, id)?.ok_or_else(|| anyhow!("History entry {} not found", id))?;
    add_to_stats(&tx, &before, -1)?;
    let words = word_count(&final_text(transcription_text, post_processed_text));
    let status = if before.status == Some(DictationStatus::Failed) {
        None
    } else {
        before.status.map(DictationStatus::as_str)
    };
    tx.execute(
        "UPDATE transcription_history
         SET transcription_text = ?1, post_processed_text = ?2, post_process_prompt = ?3,
             transform = ?4, word_count = ?5, status = ?6, error = NULL,
             model_id = COALESCE(?7, model_id), model_name = COALESCE(?8, model_name),
             fixes = ?9, heard_text = COALESCE(?11, heard_text)
         WHERE id = ?10",
        params![
            transcription_text,
            post_processed_text,
            post_process_prompt,
            transform,
            words,
            status,
            model.map(|(id, _)| id),
            model.map(|(_, name)| name),
            fixes_json(fixes),
            id,
            heard_text
        ],
    )?;
    let after = get(&tx, id)?.ok_or_else(|| anyhow!("History entry {} not found", id))?;
    add_to_stats(&tx, &after, 1)?;
    tx.commit()?;
    Ok(after)
}

/// A dictation's text and fixes before Fix a word changed it, kept for Undo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextSnapshot {
    pub id: i64,
    pub transcription_text: String,
    pub post_processed_text: Option<String>,
    pub fixes: Vec<AppliedFix>,
}

impl TextSnapshot {
    fn of(entry: &HistoryEntry) -> Self {
        Self {
            id: entry.id,
            transcription_text: entry.transcription_text.clone(),
            post_processed_text: entry.post_processed_text.clone(),
            fixes: entry.fixes.clone(),
        }
    }
}

/// Matches `from` as whole words, in any case and with any spacing.
fn word_pattern(from: &str) -> Option<Regex> {
    let from = from.trim();
    if from.is_empty() {
        return None;
    }
    let body = from
        .split_whitespace()
        .map(regex::escape)
        .collect::<Vec<_>>()
        .join(r"\s+");
    let start = if from.starts_with(char::is_alphanumeric) {
        r"\b"
    } else {
        ""
    };
    let end = if from.ends_with(char::is_alphanumeric) {
        r"\b"
    } else {
        ""
    };
    Regex::new(&format!("(?i){start}{body}{end}")).ok()
}

/// Replaces every whole-word `from` that doesn't already read `to`.
fn replace_words(text: &str, pattern: &Regex, to: &str) -> (String, usize) {
    let mut count = 0;
    let out = pattern.replace_all(text, |caps: &regex::Captures| {
        if &caps[0] == to {
            caps[0].to_string()
        } else {
            count += 1;
            to.to_string()
        }
    });
    (out.into_owned(), count)
}

/// Writes new text and fixes for a dictation, keeping the daily tables in step.
fn set_text(
    tx: &Connection,
    before: &HistoryEntry,
    transcription_text: &str,
    post_processed_text: Option<&str>,
    fixes: &[AppliedFix],
) -> Result<HistoryEntry> {
    add_to_stats(tx, before, -1)?;
    let words = word_count(&final_text(transcription_text, post_processed_text));
    tx.execute(
        "UPDATE transcription_history
         SET transcription_text = ?1, post_processed_text = ?2, word_count = ?3, fixes = ?4
         WHERE id = ?5",
        params![
            transcription_text,
            post_processed_text,
            words,
            fixes_json(fixes),
            before.id
        ],
    )?;
    let after =
        get(tx, before.id)?.ok_or_else(|| anyhow!("History entry {} not found", before.id))?;
    add_to_stats(tx, &after, 1)?;
    Ok(after)
}

/// Fix a word on one dictation. `shown_after` replaces the text Home shows
/// (the rewritten text when there is one), but only if it still reads
/// `shown_before`; the other version gets every whole-word `fix.from`
/// replaced.
pub fn fix_row(
    conn: &mut Connection,
    id: i64,
    shown_before: &str,
    shown_after: &str,
    fix: &AppliedFix,
) -> Result<(TextSnapshot, HistoryEntry)> {
    let tx = conn.transaction()?;
    let before = get(&tx, id)?.ok_or_else(|| anyhow!("History entry {} not found", id))?;
    if final_text(
        &before.transcription_text,
        before.post_processed_text.as_deref(),
    ) != shown_before
    {
        return Err(anyhow!("This transcript changed; try again"));
    }
    let pattern = word_pattern(&fix.from);
    let other = |text: &str| match &pattern {
        Some(p) => replace_words(text, p, &fix.to).0,
        None => text.to_string(),
    };
    let (text, processed) = match &before.post_processed_text {
        Some(_) => (
            other(&before.transcription_text),
            Some(shown_after.to_string()),
        ),
        None => (shown_after.to_string(), None),
    };
    let mut fixes = before.fixes.clone();
    fixes.push(fix.clone());
    let after = set_text(&tx, &before, &text, processed.as_deref(), &fixes)?;
    tx.commit()?;
    Ok((TextSnapshot::of(&before), after))
}

/// Dictations other than `except` where `from` appears as whole words and
/// doesn't already read `to`, newest first.
pub fn rows_with(conn: &Connection, from: &str, to: &str, except: i64) -> Result<Vec<i64>> {
    let Some(pattern) = word_pattern(from) else {
        return Ok(Vec::new());
    };
    // LIKE narrows the rows cheaply; the pattern decides.
    let like = format!("%{}%", from.split_whitespace().next().unwrap_or(from));
    let mut stmt = conn.prepare(
        "SELECT id, transcription_text, post_processed_text FROM transcription_history
         WHERE id != ?1 AND (transcription_text LIKE ?2 OR post_processed_text LIKE ?2)
         ORDER BY id DESC",
    )?;
    let rows = stmt
        .query_map(params![except, like], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows
        .into_iter()
        .filter(|(_, text, processed)| {
            std::iter::once(text.as_str())
                .chain(processed.as_deref())
                .any(|t| replace_words(t, &pattern, to).1 > 0)
        })
        .map(|(id, _, _)| id)
        .collect())
}

/// Replaces `fix.from` with `fix.to` in other dictations, both versions.
pub fn fix_rows(
    conn: &mut Connection,
    ids: &[i64],
    fix: &AppliedFix,
) -> Result<Vec<(TextSnapshot, HistoryEntry)>> {
    let Some(pattern) = word_pattern(&fix.from) else {
        return Ok(Vec::new());
    };
    let tx = conn.transaction()?;
    let mut changed = Vec::new();
    for &id in ids {
        let Some(before) = get(&tx, id)? else {
            continue;
        };
        let (text, a) = replace_words(&before.transcription_text, &pattern, &fix.to);
        let processed = before
            .post_processed_text
            .as_deref()
            .map(|p| replace_words(p, &pattern, &fix.to));
        if a == 0 && processed.as_ref().is_none_or(|(_, b)| *b == 0) {
            continue;
        }
        let mut fixes = before.fixes.clone();
        fixes.push(fix.clone());
        let after = set_text(
            &tx,
            &before,
            &text,
            processed.as_ref().map(|(p, _)| p.as_str()),
            &fixes,
        )?;
        changed.push((TextSnapshot::of(&before), after));
    }
    tx.commit()?;
    Ok(changed)
}

/// Puts dictations back the way they were before a fix (Undo). Rows deleted
/// since are skipped.
pub fn restore_texts(
    conn: &mut Connection,
    snapshots: &[TextSnapshot],
) -> Result<Vec<HistoryEntry>> {
    let tx = conn.transaction()?;
    let mut restored = Vec::new();
    for snap in snapshots {
        let Some(before) = get(&tx, snap.id)? else {
            continue;
        };
        restored.push(set_text(
            &tx,
            &before,
            &snap.transcription_text,
            snap.post_processed_text.as_deref(),
            &snap.fixes,
        )?);
    }
    tx.commit()?;
    Ok(restored)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum HistoryFilter {
    #[default]
    All,
    Starred,
    Fixed,
    Transformed,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Type)]
pub struct HistoryQuery {
    /// Return rows older than this id.
    pub cursor: Option<i64>,
    pub limit: Option<u32>,
    pub search: Option<String>,
    #[serde(default)]
    pub filter: HistoryFilter,
    /// An app's name, as in `HistoryEntry::app_name`.
    pub app: Option<String>,
    /// A local date, YYYY-MM-DD.
    #[serde(default)]
    pub day: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct PaginatedHistory {
    pub entries: Vec<HistoryEntry>,
    pub has_more: bool,
}

/// Turns what was typed into an FTS5 query: every word must appear, each as
/// a prefix, so "dok net" finds "Dokploy" and "Netbird".
pub fn fts_query(input: &str) -> Option<String> {
    let terms: Vec<String> = input
        .split_whitespace()
        .map(|t| format!("\"{}\"*", t.replace('"', "\"\"")))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" "))
}

const FILTER_SQL: [(HistoryFilter, &str); 3] = [
    (HistoryFilter::Starred, "saved = 1"),
    (HistoryFilter::Fixed, "fixes IS NOT NULL AND fixes != '[]'"),
    (
        HistoryFilter::Transformed,
        "post_processed_text IS NOT NULL",
    ),
];

pub fn query(conn: &Connection, q: &HistoryQuery) -> Result<PaginatedHistory> {
    let mut clauses: Vec<String> = Vec::new();
    let mut args: Vec<Box<dyn ToSql>> = Vec::new();
    if let Some(cursor) = q.cursor {
        args.push(Box::new(cursor));
        clauses.push(format!("id < ?{}", args.len()));
    }
    if let Some(fts) = q.search.as_deref().and_then(fts_query) {
        args.push(Box::new(fts));
        clauses.push(format!(
            "id IN (SELECT rowid FROM history_fts WHERE history_fts MATCH ?{})",
            args.len()
        ));
    }
    if let Some((_, sql)) = FILTER_SQL.iter().find(|(f, _)| *f == q.filter) {
        clauses.push(sql.to_string());
    }
    if let Some(app) = q.app.as_deref().filter(|a| !a.is_empty()) {
        args.push(Box::new(app.to_string()));
        clauses.push(format!("app_name = ?{}", args.len()));
    }
    if let Some(day) = q.day.as_deref().filter(|d| !d.is_empty()) {
        args.push(Box::new(day.to_string()));
        clauses.push(format!(
            "date(timestamp, 'unixepoch', 'localtime') = ?{}",
            args.len()
        ));
    }
    let limit = q.limit.map(|l| l.clamp(1, 200));
    let mut sql = format!("SELECT {COLUMNS} FROM transcription_history");
    if !clauses.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&clauses.join(" AND "));
    }
    sql.push_str(" ORDER BY id DESC");
    if let Some(limit) = limit {
        sql.push_str(&format!(" LIMIT {}", limit + 1));
    }
    let mut stmt = conn.prepare(&sql)?;
    let mut entries = stmt
        .query_map(
            rusqlite::params_from_iter(args.iter().map(|a| a.as_ref())),
            map_entry,
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let has_more = limit.is_some_and(|l| entries.len() > l as usize);
    if has_more {
        entries.pop();
    }
    Ok(PaginatedHistory { entries, has_more })
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct DayStats {
    /// Local date, YYYY-MM-DD.
    pub day: String,
    pub dictations: i64,
    pub words: i64,
    pub speech_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct AppCount {
    pub name: String,
    /// An executable to take the app's icon from.
    pub path: Option<String>,
    pub count: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct FilterCounts {
    pub all: i64,
    pub starred: i64,
    pub fixed: i64,
    pub transformed: i64,
}

/// One fix made today, for Home's rail.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct TodayFix {
    pub history_id: i64,
    pub timestamp: i64,
    pub from: String,
    pub to: String,
    pub source: FixSource,
    pub app: Option<String>,
}

/// What Home shows above the list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct HistorySummary {
    pub today: DayStats,
    pub counts: FilterCounts,
    /// Apps in the history, most used first.
    pub apps: Vec<AppCount>,
    /// Today's fixes, newest first, at most 20.
    pub fixed_today: Vec<TodayFix>,
}

const FIXED_TODAY_LIMIT: usize = 20;

fn fixed_today(conn: &Connection) -> Result<Vec<TodayFix>> {
    let mut stmt = conn.prepare(
        "SELECT id, timestamp, fixes, app_name FROM transcription_history
         WHERE fixes IS NOT NULL AND fixes != '[]'
           AND date(timestamp, 'unixepoch', 'localtime') = date('now', 'localtime')
         ORDER BY id DESC LIMIT 100",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut out = Vec::new();
    for (history_id, timestamp, json, app) in rows {
        let fixes: Vec<AppliedFix> = serde_json::from_str(&json).unwrap_or_default();
        // Within a dictation, the last fix is the newest.
        for fix in fixes.into_iter().rev() {
            out.push(TodayFix {
                history_id,
                timestamp,
                from: fix.from,
                to: fix.to,
                source: fix.source,
                app: app.clone(),
            });
            if out.len() == FIXED_TODAY_LIMIT {
                return Ok(out);
            }
        }
    }
    Ok(out)
}

/// An app dictated into, as Style › Edit apps lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeenApp {
    pub exe_path: String,
    /// The latest window title, which tells sites and Claude Code apart.
    pub title: String,
    pub words: i64,
    pub last_at: i64,
}

/// Apps and sites dictated into, most words first.
pub fn seen_apps(conn: &Connection, limit: usize) -> Result<Vec<SeenApp>> {
    let mut stmt = conn.prepare(
        "SELECT app_path, app_title, SUM(COALESCE(word_count, 0)) AS words, MAX(timestamp) AS last_at
         FROM transcription_history
         WHERE app_path IS NOT NULL AND app_path != ''
         GROUP BY app_exe, COALESCE(app_site, ''), app_name
         ORDER BY words DESC, last_at DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit as i64], |row| {
        Ok(SeenApp {
            exe_path: row.get(0)?,
            title: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            words: row.get(2)?,
            last_at: row.get(3)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn summary(conn: &Connection) -> Result<HistorySummary> {
    let today = conn
        .query_row(
            "SELECT d.day, COALESCE(s.dictations, 0), COALESCE(s.words, 0), COALESCE(s.speech_ms, 0)
             FROM (SELECT date('now', 'localtime') AS day) d
             LEFT JOIN daily_stats s ON s.day = d.day",
            [],
            |row| {
                Ok(DayStats {
                    day: row.get(0)?,
                    dictations: row.get(1)?,
                    words: row.get(2)?,
                    speech_ms: row.get(3)?,
                })
            },
        )
        .optional()?
        .unwrap_or_default();
    let count = |filter: &str| -> Result<i64> {
        Ok(conn.query_row(
            &format!("SELECT COUNT(*) FROM transcription_history WHERE {filter}"),
            [],
            |row| row.get(0),
        )?)
    };
    let counts = FilterCounts {
        all: count("1")?,
        starred: count(FILTER_SQL[0].1)?,
        fixed: count(FILTER_SQL[1].1)?,
        transformed: count(FILTER_SQL[2].1)?,
    };
    let mut stmt = conn.prepare(
        "SELECT app_name, MAX(app_path), COUNT(*) FROM transcription_history
         WHERE app_name IS NOT NULL AND app_name != ''
         GROUP BY app_name ORDER BY COUNT(*) DESC, app_name",
    )?;
    let apps = stmt
        .query_map([], |row| {
            Ok(AppCount {
                name: row.get(0)?,
                path: row.get(1)?,
                count: row.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(HistorySummary {
        today,
        counts,
        apps,
        fixed_today: fixed_today(conn)?,
    })
}

/// The span Insights covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum InsightsRange {
    Week,
    Month,
    All,
}

impl InsightsRange {
    /// The first day in the range, as an SQL date expression.
    fn since(self) -> &'static str {
        match self {
            InsightsRange::Week => "date('now', 'localtime', '-6 days')",
            InsightsRange::Month => "date('now', 'localtime', '-29 days')",
            InsightsRange::All => "'0000-00-00'",
        }
    }
}

/// One day's totals, for the charts.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct InsightsDay {
    /// Local date, YYYY-MM-DD.
    pub day: String,
    pub dictations: i64,
    pub words: i64,
    pub speech_ms: i64,
    pub fixes: i64,
    pub transformed: i64,
    pub snippets: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct InsightsApp {
    pub name: String,
    /// An executable to take the app's icon from.
    pub path: Option<String>,
    pub category: Option<String>,
    pub words: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct InsightsCount {
    pub text: String,
    pub count: i64,
}

/// A Dictionary word and how often Legato has fixed it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct InsightsFixedWord {
    pub id: i64,
    pub text: String,
    pub hits: i64,
    /// Learned from a fix or an edit rather than added by hand.
    pub learned: bool,
}

/// A speech model's use: dictations and words in the range, the rest since
/// it was first used.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct InsightsModel {
    pub model: String,
    pub dictations: i64,
    pub words: i64,
    pub all_words: i64,
    pub pastes: i64,
    pub paste_ms: i64,
    pub fixes: i64,
    /// Local date, YYYY-MM-DD.
    pub last_day: String,
    /// Dictations in the range with a timing, how long the model took on
    /// them in all once you stopped speaking, and how many ran on the GPU.
    pub timed: i64,
    pub transcribe_ms: i64,
    pub on_gpu: i64,
}

/// Everything Insights shows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Insights {
    /// Every day with dictations, oldest first, for totals, the activity map
    /// and speed by week.
    pub days: Vec<InsightsDay>,
    /// The day fillers, snippets and dictations by hour started being
    /// counted; earlier days don't have them.
    pub counted_since: Option<String>,
    pub apps: Vec<InsightsApp>,
    /// Dictations in each hour of the day, midnight first.
    pub hours: Vec<i64>,
    pub fillers: Vec<InsightsCount>,
    pub transforms: i64,
    /// The Dictionary words fixed most, all time.
    pub fixed_words: Vec<InsightsFixedWord>,
    /// Dictionary words fixed at least once, and their fixes, all time.
    pub fixed_word_count: i64,
    pub fixed_word_hits: i64,
    pub models: Vec<InsightsModel>,
}

const FIXED_WORDS_SHOWN: i64 = 8;

pub fn insights(conn: &Connection, range: InsightsRange) -> Result<Insights> {
    let since = range.since();
    let days = conn
        .prepare(
            "SELECT day, dictations, words, speech_ms, fixes, transformed, snippets
             FROM daily_stats WHERE dictations > 0 ORDER BY day",
        )?
        .query_map([], |row| {
            Ok(InsightsDay {
                day: row.get(0)?,
                dictations: row.get(1)?,
                words: row.get(2)?,
                speech_ms: row.get(3)?,
                fixes: row.get(4)?,
                transformed: row.get(5)?,
                snippets: row.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let counted_since = conn
        .query_row(
            "SELECT value FROM insights_meta WHERE key = 'counted_since'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let apps = conn
        .prepare(&format!(
            "SELECT a.app,
                    COALESCE(
                        (SELECT p.app_path FROM daily_app_stats p
                         WHERE p.app = a.app AND p.app_path IS NOT NULL
                         ORDER BY p.day DESC LIMIT 1),
                        (SELECT h.app_path FROM transcription_history h
                         WHERE h.app_name = a.app AND h.app_path IS NOT NULL
                         ORDER BY h.id DESC LIMIT 1)),
                    (SELECT c.category FROM daily_app_stats c
                     WHERE c.app = a.app AND c.category IS NOT NULL
                     ORDER BY c.day DESC LIMIT 1),
                    SUM(a.words) AS words
             FROM daily_app_stats a WHERE a.day >= {since}
             GROUP BY a.app HAVING words > 0 ORDER BY words DESC, a.app"
        ))?
        .query_map([], |row| {
            Ok(InsightsApp {
                name: row.get(0)?,
                path: row.get(1)?,
                category: row.get(2)?,
                words: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut hours = vec![0i64; 24];
    let mut stmt = conn.prepare(&format!(
        "SELECT hour, SUM(dictations) FROM daily_hour_stats WHERE day >= {since} GROUP BY hour"
    ))?;
    for row in stmt.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))? {
        let (hour, count) = row?;
        if let Some(slot) = usize::try_from(hour).ok().and_then(|h| hours.get_mut(h)) {
            *slot = count.max(0);
        }
    }
    let fillers = conn
        .prepare(&format!(
            "SELECT word, SUM(count) AS n FROM daily_filler_stats WHERE day >= {since}
             GROUP BY word HAVING n > 0 ORDER BY n DESC, word"
        ))?
        .query_map([], |row| {
            Ok(InsightsCount {
                text: row.get(0)?,
                count: row.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let transforms = conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM transform_run
             WHERE date(at, 'unixepoch', 'localtime') >= {since} AND result != 'failed'"
        ),
        [],
        |row| row.get(0),
    )?;
    let fixed_words = conn
        .prepare(
            "SELECT id, text, hits, source IN ('fixed', 'edited', 'spelled') FROM dictionary_entry
             WHERE hits > 0 ORDER BY hits DESC, text LIMIT ?1",
        )?
        .query_map([FIXED_WORDS_SHOWN], |row| {
            Ok(InsightsFixedWord {
                id: row.get(0)?,
                text: row.get(1)?,
                hits: row.get(2)?,
                learned: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let (fixed_word_count, fixed_word_hits) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(hits), 0) FROM dictionary_entry WHERE hits > 0",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let models = conn
        .prepare(&format!(
            "SELECT model,
                    SUM(CASE WHEN day >= {since} THEN dictations ELSE 0 END),
                    SUM(CASE WHEN day >= {since} THEN words ELSE 0 END),
                    SUM(words), SUM(pastes), SUM(paste_ms), SUM(fixes),
                    MAX(CASE WHEN dictations > 0 THEN day END) AS last_day,
                    SUM(CASE WHEN day >= {since} THEN timed ELSE 0 END),
                    SUM(CASE WHEN day >= {since} THEN transcribe_ms ELSE 0 END),
                    SUM(CASE WHEN day >= {since} THEN on_gpu ELSE 0 END)
             FROM daily_model_stats GROUP BY model HAVING last_day IS NOT NULL
             ORDER BY 3 DESC, last_day DESC"
        ))?
        .query_map([], |row| {
            Ok(InsightsModel {
                model: row.get(0)?,
                dictations: row.get(1)?,
                words: row.get(2)?,
                all_words: row.get(3)?,
                pastes: row.get(4)?,
                paste_ms: row.get(5)?,
                fixes: row.get(6)?,
                last_day: row.get(7)?,
                timed: row.get(8)?,
                transcribe_ms: row.get(9)?,
                on_gpu: row.get(10)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(Insights {
        days,
        counted_since,
        apps,
        hours,
        fillers,
        transforms,
        fixed_words,
        fixed_word_count,
        fixed_word_hits,
        models,
    })
}

/// Speaking time of a saved recording: 16 kHz, 16-bit mono after the
/// 44-byte header.
fn wav_speech_ms(path: &Path) -> Option<i64> {
    let len = std::fs::metadata(path).ok()?.len();
    Some((len.saturating_sub(44) / 32) as i64)
}

/// Fills the columns and tables added in version 7 from the rows that were
/// there before: word counts, speaking time from the recordings, and the
/// daily totals. Runs once, when the database first reaches version 7.
pub fn backfill(conn: &mut Connection, recordings_dir: &Path) -> Result<()> {
    let tx = conn.transaction()?;
    let rows: Vec<(i64, String, String, Option<String>)> = {
        let mut stmt = tx.prepare(
            "SELECT id, file_name, transcription_text, post_processed_text
             FROM transcription_history WHERE word_count IS NULL OR speech_ms IS NULL",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows
    };
    for (id, file_name, text, processed) in rows {
        let words = word_count(&final_text(&text, processed.as_deref()));
        let speech = wav_speech_ms(&recordings_dir.join(&file_name));
        let status = text.is_empty().then_some(DictationStatus::Failed.as_str());
        tx.execute(
            "UPDATE transcription_history
             SET word_count = ?1, speech_ms = COALESCE(speech_ms, ?2), status = COALESCE(status, ?3)
             WHERE id = ?4",
            params![words, speech, status, id],
        )?;
    }
    tx.execute("DELETE FROM daily_stats", [])?;
    tx.execute(
        "INSERT INTO daily_stats (day, dictations, words, speech_ms, transformed)
         SELECT date(timestamp, 'unixepoch', 'localtime'), COUNT(*),
                SUM(COALESCE(word_count, 0)), SUM(COALESCE(speech_ms, 0)),
                SUM(post_processed_text IS NOT NULL)
         FROM transcription_history WHERE transcription_text != ''
         GROUP BY 1",
        [],
    )?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite_migration::Migrations;

    fn db() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        Migrations::new(MIGRATIONS.to_vec())
            .to_latest(&mut conn)
            .unwrap();
        conn
    }

    fn now() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }

    fn entry(text: &str, app: Option<&str>) -> NewEntry {
        NewEntry {
            file_name: "legato-1.wav".into(),
            timestamp: now(),
            title: "t".into(),
            transcription_text: text.into(),
            app: app.map(|name| AppFields {
                exe: "slack.exe".into(),
                path: "C:\\Slack\\slack.exe".into(),
                name: name.into(),
                title: format!("general - {name}"),
                site: None,
                category: "work".into(),
            }),
            speech_ms: Some(4000),
            model_id: Some("parakeet".into()),
            model_name: Some("Parakeet V3".into()),
            ..Default::default()
        }
    }

    #[test]
    fn insights_count_words_fillers_hours_and_models() {
        let mut conn = db();
        let mut first = entry("move the staging app to Dokploy", Some("Slack"));
        first.fillers = vec![("um".into(), 2), ("uh".into(), 1)];
        first.snippets = 1;
        first.fixes = vec![AppliedFix {
            from: "dock ploy".into(),
            to: "Dokploy".into(),
            source: FixSource::Heard,
        }];
        first.transcribe_ms = Some(300);
        first.on_gpu = Some(true);
        let first = insert(&mut conn, &first).unwrap();
        let mut second = entry("ship it today", Some("Slack"));
        second.fillers = vec![("um".into(), 1)];
        second.transcribe_ms = Some(500);
        second.on_gpu = Some(false);
        insert(&mut conn, &second).unwrap();
        // Saved before timings were kept: counted, but not timed.
        insert(&mut conn, &entry("old one", None)).unwrap();
        record_paste(
            &mut conn,
            first.id,
            &PasteRecord {
                status: DictationStatus::Pasted,
                error: None,
                paste_ms: Some(400),
                paste_app: None,
            },
        )
        .unwrap();

        let got = insights(&conn, InsightsRange::Week).unwrap();
        assert_eq!(got.days.len(), 1);
        let day = &got.days[0];
        assert_eq!(
            (day.dictations, day.words, day.fixes, day.snippets),
            (3, 11, 1, 1)
        );
        assert!(got.counted_since.is_some());
        assert_eq!(got.apps.len(), 1);
        assert_eq!(got.apps[0].words, 9);
        assert_eq!(got.apps[0].path.as_deref(), Some("C:\\Slack\\slack.exe"));
        assert_eq!(got.hours.iter().sum::<i64>(), 3);
        assert_eq!(
            got.fillers,
            vec![
                InsightsCount {
                    text: "um".into(),
                    count: 3
                },
                InsightsCount {
                    text: "uh".into(),
                    count: 1
                }
            ]
        );
        let model = &got.models[0];
        assert_eq!(
            (
                model.dictations,
                model.words,
                model.pastes,
                model.paste_ms,
                model.fixes
            ),
            (3, 11, 1, 400, 1)
        );
        assert_eq!(
            (model.timed, model.transcribe_ms, model.on_gpu),
            (2, 800, 1)
        );
    }

    #[test]
    fn the_day_filter_finds_that_days_dictations() {
        let mut conn = db();
        insert(&mut conn, &entry("today's words", None)).unwrap();
        let today: String = conn
            .query_row("SELECT date('now', 'localtime')", [], |r| r.get(0))
            .unwrap();
        let q = |day: &str| HistoryQuery {
            day: Some(day.into()),
            ..Default::default()
        };
        assert_eq!(query(&conn, &q(&today)).unwrap().entries.len(), 1);
        assert!(query(&conn, &q("2001-01-01")).unwrap().entries.is_empty());
    }

    #[test]
    fn migrations_are_valid() {
        Migrations::new(MIGRATIONS.to_vec()).validate().unwrap();
    }

    #[test]
    fn words_are_counted_across_scripts() {
        assert_eq!(word_count("move the staging app to Dokploy"), 6);
        assert_eq!(word_count("  hello,  world -- ok "), 3);
        assert_eq!(word_count("你好世界"), 4);
        assert_eq!(word_count(""), 0);
    }

    #[test]
    fn insert_counts_the_day_app_and_model() {
        let mut conn = db();
        insert(&mut conn, &entry("keep the Netbird route", Some("Slack"))).unwrap();
        insert(&mut conn, &entry("two words", Some("Slack"))).unwrap();
        let s = summary(&conn).unwrap();
        assert_eq!(s.today.dictations, 2);
        assert_eq!(s.today.words, 6);
        assert_eq!(s.today.speech_ms, 8000);
        assert_eq!(s.counts.all, 2);
        assert_eq!(s.apps.len(), 1);
        assert_eq!(s.apps[0].count, 2);
        let model: i64 = conn
            .query_row(
                "SELECT dictations FROM daily_model_stats WHERE model = 'parakeet'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(model, 2);
    }

    #[test]
    fn failed_dictations_are_not_counted() {
        let mut conn = db();
        let mut failed = entry("", None);
        failed.status = Some(DictationStatus::Failed);
        let saved = insert(&mut conn, &failed).unwrap();
        assert_eq!(saved.status, Some(DictationStatus::Failed));
        assert_eq!(summary(&conn).unwrap().today.dictations, 0);

        // Trying again counts it once, and clears the failure.
        let fixed = update_transcription(
            &mut conn,
            saved.id,
            "it worked",
            None,
            None,
            None,
            None,
            &[],
            None,
        )
        .unwrap();
        assert_eq!(fixed.status, None);
        assert_eq!(summary(&conn).unwrap().today.dictations, 1);
        assert_eq!(summary(&conn).unwrap().today.words, 2);
    }

    #[test]
    fn search_matches_prefixes_and_app_names() {
        let mut conn = db();
        insert(
            &mut conn,
            &entry("deploy to Dokploy tonight", Some("Slack")),
        )
        .unwrap();
        insert(&mut conn, &entry("lunch tomorrow", None)).unwrap();
        let find = |s: &str| {
            query(
                &conn,
                &HistoryQuery {
                    search: Some(s.into()),
                    ..Default::default()
                },
            )
            .unwrap()
            .entries
            .len()
        };
        assert_eq!(find("dok"), 1);
        assert_eq!(find("slack"), 1);
        assert_eq!(find("lunch tom"), 1);
        assert_eq!(find("lunch slack"), 0);
        assert_eq!(find("\"quoted"), 0);
    }

    #[test]
    fn search_follows_deletes_and_updates() {
        let mut conn = db();
        let a = insert(&mut conn, &entry("alpha", None)).unwrap();
        update_transcription(&mut conn, a.id, "beta", None, None, None, None, &[], None).unwrap();
        let count = |conn: &Connection, s: &str| {
            query(
                conn,
                &HistoryQuery {
                    search: Some(s.into()),
                    ..Default::default()
                },
            )
            .unwrap()
            .entries
            .len()
        };
        assert_eq!(count(&conn, "alpha"), 0);
        assert_eq!(count(&conn, "beta"), 1);
        conn.execute("DELETE FROM transcription_history WHERE id = ?1", [a.id])
            .unwrap();
        assert_eq!(count(&conn, "beta"), 0);
        // Deleting keeps the day's totals.
        assert_eq!(summary(&conn).unwrap().today.dictations, 1);
    }

    #[test]
    fn insights_keep_an_apps_icon_after_its_dictations_go() {
        let mut conn = db();
        insert(&mut conn, &entry("ship it today", Some("Slack"))).unwrap();
        conn.execute("DELETE FROM transcription_history", [])
            .unwrap();
        let got = insights(&conn, InsightsRange::Week).unwrap();
        assert_eq!(got.apps[0].path.as_deref(), Some("C:\\Slack\\slack.exe"));
    }

    #[test]
    fn filters_and_pages() {
        let mut conn = db();
        for i in 0..5 {
            insert(&mut conn, &entry(&format!("entry {i}"), Some("Slack"))).unwrap();
        }
        let mut transformed = entry("raw", None);
        transformed.post_processed_text = Some("Polished.".into());
        insert(&mut conn, &transformed).unwrap();
        conn.execute(
            "UPDATE transcription_history SET saved = 1 WHERE id = 1",
            [],
        )
        .unwrap();

        let run = |q: HistoryQuery| query(&conn, &q).unwrap();
        assert_eq!(
            run(HistoryQuery {
                filter: HistoryFilter::Starred,
                ..Default::default()
            })
            .entries
            .len(),
            1
        );
        assert_eq!(
            run(HistoryQuery {
                filter: HistoryFilter::Transformed,
                ..Default::default()
            })
            .entries[0]
                .word_count,
            Some(1)
        );
        assert_eq!(
            run(HistoryQuery {
                app: Some("Slack".into()),
                ..Default::default()
            })
            .entries
            .len(),
            5
        );
        let first = run(HistoryQuery {
            limit: Some(4),
            ..Default::default()
        });
        assert!(first.has_more);
        let rest = run(HistoryQuery {
            limit: Some(4),
            cursor: first.entries.last().map(|e| e.id),
            ..Default::default()
        });
        assert!(!rest.has_more);
        assert_eq!(first.entries.len() + rest.entries.len(), 6);
    }

    #[test]
    fn paste_is_recorded_once_per_model() {
        let mut conn = db();
        let saved = insert(&mut conn, &entry("hello there", Some("Slack"))).unwrap();
        let pasted = record_paste(
            &mut conn,
            saved.id,
            &PasteRecord {
                status: DictationStatus::Pasted,
                error: None,
                paste_ms: Some(640),
                paste_app: Some(("Teams".into(), "Chat".into())),
            },
        )
        .unwrap();
        assert_eq!(pasted.status, Some(DictationStatus::Pasted));
        assert_eq!(pasted.paste_app_name.as_deref(), Some("Teams"));
        let (pastes, ms): (i64, i64) = conn
            .query_row(
                "SELECT pastes, paste_ms FROM daily_model_stats WHERE model = 'parakeet'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((pastes, ms), (1, 640));
    }

    #[test]
    fn backfill_seeds_old_rows() {
        let mut conn = Connection::open_in_memory().unwrap();
        // A database from before version 5, with one dictation and one failure.
        Migrations::new(MIGRATIONS[..4].to_vec())
            .to_latest(&mut conn)
            .unwrap();
        let dir = std::env::temp_dir().join(format!("legato-backfill-{}", now()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.wav"), vec![0u8; 44 + 32 * 1500]).unwrap();
        conn.execute(
            "INSERT INTO transcription_history (file_name, timestamp, title, transcription_text)
             VALUES ('a.wav', ?1, 't', 'three old words'), ('b.wav', ?1, 't', '')",
            [now()],
        )
        .unwrap();
        Migrations::new(MIGRATIONS.to_vec())
            .to_latest(&mut conn)
            .unwrap();
        backfill(&mut conn, &dir).unwrap();
        let s = summary(&conn).unwrap();
        assert_eq!(s.today.dictations, 1);
        assert_eq!(s.today.words, 3);
        assert_eq!(s.today.speech_ms, 1500);
        let failed = get(&conn, 2).unwrap().unwrap();
        assert_eq!(failed.status, Some(DictationStatus::Failed));
        // Old rows are searchable too.
        assert_eq!(
            query(
                &conn,
                &HistoryQuery {
                    search: Some("old".into()),
                    ..Default::default()
                }
            )
            .unwrap()
            .entries
            .len(),
            1
        );
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn fix_a_word_edits_both_versions_and_can_be_undone() {
        let mut conn = db();
        let mut new = entry("restart doc ploy on the arm box", Some("Slack"));
        new.post_processed_text = Some("Restart doc ploy on the ARM box.".into());
        let saved = insert(&mut conn, &new).unwrap();
        let fix = AppliedFix {
            from: "doc ploy".into(),
            to: "Dokploy".into(),
            source: FixSource::Manual,
        };
        // A stale view of the text is refused.
        assert!(fix_row(&mut conn, saved.id, "old text", "new", &fix).is_err());
        let (snap, after) = fix_row(
            &mut conn,
            saved.id,
            "Restart doc ploy on the ARM box.",
            "Restart Dokploy on the ARM box.",
            &fix,
        )
        .unwrap();
        assert_eq!(after.transcription_text, "restart Dokploy on the arm box");
        assert_eq!(
            after.post_processed_text.as_deref(),
            Some("Restart Dokploy on the ARM box.")
        );
        assert_eq!(after.fixes, vec![fix.clone()]);
        assert_eq!(summary(&conn).unwrap().counts.fixed, 1);
        assert_eq!(summary(&conn).unwrap().fixed_today.len(), 1);

        let restored = restore_texts(&mut conn, &[snap]).unwrap();
        assert_eq!(
            restored[0].transcription_text,
            "restart doc ploy on the arm box"
        );
        assert!(restored[0].fixes.is_empty());
        assert_eq!(summary(&conn).unwrap().counts.fixed, 0);
        assert_eq!(summary(&conn).unwrap().today.words, 7);
    }

    #[test]
    fn other_transcripts_with_the_same_miss_are_found_and_fixed() {
        let mut conn = db();
        let a = insert(&mut conn, &entry("ask carol about it", None)).unwrap();
        let b = insert(&mut conn, &entry("Carol said yes", None)).unwrap();
        let c = insert(&mut conn, &entry("christmas carols", None)).unwrap();
        let d = insert(&mut conn, &entry("Karol is here", None)).unwrap();
        assert_eq!(
            rows_with(&conn, "carol", "Karol", a.id).unwrap(),
            vec![b.id]
        );
        let fix = AppliedFix {
            from: "carol".into(),
            to: "Karol".into(),
            source: FixSource::Manual,
        };
        let changed = fix_rows(&mut conn, &[b.id, c.id, d.id], &fix).unwrap();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].1.transcription_text, "Karol said yes");
        assert_eq!(
            get(&conn, c.id).unwrap().unwrap().transcription_text,
            "christmas carols"
        );
    }

    #[test]
    fn saved_fixes_are_counted_and_listed_for_today() {
        let mut conn = db();
        let mut new = entry("deploy to Dokploy", Some("Cursor"));
        new.fixes = vec![AppliedFix {
            from: "doc ploy".into(),
            to: "Dokploy".into(),
            source: FixSource::Heard,
        }];
        let saved = insert(&mut conn, &new).unwrap();
        assert_eq!(saved.fixes.len(), 1);
        let s = summary(&conn).unwrap();
        assert_eq!(s.counts.fixed, 1);
        assert_eq!(s.fixed_today[0].to, "Dokploy");
        assert_eq!(s.fixed_today[0].app.as_deref(), Some("Cursor"));
        // Fixes written before they had a source read as close matches.
        let old: Vec<AppliedFix> = serde_json::from_str(r#"[{"from":"a","to":"b"}]"#).unwrap();
        assert_eq!(old[0].source, FixSource::Match);
    }
}
