//! Notes' tables: recordings with their transcript lines, tags, action items
//! and summary. Plain rusqlite, like `scratch_store`; `NotesManager` wraps
//! it with files and events.

use anyhow::{anyhow, Result};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::notes_rules::{clock, NotesSummary};

/// A voice memo is the microphone alone; a meeting adds what the PC plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum NoteKind {
    Memo,
    Meeting,
}

impl NoteKind {
    pub fn as_str(self) -> &'static str {
        match self {
            NoteKind::Memo => "memo",
            NoteKind::Meeting => "meeting",
        }
    }

    fn parse(s: &str) -> Self {
        if s == "meeting" {
            NoteKind::Meeting
        } else {
            NoteKind::Memo
        }
    }
}

/// Where a note is in its life.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum NoteStatus {
    Recording,
    /// Stopped; the last lines are still being transcribed.
    Transcribing,
    /// The transcript is done; a summary is being written.
    Summarising,
    Ready,
}

impl NoteStatus {
    fn as_str(self) -> &'static str {
        match self {
            NoteStatus::Recording => "recording",
            NoteStatus::Transcribing => "transcribing",
            NoteStatus::Summarising => "summarising",
            NoteStatus::Ready => "ready",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "recording" => NoteStatus::Recording,
            "transcribing" => NoteStatus::Transcribing,
            "summarising" => NoteStatus::Summarising,
            _ => NoteStatus::Ready,
        }
    }
}

/// Who said a line: the microphone, or what the PC played.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum Speaker {
    You,
    Them,
}

impl Speaker {
    pub fn as_str(self) -> &'static str {
        match self {
            Speaker::You => "you",
            Speaker::Them => "them",
        }
    }

    fn parse(s: &str) -> Self {
        if s == "them" {
            Speaker::Them
        } else {
            Speaker::You
        }
    }
}

/// A note as the list shows it, without its transcript.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct Note {
    pub id: i64,
    pub title: String,
    pub kind: NoteKind,
    pub status: NoteStatus,
    /// The app whose sound was recorded, when one app was.
    pub app: Option<String>,
    pub app_path: Option<String>,
    /// The microphone and the system audio, by name, as recorded.
    pub microphone: Option<String>,
    pub system_audio: Option<String>,
    /// Unix seconds.
    pub started_at: i64,
    pub duration_ms: i64,
    /// The audio's file name in the notes folder, until it is deleted.
    pub audio_file: Option<String>,
    pub tags: Vec<String>,
    pub my_notes: String,
    pub summary: String,
    pub decisions: Vec<String>,
    /// "Ollama · qwen2.5:7b", the model that wrote the summary.
    pub summary_model: Option<String>,
    pub summary_at: Option<i64>,
    /// Why there is no summary, when writing one failed.
    pub summary_error: Option<String>,
    /// When a line was last corrected (Unix seconds).
    #[serde(default)]
    pub edited_at: Option<i64>,
    /// Lines that couldn't be transcribed.
    pub failed_lines: i64,
    /// Of those, the ones that can be tried again from the audio.
    #[serde(default)]
    pub retryable: i64,
    pub lines: i64,
    pub actions: Vec<NoteAction>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct NoteLine {
    pub id: i64,
    pub note_id: i64,
    /// Milliseconds from the start of the recording.
    pub at_ms: i64,
    pub end_ms: i64,
    pub who: Speaker,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct NoteAction {
    pub id: i64,
    pub owner: String,
    pub text: String,
    pub done: bool,
}

/// A note and everything in it, for Undo after a delete.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct DeletedRecording {
    pub note: Note,
    pub lines: Vec<NoteLine>,
    #[serde(default)]
    pub failed: Vec<FailedLine>,
}

/// A stretch of a recording the speech model couldn't transcribe, kept so
/// it can be tried again from the note's audio.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct FailedLine {
    pub id: i64,
    pub note_id: i64,
    pub at_ms: i64,
    pub end_ms: i64,
    pub who: Speaker,
}

/// What a search matched in a note: a line of its transcript, or None when
/// the title, a tag, the summary or My notes matched.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct NoteMatch {
    pub id: i64,
    pub line: Option<NoteLine>,
}

