//! The history database: its schema, the queries Home runs and the daily
//! statistics. Plain rusqlite with no app state, so it can be tested on its
//! own; `HistoryManager` wraps it with files, settings and events.

use anyhow::{anyhow, Result};
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
];

/// The schema version that added the columns and tables `backfill` fills in.
pub const BACKFILL_VERSION: i32 = 7;

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

/// A Dictionary fix made on one dictation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct AppliedFix {
    pub from: String,
    pub to: String,
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
}

pub const COLUMNS: &str = "id, file_name, timestamp, saved, title, transcription_text,
    post_processed_text, post_process_prompt, post_process_requested, app_exe, app_path,
    app_name, app_title, app_site, category, paste_app_name, speech_ms, word_count,
    model_id, model_name, transform, fixes, paste_ms, status, error";

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
                "INSERT INTO daily_app_stats (day, app, category, dictations, words, speech_ms)
                 VALUES ({DAY_OF}, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(day, app) DO UPDATE SET
                    category = excluded.category,
                    dictations = dictations + excluded.dictations,
                    words = words + excluded.words,
                    speech_ms = speech_ms + excluded.speech_ms"
            ),
            params![entry.timestamp, app, entry.category, sign, words, speech],
        )?;
    }
    if let Some(model) = entry.model_id.as_deref().filter(|m| !m.is_empty()) {
        conn.execute(
            &format!(
                "INSERT INTO daily_model_stats (day, model, dictations, words, speech_ms)
                 VALUES ({DAY_OF}, ?2, ?3, ?4, ?5)
                 ON CONFLICT(day, model) DO UPDATE SET
                    dictations = dictations + excluded.dictations,
                    words = words + excluded.words,
                    speech_ms = speech_ms + excluded.speech_ms"
            ),
            params![entry.timestamp, model, sign, words, speech],
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
            transform, status
         ) VALUES (?1, ?2, 0, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
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
        ],
    )?;
    let entry = get(&tx, tx.last_insert_rowid())?.ok_or_else(|| anyhow!("Saved row is missing"))?;
    add_to_stats(&tx, &entry, 1)?;
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
pub fn update_transcription(
    conn: &mut Connection,
    id: i64,
    transcription_text: &str,
    post_processed_text: Option<&str>,
    post_process_prompt: Option<&str>,
    transform: Option<&str>,
    model: Option<(&str, &str)>,
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
             model_id = COALESCE(?7, model_id), model_name = COALESCE(?8, model_name)
         WHERE id = ?9",
        params![
            transcription_text,
            post_processed_text,
            post_process_prompt,
            transform,
            words,
            status,
            model.map(|(id, _)| id),
            model.map(|(_, name)| name),
            id
        ],
    )?;
    let after = get(&tx, id)?.ok_or_else(|| anyhow!("History entry {} not found", id))?;
    add_to_stats(&tx, &after, 1)?;
    tx.commit()?;
    Ok(after)
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

/// What Home shows above the list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct HistorySummary {
    pub today: DayStats,
    pub counts: FilterCounts,
    /// Apps in the history, most used first.
    pub apps: Vec<AppCount>,
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
        let fixed =
            update_transcription(&mut conn, saved.id, "it worked", None, None, None, None).unwrap();
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
        update_transcription(&mut conn, a.id, "beta", None, None, None, None).unwrap();
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
}
