//! The Dictionary's tables: words spelled the user's way, the ways the model
//! mishears each one, and how often each fixed a transcript. Plain rusqlite
//! with no app state, like `history_store`; `DictionaryManager` wraps it with
//! events and the snapshot the pipeline reads.

use crate::audio_toolkit::text::{CompiledDictionary, DictionaryWord};
use crate::learn_rules::{spelled_differently, Said, Spelling};
use anyhow::{anyhow, Result};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::{HashMap, HashSet};

/// Words the speech model gets before it listens.
pub const HINT_LIMIT: usize = 40;
/// Words that always go into the hint.
pub const PIN_LIMIT: usize = 20;
pub const MAX_WORD_CHARS: usize = 60;

/// Where a word came from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum DictionarySource {
    /// Added on the Dictionary page, or moved from the old custom words.
    #[default]
    Manual,
    /// Learned from Fix a word on Home.
    Fixed,
    /// Selected in another app and added with the shortcut.
    Selection,
    Imported,
    /// Corrected in another app right after Legato pasted it.
    Edited,
    /// Written more than one way by the model, and added from Needs review
    /// or Insights.
    Spelled,
}

impl DictionarySource {
    fn as_str(self) -> &'static str {
        match self {
            DictionarySource::Manual => "manual",
            DictionarySource::Fixed => "fixed",
            DictionarySource::Selection => "selection",
            DictionarySource::Imported => "imported",
            DictionarySource::Edited => "edited",
            DictionarySource::Spelled => "spelled",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "fixed" => DictionarySource::Fixed,
            "selection" => DictionarySource::Selection,
            "imported" => DictionarySource::Imported,
            "edited" => DictionarySource::Edited,
            "spelled" => DictionarySource::Spelled,
            _ => DictionarySource::Manual,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct DictionaryEntry {
    pub id: i64,
    pub text: String,
    /// Ways the model mishears the word.
    pub heard: Vec<String>,
    pub source: DictionarySource,
    pub pinned: bool,
    pub case_sensitive: bool,
    pub added_at: i64,
    pub last_fix_at: Option<i64>,
    /// The app the word was last fixed in.
    pub last_app: Option<String>,
    pub last_app_path: Option<String>,
    /// Transcripts the word fixed.
    pub hits: i64,
}

/// A word as the editor saves it, and as files import it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct EntryInput {
    pub text: String,
    #[serde(default)]
    pub heard: Vec<String>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub case_sensitive: bool,
}

/// An app a fix happened in: its name, and an executable for its icon.
pub type FixApp<'a> = Option<(&'a str, Option<&'a str>)>;

const COLUMNS: &str = "id, text, source, pinned, case_sensitive, added_at, last_fix_at, last_app,
    last_app_path, hits";

fn map_entry(row: &Row<'_>) -> rusqlite::Result<DictionaryEntry> {
    let source: String = row.get("source")?;
    Ok(DictionaryEntry {
        id: row.get("id")?,
        text: row.get("text")?,
        heard: Vec::new(),
        source: DictionarySource::parse(&source),
        pinned: row.get("pinned")?,
        case_sensitive: row.get("case_sensitive")?,
        added_at: row.get("added_at")?,
        last_fix_at: row.get("last_fix_at")?,
        last_app: row.get("last_app")?,
        last_app_path: row.get("last_app_path")?,
        hits: row.get("hits")?,
    })
}

/// A word with single spaces and no padding.
pub fn clean_word(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A heard form is matched as whole words, so it starts and ends with a
/// letter or digit.
pub fn clean_heard(text: &str) -> String {
    clean_word(text.trim_matches(|c: char| !c.is_alphanumeric()))
}

fn heard_of(conn: &Connection, id: i64) -> Result<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT heard FROM dictionary_heard WHERE entry_id = ?1 ORDER BY rowid")?;
    let heard = stmt
        .query_map(params![id], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(heard)
}

pub fn get(conn: &Connection, id: i64) -> Result<Option<DictionaryEntry>> {
    let entry = conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM dictionary_entry WHERE id = ?1"),
            params![id],
            map_entry,
        )
        .optional()?;
    match entry {
        Some(mut entry) => {
            entry.heard = heard_of(conn, entry.id)?;
            Ok(Some(entry))
        }
        None => Ok(None),
    }
}

/// The entry spelled `text`, in any case.
pub fn find(conn: &Connection, text: &str) -> Result<Option<DictionaryEntry>> {
    let id: Option<i64> = conn
        .query_row(
            "SELECT id FROM dictionary_entry WHERE text = ?1 COLLATE NOCASE",
            params![clean_word(text)],
            |row| row.get(0),
        )
        .optional()?;
    match id {
        Some(id) => get(conn, id),
        None => Ok(None),
    }
}

/// Every word, oldest first.
pub fn list(conn: &Connection) -> Result<Vec<DictionaryEntry>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM dictionary_entry ORDER BY id"
    ))?;
    let mut entries = stmt
        .query_map([], map_entry)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut heard: HashMap<i64, Vec<String>> = HashMap::new();
    let mut stmt = conn.prepare("SELECT entry_id, heard FROM dictionary_heard ORDER BY rowid")?;
    for row in stmt.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get(1)?)))? {
        let (id, form) = row?;
        heard.entry(id).or_default().push(form);
    }
    for entry in &mut entries {
        entry.heard = heard.remove(&entry.id).unwrap_or_default();
    }
    Ok(entries)
}