pub struct NewNote<'a> {
    pub title: &'a str,
    pub kind: NoteKind,
    pub app: Option<&'a str>,
    pub app_path: Option<&'a str>,
    pub microphone: Option<&'a str>,
    pub system_audio: Option<&'a str>,
    pub started_at: i64,
}

const COLUMNS: &str = "id, title, kind, status, app, app_path, microphone, system_audio,
    started_at, duration_ms, audio_file, my_notes, summary, decisions, summary_model,
    summary_at, summary_error, failed_lines, edited_at,
    (SELECT COUNT(*) FROM note_line l WHERE l.note_id = note.id) AS lines,
    (SELECT COUNT(*) FROM note_failed_line f WHERE f.note_id = note.id) AS retryable";

fn map(row: &Row<'_>) -> rusqlite::Result<Note> {
    let kind: String = row.get("kind")?;
    let status: String = row.get("status")?;
    let decisions: String = row.get("decisions")?;
    Ok(Note {
        id: row.get("id")?,
        title: row.get("title")?,
        kind: NoteKind::parse(&kind),
        status: NoteStatus::parse(&status),
        app: row.get("app")?,
        app_path: row.get("app_path")?,
        microphone: row.get("microphone")?,
        system_audio: row.get("system_audio")?,
        started_at: row.get("started_at")?,
        duration_ms: row.get("duration_ms")?,
        audio_file: row.get("audio_file")?,
        tags: Vec::new(),
        my_notes: row.get("my_notes")?,
        summary: row.get("summary")?,
        decisions: serde_json::from_str(&decisions).unwrap_or_default(),
        summary_model: row.get("summary_model")?,
        summary_at: row.get("summary_at")?,
        summary_error: row.get("summary_error")?,
        edited_at: row.get("edited_at")?,
        failed_lines: row.get("failed_lines")?,
        retryable: row.get("retryable")?,
        lines: row.get("lines")?,
        actions: Vec::new(),
    })
}

fn map_line(row: &Row<'_>) -> rusqlite::Result<NoteLine> {
    let who: String = row.get("who")?;
    Ok(NoteLine {
        id: row.get("id")?,
        note_id: row.get("note_id")?,
        at_ms: row.get("at_ms")?,
        end_ms: row.get("end_ms")?,
        who: Speaker::parse(&who),
        text: row.get("text")?,
    })
}

fn fill(conn: &Connection, mut note: Note) -> Result<Note> {
    let mut stmt =
        conn.prepare("SELECT tag FROM note_tag WHERE note_id = ?1 ORDER BY tag COLLATE NOCASE")?;
    note.tags = stmt
        .query_map(params![note.id], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let mut stmt = conn.prepare(
        "SELECT id, owner, text, done FROM note_action WHERE note_id = ?1 ORDER BY position, id",
    )?;
    note.actions = stmt
        .query_map(params![note.id], |r| {
            Ok(NoteAction {
                id: r.get(0)?,
                owner: r.get(1)?,
                text: r.get(2)?,
                done: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(note)
}

/// Every note, newest first.
pub fn list(conn: &Connection) -> Result<Vec<Note>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM note ORDER BY started_at DESC, id DESC"
    ))?;
    let notes = stmt
        .query_map([], map)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    notes.into_iter().map(|n| fill(conn, n)).collect()
}

pub fn get(conn: &Connection, id: i64) -> Result<Option<Note>> {
    let note = conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM note WHERE id = ?1"),
            params![id],
            map,
        )
        .optional()?;
    note.map(|n| fill(conn, n)).transpose()
}

pub fn create(conn: &Connection, new: &NewNote<'_>) -> Result<Note> {
    conn.execute(
        "INSERT INTO note (title, kind, status, app, app_path, microphone, system_audio, started_at)
         VALUES (?1, ?2, 'recording', ?3, ?4, ?5, ?6, ?7)",
        params![
            new.title,
            new.kind.as_str(),
            new.app,
            new.app_path,
            new.microphone,
            new.system_audio,
            new.started_at
        ],
    )?;
    get(conn, conn.last_insert_rowid())?.ok_or_else(|| anyhow!("The new note wasn't saved"))
}

/// The transcript, in order.
pub fn lines(conn: &Connection, note_id: i64) -> Result<Vec<NoteLine>> {
    let mut stmt = conn.prepare(
        "SELECT id, note_id, at_ms, end_ms, who, text FROM note_line
         WHERE note_id = ?1 ORDER BY at_ms, id",
    )?;
    let lines = stmt
        .query_map(params![note_id], map_line)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(lines)
}

pub fn add_line(
    conn: &Connection,
    note_id: i64,
    at_ms: i64,
    end_ms: i64,
    who: Speaker,
    text: &str,
) -> Result<NoteLine> {
    conn.execute(
        "INSERT INTO note_line (note_id, at_ms, end_ms, who, text) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![note_id, at_ms, end_ms, who.as_str(), text],
    )?;
    Ok(NoteLine {
        id: conn.last_insert_rowid(),
        note_id,
        at_ms,
        end_ms,
        who,
        text: text.to_string(),
    })
}

pub fn remove_line(conn: &Connection, line_id: i64) -> Result<()> {
    conn.execute("DELETE FROM note_line WHERE id = ?1", params![line_id])?;
    Ok(())
}

pub fn line(conn: &Connection, line_id: i64) -> Result<Option<NoteLine>> {
    Ok(conn
        .query_row(
            "SELECT id, note_id, at_ms, end_ms, who, text FROM note_line WHERE id = ?1",
            params![line_id],
            map_line,
        )
        .optional()?)
}

/// A line's corrected words, or who said it.
pub fn update_line(conn: &Connection, line_id: i64, who: Speaker, text: &str) -> Result<NoteLine> {
    let text = text.trim();
    if text.is_empty() {
        return Err(anyhow!("A line needs some words; delete it instead"));
    }
    conn.execute(
        "UPDATE note_line SET who = ?2, text = ?3 WHERE id = ?1",
        params![line_id, who.as_str(), text],
    )?;
    line(conn, line_id)?.ok_or_else(|| anyhow!("That line is gone"))
}

/// Notes that the transcript was corrected now.
pub fn mark_edited(conn: &Connection, note_id: i64, at: i64) -> Result<()> {
    conn.execute(
        "UPDATE note SET edited_at = ?2 WHERE id = ?1",
        params![note_id, at],
    )?;
    Ok(())
}

/// Puts a deleted line back as it was, id and all.
pub fn restore_line(conn: &Connection, line: &NoteLine) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO note_line (id, note_id, at_ms, end_ms, who, text)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            line.id,
            line.note_id,
            line.at_ms,
            line.end_ms,
            line.who.as_str(),
            line.text
        ],
    )?;
    Ok(())
}

/// Lines by the other speaker that overlap `[from_ms, to_ms]`.
pub fn lines_near(
    conn: &Connection,
    note_id: i64,
    who: Speaker,
    from_ms: i64,
    to_ms: i64,
) -> Result<Vec<NoteLine>> {
    let mut stmt = conn.prepare(
        "SELECT id, note_id, at_ms, end_ms, who, text FROM note_line
         WHERE note_id = ?1 AND who = ?2 AND end_ms >= ?3 AND at_ms <= ?4",
    )?;
    let lines = stmt
        .query_map(params![note_id, who.as_str(), from_ms, to_ms], map_line)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(lines)
}

pub fn set_status(conn: &Connection, id: i64, status: NoteStatus) -> Result<()> {
    conn.execute(
        "UPDATE note SET status = ?2 WHERE id = ?1",
        params![id, status.as_str()],
    )?;
    Ok(())
}

pub fn add_failed_line(
    conn: &Connection,
    note_id: i64,
    at_ms: i64,
    end_ms: i64,
    who: Speaker,
) -> Result<()> {
    conn.execute(
        "INSERT INTO note_failed_line (note_id, at_ms, end_ms, who) VALUES (?1, ?2, ?3, ?4)",
        params![note_id, at_ms, end_ms, who.as_str()],
    )?;
    Ok(())
}