fn pinned_count(conn: &Connection, except: Option<i64>) -> Result<usize> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM dictionary_entry WHERE pinned = 1 AND id != ?1",
        params![except.unwrap_or(-1)],
        |row| row.get(0),
    )?;
    Ok(count as usize)
}

/// Checks a word before it is saved, returning it cleaned.
fn validate(conn: &Connection, input: &EntryInput, id: Option<i64>) -> Result<String> {
    let text = clean_word(&input.text);
    if text.is_empty() {
        return Err(anyhow!("Type the word the way it should be spelled"));
    }
    if text.chars().count() > MAX_WORD_CHARS {
        return Err(anyhow!("Words can be up to {MAX_WORD_CHARS} characters"));
    }
    if let Some(other) = find(conn, &text)? {
        if Some(other.id) != id {
            return Err(anyhow!("\"{}\" is already in your Dictionary", other.text));
        }
    }
    if input.pinned && pinned_count(conn, id)? >= PIN_LIMIT {
        let already = match id {
            Some(id) => get(conn, id)?.is_some_and(|e| e.pinned),
            None => false,
        };
        if !already {
            return Err(anyhow!("You can pin up to {PIN_LIMIT} words"));
        }
    }
    Ok(text)
}

/// Adds heard forms to a word. A form another word already had moves to
/// this one; forms that are the word itself, or another word's spelling,
/// are skipped. Returns the forms added.
fn add_heard(conn: &Connection, id: i64, text: &str, heard: &[String]) -> Result<Vec<String>> {
    let mut added: Vec<String> = Vec::new();
    for form in heard.iter().map(|h| clean_heard(h)) {
        if form.is_empty()
            || form.to_lowercase() == text.to_lowercase()
            || added
                .iter()
                .any(|a| a.to_lowercase() == form.to_lowercase())
        {
            continue;
        }
        let is_a_word: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM dictionary_entry WHERE text = ?1 COLLATE NOCASE AND id != ?2",
            params![form, id],
            |row| row.get(0),
        )?;
        if is_a_word {
            continue;
        }
        conn.execute(
            "DELETE FROM dictionary_heard WHERE heard = ?1 COLLATE NOCASE",
            params![form],
        )?;
        conn.execute(
            "INSERT INTO dictionary_heard (entry_id, heard) VALUES (?1, ?2)",
            params![id, form],
        )?;
        added.push(form);
    }
    Ok(added)
}

pub fn add(
    conn: &mut Connection,
    input: &EntryInput,
    source: DictionarySource,
    now: i64,
) -> Result<DictionaryEntry> {
    let tx = conn.transaction()?;
    let text = validate(&tx, input, None)?;
    tx.execute(
        "INSERT INTO dictionary_entry (text, source, pinned, case_sensitive, added_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            text,
            source.as_str(),
            input.pinned,
            input.case_sensitive,
            now
        ],
    )?;
    let id = tx.last_insert_rowid();
    // A new word that was someone else's heard form stops being one.
    tx.execute(
        "DELETE FROM dictionary_heard WHERE heard = ?1 COLLATE NOCASE",
        params![text],
    )?;
    add_heard(&tx, id, &text, &input.heard)?;
    let entry = get(&tx, id)?.ok_or_else(|| anyhow!("Saved word is missing"))?;
    tx.commit()?;
    Ok(entry)
}

pub fn update(conn: &mut Connection, id: i64, input: &EntryInput) -> Result<DictionaryEntry> {
    let tx = conn.transaction()?;
    get(&tx, id)?.ok_or_else(|| anyhow!("That word is no longer in your Dictionary"))?;
    let text = validate(&tx, input, Some(id))?;
    tx.execute(
        "UPDATE dictionary_entry SET text = ?1, pinned = ?2, case_sensitive = ?3 WHERE id = ?4",
        params![text, input.pinned, input.case_sensitive, id],
    )?;
    tx.execute(
        "DELETE FROM dictionary_heard WHERE entry_id = ?1 OR heard = ?2 COLLATE NOCASE",
        params![id, text],
    )?;
    add_heard(&tx, id, &text, &input.heard)?;
    let entry = get(&tx, id)?.ok_or_else(|| anyhow!("Saved word is missing"))?;
    tx.commit()?;
    Ok(entry)
}