pub fn failed_lines(conn: &Connection, note_id: i64) -> Result<Vec<FailedLine>> {
    let mut stmt = conn.prepare(
        "SELECT id, note_id, at_ms, end_ms, who FROM note_failed_line
         WHERE note_id = ?1 ORDER BY at_ms",
    )?;
    let rows = stmt.query_map(params![note_id], |row| {
        let who: String = row.get("who")?;
        Ok(FailedLine {
            id: row.get("id")?,
            note_id: row.get("note_id")?,
            at_ms: row.get("at_ms")?,
            end_ms: row.get("end_ms")?,
            who: Speaker::parse(&who),
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// A failed line that was transcribed on a retry; the note's count follows.
pub fn resolve_failed_line(conn: &Connection, line: &FailedLine) -> Result<()> {
    conn.execute(
        "DELETE FROM note_failed_line WHERE id = ?1",
        params![line.id],
    )?;
    conn.execute(
        "UPDATE note SET failed_lines = MAX(failed_lines - 1, 0) WHERE id = ?1",
        params![line.note_id],
    )?;
    Ok(())
}

pub fn finish_recording(
    conn: &Connection,
    id: i64,
    duration_ms: i64,
    audio_file: Option<&str>,
    failed_lines: i64,
) -> Result<()> {
    conn.execute(
        "UPDATE note SET duration_ms = ?2, audio_file = ?3, failed_lines = ?4 WHERE id = ?1",
        params![id, duration_ms, audio_file, failed_lines],
    )?;
    Ok(())
}

pub fn rename(conn: &Connection, id: i64, title: &str) -> Result<()> {
    let title = title.trim();
    if title.is_empty() {
        return Err(anyhow!("A note needs a title"));
    }
    conn.execute(
        "UPDATE note SET title = ?2 WHERE id = ?1",
        params![id, title],
    )?;
    Ok(())
}

pub fn set_my_notes(conn: &Connection, id: i64, text: &str) -> Result<()> {
    conn.execute(
        "UPDATE note SET my_notes = ?2 WHERE id = ?1",
        params![id, text],
    )?;
    Ok(())
}

pub fn add_tag(conn: &Connection, id: i64, tag: &str) -> Result<()> {
    let tag = tag.trim();
    if tag.is_empty() {
        return Ok(());
    }
    conn.execute(
        "INSERT OR IGNORE INTO note_tag (note_id, tag) VALUES (?1, ?2)",
        params![id, tag],
    )?;
    Ok(())
}

pub fn remove_tag(conn: &Connection, id: i64, tag: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM note_tag WHERE note_id = ?1 AND tag = ?2",
        params![id, tag],
    )?;
    Ok(())
}

pub fn set_action_done(conn: &Connection, action_id: i64, done: bool) -> Result<()> {
    conn.execute(
        "UPDATE note_action SET done = ?2 WHERE id = ?1",
        params![action_id, done],
    )?;
    Ok(())
}

/// Replaces the summary, its decisions and its action items.
pub fn set_summary(
    conn: &mut Connection,
    id: i64,
    summary: &NotesSummary,
    model: &str,
    at: i64,
) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE note SET summary = ?2, decisions = ?3, summary_model = ?4, summary_at = ?5,
            summary_error = NULL WHERE id = ?1",
        params![
            id,
            summary.summary,
            serde_json::to_string(&summary.decisions)?,
            model,
            at
        ],
    )?;
    tx.execute("DELETE FROM note_action WHERE note_id = ?1", params![id])?;
    for (position, (owner, text)) in summary.actions.iter().enumerate() {
        tx.execute(
            "INSERT INTO note_action (note_id, owner, text, done, position)
             VALUES (?1, ?2, ?3, 0, ?4)",
            params![id, owner, text, position as i64],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub fn set_summary_error(conn: &Connection, id: i64, error: Option<&str>) -> Result<()> {
    conn.execute(
        "UPDATE note SET summary_error = ?2 WHERE id = ?1",
        params![id, error],
    )?;
    Ok(())
}

/// Every tag in use, for Add tag and the filter.
pub fn tags(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT DISTINCT tag FROM note_tag ORDER BY tag COLLATE NOCASE")?;
    let tags = stmt
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(tags)
}

/// Notes whose title, tags, summary, My notes or transcript contain `query`,
/// newest first, each with the first matching line when the transcript
/// matched.
pub fn search(conn: &Connection, query: &str) -> Result<Vec<NoteMatch>> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let pattern = format!(
        "%{}%",
        query
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    );
    let mut stmt = conn.prepare(
        "SELECT n.id,
            (SELECT l.id FROM note_line l WHERE l.note_id = n.id
                AND l.text LIKE ?1 ESCAPE '\\' ORDER BY l.at_ms LIMIT 1) AS line_id,
            n.title LIKE ?1 ESCAPE '\\' OR n.summary LIKE ?1 ESCAPE '\\'
                OR n.my_notes LIKE ?1 ESCAPE '\\'
                OR EXISTS (SELECT 1 FROM note_tag t WHERE t.note_id = n.id
                    AND t.tag LIKE ?1 ESCAPE '\\') AS direct
         FROM note n ORDER BY n.started_at DESC, n.id DESC",
    )?;
    let rows = stmt
        .query_map(params![pattern], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<i64>>(1)?,
                r.get::<_, bool>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut matches = Vec::new();
    for (id, line_id, direct) in rows {
        if line_id.is_none() && !direct {
            continue;
        }
        let line = match line_id {
            Some(line_id) if !direct => conn
                .query_row(
                    "SELECT id, note_id, at_ms, end_ms, who, text FROM note_line WHERE id = ?1",
                    params![line_id],
                    map_line,
                )
                .optional()?,
            _ => None,
        };
        matches.push(NoteMatch { id, line });
    }
    Ok(matches)
}

/// Removes a note and returns it whole, for Undo.
pub fn delete(conn: &mut Connection, id: i64) -> Result<Option<DeletedRecording>> {
    let Some(note) = get(conn, id)? else {
        return Ok(None);
    };
    let lines = lines(conn, id)?;
    let failed = failed_lines(conn, id)?;
    let tx = conn.transaction()?;
    for table in ["note_line", "note_tag", "note_action", "note_failed_line"] {
        tx.execute(
            &format!("DELETE FROM {table} WHERE note_id = ?1"),
            params![id],
        )?;
    }
    tx.execute("DELETE FROM note WHERE id = ?1", params![id])?;
    tx.commit()?;
    Ok(Some(DeletedRecording {
        note,
        lines,
        failed,
    }))
}

/// Puts a deleted note back with its id.
pub fn restore(conn: &mut Connection, deleted: &DeletedRecording) -> Result<()> {
    let n = &deleted.note;
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO note (id, title, kind, status, app, app_path, microphone, system_audio,
            started_at, duration_ms, audio_file, my_notes, summary, decisions, summary_model,
            summary_at, summary_error, failed_lines, edited_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18,
            ?19)",
        params![
            n.id,
            n.title,
            n.kind.as_str(),
            n.status.as_str(),
            n.app,
            n.app_path,
            n.microphone,
            n.system_audio,
            n.started_at,
            n.duration_ms,
            n.audio_file,
            n.my_notes,
            n.summary,
            serde_json::to_string(&n.decisions)?,
            n.summary_model,
            n.summary_at,
            n.summary_error,
            n.failed_lines,
            n.edited_at
        ],
    )?;
    for tag in &n.tags {
        tx.execute(
            "INSERT INTO note_tag (note_id, tag) VALUES (?1, ?2)",
            params![n.id, tag],
        )?;
    }
    for (position, a) in n.actions.iter().enumerate() {
        tx.execute(
            "INSERT INTO note_action (id, note_id, owner, text, done, position)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![a.id, n.id, a.owner, a.text, a.done, position as i64],
        )?;
    }
    for l in &deleted.lines {
        tx.execute(
            "INSERT INTO note_line (id, note_id, at_ms, end_ms, who, text)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![l.id, n.id, l.at_ms, l.end_ms, l.who.as_str(), l.text],
        )?;
    }
    for f in &deleted.failed {
        tx.execute(
            "INSERT INTO note_failed_line (id, note_id, at_ms, end_ms, who)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![f.id, n.id, f.at_ms, f.end_ms, f.who.as_str()],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// Notes left in Recording or Transcribing by a crash or a quit: they keep
/// what was transcribed and become ready.
pub fn recover_unfinished(conn: &Connection) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT id FROM note WHERE status IN ('recording', 'transcribing', 'summarising')",
    )?;
    let ids: Vec<i64> = stmt
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    conn.execute(
        "UPDATE note SET status = 'ready' WHERE status IN ('recording', 'transcribing', 'summarising')",
        [],
    )?;
    Ok(ids)
}

/// The note as Markdown, for Copy and Export.
pub fn markdown(note: &Note, lines: &[NoteLine], you: &str, them: &str) -> String {
    let mut out = format!("# {}\n\n", note.title);
    if !note.tags.is_empty() {
        out.push_str(&format!("Tags: {}\n\n", note.tags.join(", ")));
    }
    if !note.summary.is_empty() {
        out.push_str(&format!("## Summary\n\n{}\n\n", note.summary));
    }
    if !note.decisions.is_empty() {
        out.push_str("## Decisions\n\n");
        for d in &note.decisions {
            out.push_str(&format!("- {d}\n"));
        }
        out.push('\n');
    }
    if !note.actions.is_empty() {
        out.push_str("## Action items\n\n");
        for a in &note.actions {
            let mark = if a.done { "x" } else { " " };
            out.push_str(&format!("- [{mark}] {} ({})\n", a.text, a.owner));
        }
        out.push('\n');
    }
    if !note.my_notes.trim().is_empty() {
        out.push_str(&format!("## My notes\n\n{}\n\n", note.my_notes.trim()));
    }
    if !lines.is_empty() {
        out.push_str("## Transcript\n\n");
        for l in lines {
            let who = match l.who {
                Speaker::You => you,
                Speaker::Them => them,
            };
            out.push_str(&format!("{} {}: {}\n", clock(l.at_ms), who, l.text));
        }
    }
    out.trim_end().to_string() + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        rusqlite_migration::Migrations::new(super::super::history_store::MIGRATIONS.to_vec())
            .to_latest(&mut conn)
            .unwrap();
        conn
    }

    fn meeting(conn: &Connection) -> Note {
        create(
            conn,
            &NewNote {
                title: "Infra sync",
                kind: NoteKind::Meeting,
                app: Some("Microsoft Teams"),
                app_path: None,
                microphone: Some("Yeti X"),
                system_audio: Some("Microsoft Teams"),
                started_at: 1_760_700_000,
            },
        )
        .unwrap()
    }

    #[test]
    fn a_note_keeps_its_lines_tags_and_summary() {
        let mut conn = db();
        let note = meeting(&conn);
        assert_eq!(note.status, NoteStatus::Recording);
        add_line(
            &conn,
            note.id,
            11_000,
            14_000,
            Speaker::Them,
            "Netbird routes are up.",
        )
        .unwrap();
        add_line(
            &conn,
            note.id,
            4_000,
            9_000,
            Speaker::You,
            "Where are we with the ARM box?",
        )
        .unwrap();
        add_tag(&conn, note.id, "AIGHT RE").unwrap();
        add_tag(&conn, note.id, "aight re").unwrap();
        set_summary(
            &mut conn,
            note.id,
            &NotesSummary {
                summary: "Dokploy moves to the ARM box.".into(),
                decisions: vec!["Consolidate on the ARM box".into()],
                actions: vec![("Samir".into(), "Move Dokploy".into())],
            },
            "Ollama · qwen2.5:7b",
            1_760_703_000,
        )
        .unwrap();
        let got = get(&conn, note.id).unwrap().unwrap();
        assert_eq!(got.tags, vec!["AIGHT RE"]);
        assert_eq!(got.lines, 2);
        assert_eq!(got.decisions, vec!["Consolidate on the ARM box"]);
        assert_eq!(got.actions[0].owner, "Samir");
        let lines = lines(&conn, note.id).unwrap();
        assert_eq!(lines[0].who, Speaker::You);
        set_action_done(&conn, got.actions[0].id, true).unwrap();
        assert!(get(&conn, note.id).unwrap().unwrap().actions[0].done);
    }

    #[test]
    fn search_finds_titles_tags_and_transcript_lines() {
        let conn = db();
        let note = meeting(&conn);
        add_line(
            &conn,
            note.id,
            58_000,
            61_000,
            Speaker::Them,
            "Blocked on the ADB API keys.",
        )
        .unwrap();
        add_tag(&conn, note.id, "AIGHT RE").unwrap();
        let by_line = search(&conn, "adb api").unwrap();
        assert_eq!(by_line.len(), 1);
        assert_eq!(by_line[0].line.as_ref().unwrap().at_ms, 58_000);
        let by_tag = search(&conn, "aight").unwrap();
        assert!(by_tag[0].line.is_none());
        assert!(search(&conn, "50%").unwrap().is_empty());
    }

    #[test]
    fn delete_and_restore_bring_everything_back() {
        let mut conn = db();
        let note = meeting(&conn);
        add_line(
            &conn,
            note.id,
            4_000,
            9_000,
            Speaker::You,
            "Okay, let's start.",
        )
        .unwrap();
        add_tag(&conn, note.id, "ALV").unwrap();
        let deleted = delete(&mut conn, note.id).unwrap().unwrap();
        assert!(get(&conn, note.id).unwrap().is_none());
        assert!(lines(&conn, note.id).unwrap().is_empty());
        restore(&mut conn, &deleted).unwrap();
        let back = get(&conn, note.id).unwrap().unwrap();
        assert_eq!(back.tags, vec!["ALV"]);
        assert_eq!(lines(&conn, note.id).unwrap().len(), 1);
    }

    #[test]
    fn unfinished_notes_become_ready() {
        let conn = db();
        let note = meeting(&conn);
        assert_eq!(recover_unfinished(&conn).unwrap(), vec![note.id]);
        assert_eq!(
            get(&conn, note.id).unwrap().unwrap().status,
            NoteStatus::Ready
        );
    }

    #[test]
    fn markdown_has_every_part() {
        let conn = db();
        let note = meeting(&conn);
        let line = add_line(
            &conn,
            note.id,
            1_410_000,
            1_412_000,
            Speaker::Them,
            "Extension, right?",
        )
        .unwrap();
        let md = markdown(
            &get(&conn, note.id).unwrap().unwrap(),
            &[line],
            "You",
            "Them",
        );
        assert!(md.starts_with("# Infra sync\n"));
        assert!(md.contains("23:30 Them: Extension, right?"));
        assert_eq!(clock(3_725_000), "1:02:05");
    }

    #[test]
    fn failed_lines_wait_for_a_retry_and_survive_undo() {
        let mut conn = db();
        let note = meeting(&conn);
        add_failed_line(&conn, note.id, 9_000, 12_000, Speaker::Them).unwrap();
        add_failed_line(&conn, note.id, 2_000, 4_000, Speaker::You).unwrap();
        finish_recording(&conn, note.id, 60_000, Some("a.wav"), 2).unwrap();

        let failed = failed_lines(&conn, note.id).unwrap();
        assert_eq!(failed.len(), 2);
        assert_eq!(failed[0].at_ms, 2_000);
        assert_eq!(failed[1].who, Speaker::Them);
        assert_eq!(get(&conn, note.id).unwrap().unwrap().retryable, 2);

        resolve_failed_line(&conn, &failed[0]).unwrap();
        let after = get(&conn, note.id).unwrap().unwrap();
        assert_eq!((after.failed_lines, after.retryable), (1, 1));

        let deleted = delete(&mut conn, note.id).unwrap().unwrap();
        assert!(failed_lines(&conn, note.id).unwrap().is_empty());
        restore(&mut conn, &deleted).unwrap();
        assert_eq!(
            failed_lines(&conn, note.id).unwrap(),
            vec![failed[1].clone()]
        );
    }

    #[test]
    fn a_line_is_corrected_and_comes_back_after_delete() {
        let conn = db();
        let note = meeting(&conn);
        let line = add_line(
            &conn,
            note.id,
            1_000,
            3_000,
            Speaker::Them,
            "ship cubernetes",
        )
        .unwrap();
        let fixed = update_line(&conn, line.id, Speaker::You, " ship Kubernetes ").unwrap();
        assert_eq!(fixed.text, "ship Kubernetes");
        assert_eq!(fixed.who, Speaker::You);
        assert!(update_line(&conn, line.id, Speaker::You, "  ").is_err());

        remove_line(&conn, line.id).unwrap();
        assert!(super::line(&conn, line.id).unwrap().is_none());
        restore_line(&conn, &fixed).unwrap();
        assert_eq!(lines(&conn, note.id).unwrap(), vec![fixed]);
    }
}