pub fn set_pinned(conn: &mut Connection, id: i64, pinned: bool) -> Result<DictionaryEntry> {
    let entry =
        get(conn, id)?.ok_or_else(|| anyhow!("That word is no longer in your Dictionary"))?;
    if pinned && !entry.pinned && pinned_count(conn, None)? >= PIN_LIMIT {
        return Err(anyhow!("You can pin up to {PIN_LIMIT} words"));
    }
    conn.execute(
        "UPDATE dictionary_entry SET pinned = ?1 WHERE id = ?2",
        params![pinned, id],
    )?;
    get(conn, id)?.ok_or_else(|| anyhow!("Saved word is missing"))
}

/// Deletes words, returning them as they were so they can be restored.
pub fn delete(conn: &mut Connection, ids: &[i64]) -> Result<Vec<DictionaryEntry>> {
    let tx = conn.transaction()?;
    let mut removed = Vec::new();
    for &id in ids {
        if let Some(entry) = get(&tx, id)? {
            tx.execute(
                "DELETE FROM dictionary_heard WHERE entry_id = ?1",
                params![id],
            )?;
            tx.execute("DELETE FROM dictionary_entry WHERE id = ?1", params![id])?;
            removed.push(entry);
        }
    }
    tx.commit()?;
    Ok(removed)
}

/// Puts words back exactly as they were (Undo).
pub fn restore(conn: &mut Connection, entries: &[DictionaryEntry]) -> Result<()> {
    let tx = conn.transaction()?;
    for e in entries {
        // A word added since with the same spelling gives way to the original.
        tx.execute(
            "DELETE FROM dictionary_entry WHERE text = ?1 COLLATE NOCASE AND id != ?2",
            params![e.text, e.id],
        )?;
        tx.execute(
            "INSERT OR REPLACE INTO dictionary_entry (id, text, source, pinned, case_sensitive,
                added_at, last_fix_at, last_app, last_app_path, hits)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                e.id,
                e.text,
                e.source.as_str(),
                e.pinned,
                e.case_sensitive,
                e.added_at,
                e.last_fix_at,
                e.last_app,
                e.last_app_path,
                e.hits
            ],
        )?;
        tx.execute(
            "DELETE FROM dictionary_heard WHERE entry_id = ?1",
            params![e.id],
        )?;
        add_heard(&tx, e.id, &e.text, &e.heard)?;
    }
    // Heard forms whose word is gone.
    tx.execute(
        "DELETE FROM dictionary_heard WHERE entry_id NOT IN (SELECT id FROM dictionary_entry)",
        [],
    )?;
    tx.commit()?;
    Ok(())
}

/// Counts a fix for each word in `words`, in the app it happened in.
pub fn record_fixes(
    conn: &mut Connection,
    words: &[&str],
    app: FixApp<'_>,
    now: i64,
) -> Result<()> {
    let tx = conn.transaction()?;
    for word in words {
        tx.execute(
            "UPDATE dictionary_entry
             SET hits = hits + 1, last_fix_at = ?1,
                 last_app = COALESCE(?2, last_app), last_app_path = COALESCE(?3, last_app_path)
             WHERE text = ?4 COLLATE NOCASE",
            params![
                now,
                app.map(|(name, _)| name),
                app.and_then(|(_, path)| path),
                word
            ],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// A word learned from an edit, waiting under Needs review.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct PendingWord {
    pub id: i64,
    pub word: String,
    /// What Legato had pasted.
    pub heard: String,
    /// The text around the correction, about 60 characters.
    pub context: String,
    pub app: Option<String>,
    pub app_path: Option<String>,
    pub at: i64,
}

fn map_pending(row: &Row<'_>) -> rusqlite::Result<PendingWord> {
    Ok(PendingWord {
        id: row.get("id")?,
        word: row.get("word")?,
        heard: row.get("heard")?,
        context: row.get("context")?,
        app: row.get("app")?,
        app_path: row.get("app_path")?,
        at: row.get("at")?,
    })
}

/// Whether a correction is already covered: the word is never to be
/// proposed, or the Dictionary already turns this heard form into it.
pub fn is_known(conn: &Connection, word: &str, heard: &str) -> Result<bool> {
    let never: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM dictionary_never WHERE word = ?1 COLLATE NOCASE",
        params![clean_word(word)],
        |row| row.get(0),
    )?;
    if never {
        return Ok(true);
    }
    let Some(entry) = find(conn, word)? else {
        return Ok(false);
    };
    Ok(entry.text == clean_word(word)
        && (entry.heard.iter().any(|h| h.eq_ignore_ascii_case(heard))
            || heard.eq_ignore_ascii_case(&entry.text)))
}

/// Puts a correction under Needs review, or brings one already there back to
/// the top. None when it's already covered.
pub fn add_pending(
    conn: &Connection,
    word: &str,
    heard: &str,
    context: &str,
    app: FixApp<'_>,
    now: i64,
) -> Result<Option<PendingWord>> {
    let (word, heard) = (clean_word(word), clean_heard(heard));
    if word.is_empty() || heard.is_empty() || is_known(conn, &word, &heard)? {
        return Ok(None);
    }
    conn.execute(
        "INSERT INTO dictionary_pending (word, heard, context, app, app_path, at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT (word COLLATE NOCASE, heard COLLATE NOCASE)
         DO UPDATE SET context = excluded.context, app = excluded.app,
            app_path = excluded.app_path, at = excluded.at",
        params![
            word,
            heard,
            context,
            app.map(|(name, _)| name),
            app.and_then(|(_, path)| path),
            now
        ],
    )?;
    Ok(conn
        .query_row(
            "SELECT * FROM dictionary_pending
             WHERE word = ?1 COLLATE NOCASE AND heard = ?2 COLLATE NOCASE",
            params![word, heard],
            map_pending,
        )
        .optional()?)
}

/// Words under Needs review, newest first.
pub fn pending(conn: &Connection) -> Result<Vec<PendingWord>> {
    let mut stmt = conn.prepare("SELECT * FROM dictionary_pending ORDER BY at DESC, id DESC")?;
    let rows = stmt.query_map([], map_pending)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Takes words off Needs review and returns them for Undo.
pub fn remove_pending(conn: &mut Connection, ids: &[i64]) -> Result<Vec<PendingWord>> {
    let tx = conn.transaction()?;
    let mut gone = Vec::new();
    for id in ids {
        if let Some(p) = tx
            .query_row(
                "SELECT * FROM dictionary_pending WHERE id = ?1",
                params![id],
                map_pending,
            )
            .optional()?
        {
            tx.execute("DELETE FROM dictionary_pending WHERE id = ?1", params![id])?;
            gone.push(p);
        }
    }
    tx.commit()?;
    Ok(gone)
}

pub fn restore_pending(conn: &mut Connection, words: &[PendingWord]) -> Result<()> {
    let tx = conn.transaction()?;
    for p in words {
        tx.execute(
            "INSERT OR REPLACE INTO dictionary_pending (id, word, heard, context, app, app_path, at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![p.id, p.word, p.heard, p.context, p.app, p.app_path, p.at],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// Words Learn from my edits never proposes, A to Z.
pub fn never(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT word FROM dictionary_never ORDER BY word COLLATE NOCASE")?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Stops proposing a word, and drops it from Needs review.
pub fn add_never(conn: &mut Connection, word: &str, now: i64) -> Result<()> {
    let word = clean_word(word);
    if word.is_empty() {
        return Ok(());
    }
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT OR IGNORE INTO dictionary_never (word, added_at) VALUES (?1, ?2)",
        params![word, now],
    )?;
    tx.execute(
        "DELETE FROM dictionary_pending WHERE word = ?1 COLLATE NOCASE",
        params![word],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn remove_never(conn: &Connection, word: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM dictionary_never WHERE word = ?1 COLLATE NOCASE",
        params![clean_word(word)],
    )?;
    Ok(())
}

/// Days of dictations searched for terms spelled more than one way.
pub const SPELLING_DAYS: i64 = 30;

/// Terms the model wrote more than one way in the last 30 days, leaving out
/// words already in the Dictionary and words turned down.
pub fn spellings(conn: &Connection, now: i64) -> Result<Vec<Spelling>> {
    let mut stmt = conn.prepare(
        "SELECT COALESCE(heard_text, transcription_text), timestamp, app_name, app_path
         FROM transcription_history WHERE timestamp >= ?1",
    )?;
    let rows = stmt
        .query_map(params![now - SPELLING_DAYS * 86_400], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let said: Vec<Said<'_>> = rows
        .iter()
        .map(|(text, at, app, path)| Said {
            text,
            at: *at,
            app: app.as_deref(),
            app_path: path.as_deref(),
        })
        .collect();
    let never: HashSet<String> = never(conn)?.iter().map(|w| w.to_lowercase()).collect();
    let mut out = Vec::new();
    for spelling in spelled_differently(&said) {
        if !never.contains(&spelling.word.to_lowercase()) && find(conn, &spelling.word)?.is_none() {
            out.push(spelling);
        }
    }
    Ok(out)
}

/// What `learn` did, with the word as it was before for Undo.
#[derive(Clone, Debug)]
pub struct Learned {
    pub entry: DictionaryEntry,
    pub created: bool,
    /// A new heard form was added to a word that was already there.
    pub extended: bool,
    pub before: Option<DictionaryEntry>,
}

/// Adds a word, or extends the one already spelled that way. `heard` is the
/// way it was misheard, when known; `fixed` counts this as a fix.
pub fn learn(
    conn: &mut Connection,
    word: &str,
    heard: Option<&str>,
    source: DictionarySource,
    fixed: bool,
    app: FixApp<'_>,
    now: i64,
) -> Result<Learned> {
    let heard: Vec<String> = heard.map(|h| vec![h.to_string()]).unwrap_or_default();
    match find(conn, word)? {
        Some(before) => {
            let tx = conn.transaction()?;
            let added = add_heard(&tx, before.id, &before.text, &heard)?;
            if fixed {
                tx.execute(
                    "UPDATE dictionary_entry
                     SET hits = hits + 1, last_fix_at = ?1,
                         last_app = COALESCE(?2, last_app),
                         last_app_path = COALESCE(?3, last_app_path)
                     WHERE id = ?4",
                    params![
                        now,
                        app.map(|(name, _)| name),
                        app.and_then(|(_, path)| path),
                        before.id
                    ],
                )?;
            }
            let entry = get(&tx, before.id)?.ok_or_else(|| anyhow!("Saved word is missing"))?;
            tx.commit()?;
            Ok(Learned {
                entry,
                created: false,
                extended: !added.is_empty(),
                before: Some(before),
            })
        }
        None => {
            let input = EntryInput {
                text: word.to_string(),
                heard,
                ..Default::default()
            };
            let mut entry = add(conn, &input, source, now)?;
            if fixed {
                record_fixes(conn, &[entry.text.as_str()], app, now)?;
                entry = get(conn, entry.id)?.ok_or_else(|| anyhow!("Saved word is missing"))?;
            }
            Ok(Learned {
                entry,
                created: true,
                extended: false,
                before: None,
            })
        }
    }
}

/// Undoes `learn`: removes a word it created, or puts back the one it
/// changed.
pub fn unlearn(conn: &mut Connection, learned: &Learned) -> Result<()> {
    match &learned.before {
        Some(before) => restore(conn, std::slice::from_ref(before)),
        None => delete(conn, &[learned.entry.id]).map(|_| ()),
    }
}

/// How many words an import added, extended and skipped.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ImportSummary {
    pub added: u32,
    pub updated: u32,
    pub skipped: u32,
}

/// Adds imported words. A word already there gains the file's heard forms;
/// pins beyond the limit are dropped.
pub fn import(
    conn: &mut Connection,
    rows: &[EntryInput],
    source: DictionarySource,
    now: i64,
) -> Result<ImportSummary> {
    let mut summary = ImportSummary::default();
    for row in rows {
        match find(conn, &row.text)? {
            Some(existing) => {
                let tx = conn.transaction()?;
                let added = add_heard(&tx, existing.id, &existing.text, &row.heard)?;
                tx.commit()?;
                if added.is_empty() {
                    summary.skipped += 1;
                } else {
                    summary.updated += 1;
                }
            }
            None => {
                let mut row = row.clone();
                if row.pinned && pinned_count(conn, None)? >= PIN_LIMIT {
                    row.pinned = false;
                }
                match add(conn, &row, source, now) {
                    Ok(_) => summary.added += 1,
                    Err(_) => summary.skipped += 1,
                }
            }
        }
    }
    Ok(summary)
}

/// Moves the old custom-words setting into the Dictionary, in its order, as
/// words the user added. Nothing is pinned.
pub fn import_custom_words(
    conn: &mut Connection,
    words: &[String],
    now: i64,
) -> Result<ImportSummary> {
    let rows: Vec<EntryInput> = words
        .iter()
        .map(|w| EntryInput {
            text: w.clone(),
            ..Default::default()
        })
        .collect();
    import(conn, &rows, DictionarySource::Manual, now)
}

/// Splits one CSV line, honouring double quotes.
fn csv_fields(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' | ';' | '\t' if !quoted => fields.push(std::mem::take(&mut field)),
            _ => field.push(c),
        }
    }
    fields.push(field);
    fields.into_iter().map(|f| f.trim().to_string()).collect()
}

const WORD_HEADERS: &[&str] = &[
    "word",
    "words",
    "text",
    "to",
    "correction",
    "replacement",
    "spelling",
];
const HEARD_HEADERS: &[&str] = &[
    "heard",
    "heard as",
    "from",
    "misspelling",
    "phrase",
    "trigger",
];

#[derive(Deserialize)]
#[serde(untagged)]
enum JsonWord {
    Plain(String),
    Entry(EntryInput),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum JsonFile {
    List(Vec<JsonWord>),
    Export { words: Vec<JsonWord> },
}

/// Reads a CSV of words (one per line), a CSV of heard,word pairs, or JSON:
/// Legato's export, or a list of words.
pub fn parse_import(content: &str) -> Result<Vec<EntryInput>> {
    let content = content.trim_start_matches('\u{feff}').trim();
    if content.starts_with('[') || content.starts_with('{') {
        let file: JsonFile = serde_json::from_str(content)
            .map_err(|e| anyhow!("Not a word list Legato can read: {e}"))?;
        let words = match file {
            JsonFile::List(words) | JsonFile::Export { words } => words,
        };
        return Ok(words
            .into_iter()
            .map(|w| match w {
                JsonWord::Plain(text) => EntryInput {
                    text,
                    ..Default::default()
                },
                JsonWord::Entry(entry) => entry,
            })
            .filter(|w| !clean_word(&w.text).is_empty())
            .collect());
    }

    let mut lines = content.lines().filter(|l| !l.trim().is_empty()).peekable();
    // Column order: heard,word unless a header says otherwise.
    let (mut word_col, mut heard_col) = (1usize, Some(0usize));
    if let Some(first) = lines.peek() {
        let header: Vec<String> = csv_fields(first).iter().map(|f| f.to_lowercase()).collect();
        let w = header
            .iter()
            .position(|h| WORD_HEADERS.contains(&h.as_str()));
        let h = header
            .iter()
            .position(|h| HEARD_HEADERS.contains(&h.as_str()));
        if w.is_some() || h.is_some() {
            word_col = w.unwrap_or(if h == Some(0) { 1 } else { 0 });
            heard_col = h;
            lines.next();
        }
    }
    let mut rows: Vec<EntryInput> = Vec::new();
    for line in lines {
        let fields = csv_fields(line);
        let (text, heard) = if fields.len() == 1 {
            (fields[0].clone(), None)
        } else {
            (
                fields.get(word_col).cloned().unwrap_or_default(),
                heard_col.and_then(|i| fields.get(i).cloned()),
            )
        };
        let text = clean_word(&text);
        if text.is_empty() {
            continue;
        }
        let heard: Vec<String> = heard.into_iter().filter(|h| !h.is_empty()).collect();
        // Several lines for one word gather its heard forms.
        match rows
            .iter_mut()
            .find(|r| r.text.to_lowercase() == text.to_lowercase())
        {
            Some(row) => row.heard.extend(heard),
            None => rows.push(EntryInput {
                text,
                heard,
                ..Default::default()
            }),
        }
    }
    Ok(rows)
}

#[derive(Serialize)]
struct ExportFile<'a> {
    version: u32,
    words: Vec<&'a DictionaryEntry>,
}

/// The whole Dictionary as JSON, readable by `parse_import`.
pub fn export_json(entries: &[DictionaryEntry]) -> Result<String> {
    Ok(serde_json::to_string_pretty(&ExportFile {
        version: 1,
        words: entries.iter().collect(),
    })?)
}

/// The vocabulary hint: pinned words first, then the most recently used or
/// added, up to `HINT_LIMIT`.
pub fn hint(entries: &[DictionaryEntry]) -> Vec<String> {
    let mut pinned: Vec<&DictionaryEntry> = entries.iter().filter(|e| e.pinned).collect();
    pinned.sort_by_key(|e| e.id);
    let mut rest: Vec<&DictionaryEntry> = entries.iter().filter(|e| !e.pinned).collect();
    rest.sort_by(|a, b| {
        let used = |e: &DictionaryEntry| e.last_fix_at.unwrap_or(0).max(e.added_at);
        used(b).cmp(&used(a)).then(a.id.cmp(&b.id))
    });
    pinned
        .into_iter()
        .chain(rest)
        .take(HINT_LIMIT)
        .map(|e| e.text.clone())
        .collect()
}

/// The hint and the corrector for a set of words.
pub fn compile(entries: &[DictionaryEntry]) -> (Vec<String>, CompiledDictionary) {
    let hint = hint(entries);
    let words = entries
        .iter()
        .map(|e| DictionaryWord {
            text: e.text.clone(),
            heard: e.heard.clone(),
            case_sensitive: e.case_sensitive,
            in_hint: hint.contains(&e.text),
        })
        .collect();
    (hint, CompiledDictionary::new(words))
}

#[cfg(test)]
mod tests {
    use super::super::history_store::MIGRATIONS;
    use super::*;
    use rusqlite_migration::Migrations;

    fn db() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        Migrations::new(MIGRATIONS.to_vec())
            .to_latest(&mut conn)
            .unwrap();
        conn
    }

    fn input(text: &str, heard: &[&str]) -> EntryInput {
        EntryInput {
            text: text.into(),
            heard: heard.iter().map(|h| h.to_string()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn edits_wait_for_review_unless_known_or_never() {
        let mut conn = db();
        let app = Some(("Slack", Some("C:\\slack.exe")));
        let p = add_pending(&conn, "Dokploy", "doc ploy", "the Dokploy deploy", app, 10)
            .unwrap()
            .unwrap();
        assert_eq!((p.word.as_str(), p.heard.as_str()), ("Dokploy", "doc ploy"));
        // The same correction again moves it up instead of adding a row.
        let again = add_pending(&conn, "Dokploy", "Doc Ploy", "again", app, 20)
            .unwrap()
            .unwrap();
        assert_eq!(again.id, p.id);
        assert_eq!(again.at, 20);
        assert_eq!(pending(&conn).unwrap().len(), 1);

        // Already in the Dictionary with that heard form: nothing to review.
        add(
            &mut conn,
            &input("Karol", &["Carol"]),
            DictionarySource::Manual,
            1,
        )
        .unwrap();
        assert!(add_pending(&conn, "Karol", "carol", "", None, 30)
            .unwrap()
            .is_none());
        // Known word, new heard form: worth proposing.
        assert!(add_pending(&conn, "Karol", "Carl", "", None, 30)
            .unwrap()
            .is_some());

        // Never learn drops it from review and keeps it out.
        add_never(&mut conn, "dokploy", 40).unwrap();
        assert!(pending(&conn).unwrap().iter().all(|p| p.word != "Dokploy"));
        assert!(add_pending(&conn, "Dokploy", "doc ploy", "", None, 50)
            .unwrap()
            .is_none());
        assert_eq!(never(&conn).unwrap(), vec!["dokploy".to_string()]);
        remove_never(&conn, "Dokploy").unwrap();
        assert!(never(&conn).unwrap().is_empty());

        // Removed for review can come back with Undo.
        let all = pending(&conn).unwrap();
        let gone =
            remove_pending(&mut conn, &all.iter().map(|p| p.id).collect::<Vec<_>>()).unwrap();
        assert!(pending(&conn).unwrap().is_empty());
        restore_pending(&mut conn, &gone).unwrap();
        assert_eq!(pending(&conn).unwrap(), all);
    }

    #[test]
    fn words_are_added_with_their_heard_forms() {
        let mut conn = db();
        let e = add(
            &mut conn,
            &input(
                " Dokploy ",
                &["doc ploy", "Doc Ploy", "dokploy", "  deploy."],
            ),
            DictionarySource::Manual,
            10,
        )
        .unwrap();
        assert_eq!(e.text, "Dokploy");
        assert_eq!(e.heard, vec!["doc ploy", "deploy"]);
        assert!(add(
            &mut conn,
            &input("dokploy", &[]),
            DictionarySource::Manual,
            11
        )
        .is_err());
        assert!(add(&mut conn, &input("  ", &[]), DictionarySource::Manual, 11).is_err());
        assert_eq!(list(&conn).unwrap().len(), 1);
    }

    #[test]
    fn a_heard_form_belongs_to_one_word() {
        let mut conn = db();
        add(
            &mut conn,
            &input("Karol", &["carol"]),
            DictionarySource::Manual,
            1,
        )
        .unwrap();
        let other = add(
            &mut conn,
            &input("Carroll", &["carol"]),
            DictionarySource::Manual,
            2,
        )
        .unwrap();
        let all = list(&conn).unwrap();
        assert!(all[0].heard.is_empty());
        assert_eq!(all[1].heard, vec!["carol"]);
        // A word can't be heard as another word's spelling.
        let e = update(&mut conn, other.id, &input("Carroll", &["karol"])).unwrap();
        assert!(e.heard.is_empty());
    }

    #[test]
    fn pins_are_limited() {
        let mut conn = db();
        for i in 0..PIN_LIMIT {
            let mut w = input(&format!("Word{i}"), &[]);
            w.pinned = true;
            add(&mut conn, &w, DictionarySource::Manual, i as i64).unwrap();
        }
        let last = add(
            &mut conn,
            &input("Extra", &[]),
            DictionarySource::Manual,
            99,
        )
        .unwrap();
        assert!(set_pinned(&mut conn, last.id, true).is_err());
        let first = find(&conn, "word0").unwrap().unwrap();
        set_pinned(&mut conn, first.id, false).unwrap();
        assert!(set_pinned(&mut conn, last.id, true).unwrap().pinned);
    }

    #[test]
    fn delete_and_restore_round_trip() {
        let mut conn = db();
        let a = add(
            &mut conn,
            &input("Netbird", &["net bird"]),
            DictionarySource::Manual,
            1,
        )
        .unwrap();
        record_fixes(&mut conn, &["netbird"], Some(("Slack", None)), 5).unwrap();
        let before = list(&conn).unwrap();
        let removed = delete(&mut conn, &[a.id]).unwrap();
        assert!(list(&conn).unwrap().is_empty());
        restore(&mut conn, &removed).unwrap();
        assert_eq!(list(&conn).unwrap(), before);
        assert_eq!(before[0].hits, 1);
        assert_eq!(before[0].last_app.as_deref(), Some("Slack"));
    }

    #[test]
    fn learn_creates_or_extends_and_can_be_undone() {
        let mut conn = db();
        let made = learn(
            &mut conn,
            "Coolify",
            Some("cool if I"),
            DictionarySource::Fixed,
            true,
            Some(("Cursor", None)),
            3,
        )
        .unwrap();
        assert!(made.created);
        assert_eq!(made.entry.heard, vec!["cool if I"]);
        assert_eq!(made.entry.hits, 1);
        let more = learn(
            &mut conn,
            "coolify",
            Some("cool lify"),
            DictionarySource::Fixed,
            true,
            None,
            4,
        )
        .unwrap();
        assert!(!more.created && more.extended);
        assert_eq!(more.entry.hits, 2);
        assert_eq!(more.entry.last_app.as_deref(), Some("Cursor"));
        unlearn(&mut conn, &more).unwrap();
        assert_eq!(list(&conn).unwrap(), vec![made.entry.clone()]);
        unlearn(&mut conn, &made).unwrap();
        assert!(list(&conn).unwrap().is_empty());
    }

    #[test]
    fn hint_puts_pins_first_then_recent_use() {
        let mut conn = db();
        add(&mut conn, &input("Old", &[]), DictionarySource::Manual, 1).unwrap();
        add(&mut conn, &input("Newer", &[]), DictionarySource::Manual, 2).unwrap();
        let mut pinned = input("Pinned", &[]);
        pinned.pinned = true;
        add(&mut conn, &pinned, DictionarySource::Manual, 0).unwrap();
        record_fixes(&mut conn, &["old"], None, 9).unwrap();
        assert_eq!(hint(&list(&conn).unwrap()), vec!["Pinned", "Old", "Newer"]);
    }

    #[test]
    fn hint_keeps_migrated_order_and_caps_at_the_limit() {
        let mut conn = db();
        let words: Vec<String> = (0..50).map(|i| format!("Term{i}")).collect();
        let summary = import_custom_words(&mut conn, &words, 7).unwrap();
        assert_eq!(summary.added, 50);
        let h = hint(&list(&conn).unwrap());
        assert_eq!(h.len(), HINT_LIMIT);
        assert_eq!(h[0], "Term0");
        assert_eq!(h[39], "Term39");
    }

    #[test]
    fn compile_marks_hint_words() {
        let mut conn = db();
        add(
            &mut conn,
            &input("Dokploy", &["doc ploy"]),
            DictionarySource::Manual,
            1,
        )
        .unwrap();
        let (hint, dict) = compile(&list(&conn).unwrap());
        assert_eq!(hint, vec!["Dokploy"]);
        assert_eq!(
            dict.apply("restart doc ploy", true, 0.18).0,
            "restart Dokploy"
        );
    }

    #[test]
    fn csv_and_json_imports_are_read() {
        let rows = parse_import("Dokploy\nNetbird\n").unwrap();
        assert_eq!(
            rows.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
            vec!["Dokploy", "Netbird"]
        );

        let rows =
            parse_import("doc ploy,Dokploy\n\"net, bird\",Netbird\ndeploy,dokploy\n").unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].heard, vec!["doc ploy", "deploy"]);
        assert_eq!(rows[1].heard, vec!["net, bird"]);

        let rows = parse_import("word,heard\nKarol,carol\n").unwrap();
        assert_eq!(rows, vec![input("Karol", &["carol"])]);

        let rows = parse_import(
            "[\"Samir\", {\"text\": \"Karol\", \"heard\": [\"carol\"], \"pinned\": true}]",
        )
        .unwrap();
        assert_eq!(rows[0].text, "Samir");
        assert!(rows[1].pinned);

        let mut conn = db();
        add(
            &mut conn,
            &input("Karol", &["carol"]),
            DictionarySource::Manual,
            1,
        )
        .unwrap();
        let json = export_json(&list(&conn).unwrap()).unwrap();
        let rows = parse_import(&json).unwrap();
        assert_eq!(rows, vec![input("Karol", &["carol"])]);
        assert!(parse_import("{\"nope\": 1}").is_err());
    }

    #[test]
    fn import_adds_new_words_and_extends_old_ones() {
        let mut conn = db();
        add(&mut conn, &input("Karol", &[]), DictionarySource::Manual, 1).unwrap();
        let rows = vec![
            input("karol", &["carol"]),
            input("Samir", &[]),
            input("Samir", &[]),
            input("", &[]),
        ];
        let summary = import(&mut conn, &rows, DictionarySource::Imported, 2).unwrap();
        assert_eq!(
            summary,
            ImportSummary {
                added: 1,
                updated: 1,
                skipped: 2
            }
        );
        assert_eq!(
            find(&conn, "samir").unwrap().unwrap().source,
            DictionarySource::Imported
        );
    }

    #[test]
    fn spellings_leave_out_known_and_turned_down_words() {
        let mut conn = db();
        let now = 2_000_000_000;
        for (i, text) in [
            "move the app from cool if I to Dokploy",
            "the Coolify dashboard is down",
            "is coolify still up",
            "the Nemotron model and nemo tron",
            "try Nemotron again",
        ]
        .iter()
        .enumerate()
        {
            conn.execute(
                "INSERT INTO transcription_history (file_name, timestamp, title, transcription_text, app_name)
                 VALUES ('a.wav', ?1, 't', ?2, 'Claude Code')",
                params![now - 100 + i as i64, text],
            )
            .unwrap();
        }
        // Too old to count.
        conn.execute(
            "INSERT INTO transcription_history (file_name, timestamp, title, transcription_text)
             VALUES ('a.wav', ?1, 't', 'the Granola app, granola, gran ola')",
            params![now - (SPELLING_DAYS + 1) * 86_400],
        )
        .unwrap();
        let words = |conn: &Connection| -> Vec<String> {
            spellings(conn, now)
                .unwrap()
                .into_iter()
                .map(|s| s.word)
                .collect()
        };
        // As often as each other; the later one first.
        assert_eq!(words(&conn), vec!["Nemotron", "Coolify"]);
        add(
            &mut conn,
            &input("Coolify", &["cool if I"]),
            DictionarySource::Spelled,
            now,
        )
        .unwrap();
        add_never(&mut conn, "Nemotron", now).unwrap();
        assert!(words(&conn).is_empty());
    }
}
