//! Notes' tables: recordings with their transcript lines, tags, action items
//! and summary. Plain rusqlite, like `scratch_store`; `NotesManager` wraps
//! it with files and events.

use anyhow::{anyhow, Result};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::document::{self, Block, Run};
use crate::notes_rules::{clock, NoteOutline, NotesSummary};

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
    /// Names given to the note's voices; the rest are "Speaker 2" and so on.
    #[serde(default)]
    pub speakers: Vec<NoteSpeaker>,
    /// Lines that couldn't be transcribed.
    pub failed_lines: i64,
    /// Of those, the ones that can be tried again from the audio.
    #[serde(default)]
    pub retryable: i64,
    pub lines: i64,
    pub actions: Vec<NoteAction>,
    /// The summary's topics, timed details and the people it names.
    #[serde(default)]
    pub outline: NoteOutline,
    /// Who was invited, when the note began with a calendar meeting.
    #[serde(default)]
    pub attendees: Vec<String>,
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
    /// Which voice said it, 1, 2, …, when the speaker model told voices
    /// apart; None before that, or when it couldn't tell.
    #[serde(default)]
    pub speaker: Option<i64>,
}

/// A name given to one of a note's voices.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct NoteSpeaker {
    pub speaker: i64,
    pub name: String,
    /// The name came from a voice saved in an earlier meeting, not from you.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub recognised: bool,
}

/// A voice saved when you named someone, so later meetings recognise them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Voice {
    pub id: i64,
    pub name: String,
    /// The clip's file in the voices folder.
    pub file: String,
    /// The clip's length.
    pub ms: i64,
    /// When it last named someone, or was saved (Unix seconds).
    pub used_at: i64,
}

const LINE_COLUMNS: &str = "id, note_id, at_ms, end_ms, who, text, speaker";

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
    summary_at, summary_error, failed_lines, edited_at, outline, attendees,
    (SELECT COUNT(*) FROM note_line l WHERE l.note_id = note.id) AS lines,
    (SELECT COUNT(*) FROM note_failed_line f WHERE f.note_id = note.id) AS retryable";

fn map(row: &Row<'_>) -> rusqlite::Result<Note> {
    let kind: String = row.get("kind")?;
    let status: String = row.get("status")?;
    let decisions: String = row.get("decisions")?;
    let outline: Option<String> = row.get("outline")?;
    let attendees: Option<String> = row.get("attendees")?;
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
        speakers: Vec::new(),
        failed_lines: row.get("failed_lines")?,
        retryable: row.get("retryable")?,
        lines: row.get("lines")?,
        actions: Vec::new(),
        outline: outline
            .and_then(|o| serde_json::from_str(&o).ok())
            .unwrap_or_default(),
        attendees: attendees
            .and_then(|a| serde_json::from_str(&a).ok())
            .unwrap_or_default(),
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
        speaker: row.get("speaker")?,
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
    let mut speakers = conn.prepare(
        "SELECT speaker, name, recognised FROM note_speaker WHERE note_id = ?1 ORDER BY speaker",
    )?;
    note.speakers = speakers
        .query_map(params![note.id], |r| {
            Ok(NoteSpeaker {
                speaker: r.get(0)?,
                name: r.get(1)?,
                recognised: r.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
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
    let mut stmt = conn.prepare(&format!(
        "SELECT {LINE_COLUMNS} FROM note_line WHERE note_id = ?1 ORDER BY at_ms, id"
    ))?;
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
        speaker: None,
    })
}

pub fn remove_line(conn: &Connection, line_id: i64) -> Result<()> {
    conn.execute("DELETE FROM note_line WHERE id = ?1", params![line_id])?;
    Ok(())
}

pub fn line(conn: &Connection, line_id: i64) -> Result<Option<NoteLine>> {
    Ok(conn
        .query_row(
            &format!("SELECT {LINE_COLUMNS} FROM note_line WHERE id = ?1"),
            params![line_id],
            map_line,
        )
        .optional()?)
}

/// A line's corrected words, or who said it.
pub fn update_line(
    conn: &Connection,
    line_id: i64,
    who: Speaker,
    speaker: Option<i64>,
    text: &str,
) -> Result<NoteLine> {
    let text = text.trim();
    if text.is_empty() {
        return Err(anyhow!("A line needs some words; delete it instead"));
    }
    // Given to someone by hand: later passes of the speaker model leave it.
    conn.execute(
        "UPDATE note_line SET speaker_fixed = 1
         WHERE id = ?1 AND (who != ?2 OR speaker IS NOT ?3)",
        params![line_id, who.as_str(), speaker],
    )?;
    conn.execute(
        "UPDATE note_line SET who = ?2, speaker = ?3, text = ?4 WHERE id = ?1",
        params![line_id, who.as_str(), speaker, text],
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
        "INSERT OR REPLACE INTO note_line (id, note_id, at_ms, end_ms, who, text, speaker)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            line.id,
            line.note_id,
            line.at_ms,
            line.end_ms,
            line.who.as_str(),
            line.text,
            line.speaker
        ],
    )?;
    Ok(())
}

/// Lines the speaker model may give to a voice: not given to someone by
/// hand, and on `side` when there is one.
pub fn speaker_lines(
    conn: &Connection,
    note_id: i64,
    side: Option<Speaker>,
) -> Result<Vec<NoteLine>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {LINE_COLUMNS} FROM note_line
         WHERE note_id = ?1 AND speaker_fixed = 0 AND (?2 IS NULL OR who = ?2)
         ORDER BY at_ms, id"
    ))?;
    let lines = stmt
        .query_map(params![note_id, side.map(|s| s.as_str())], map_line)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(lines)
}

/// Who said each line, from the speaker model. Names stay with their
/// numbers; a name whose voice no longer says anything goes.
pub fn set_speakers(
    conn: &mut Connection,
    note_id: i64,
    lines: &[(i64, Option<i64>)],
) -> Result<()> {
    let tx = conn.transaction()?;
    for (line_id, speaker) in lines {
        tx.execute(
            "UPDATE note_line SET speaker = ?3 WHERE id = ?1 AND note_id = ?2",
            params![line_id, note_id, speaker],
        )?;
    }
    tx.execute(
        "DELETE FROM note_speaker WHERE note_id = ?1 AND speaker <> ?2 AND speaker NOT IN
         (SELECT speaker FROM note_line WHERE note_id = ?1 AND speaker IS NOT NULL)",
        params![note_id, THEM_SIDE],
    )?;
    tx.commit()?;
    Ok(())
}

/// Names a voice; an empty name goes back to "Speaker 2".
pub fn name_speaker(conn: &Connection, note_id: i64, speaker: i64, name: &str) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        conn.execute(
            "DELETE FROM note_speaker WHERE note_id = ?1 AND speaker = ?2",
            params![note_id, speaker],
        )?;
    } else {
        conn.execute(
            "INSERT INTO note_speaker (note_id, speaker, name) VALUES (?1, ?2, ?3)
             ON CONFLICT (note_id, speaker)
             DO UPDATE SET name = excluded.name, recognised = 0, voice_id = NULL",
            params![note_id, speaker, name],
        )?;
    }
    Ok(())
}

/// Names a voice after the saved voice it sounds like.
pub fn recognise_speaker(
    conn: &Connection,
    note_id: i64,
    speaker: i64,
    voice: &Voice,
    now: i64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO note_speaker (note_id, speaker, name, recognised, voice_id)
         VALUES (?1, ?2, ?3, 1, ?4)
         ON CONFLICT (note_id, speaker) DO NOTHING",
        params![note_id, speaker, voice.name, voice.id],
    )?;
    conn.execute(
        "UPDATE voice SET used_at = ?2 WHERE id = ?1",
        params![voice.id, now],
    )?;
    Ok(())
}

fn map_voice(r: &rusqlite::Row) -> rusqlite::Result<Voice> {
    Ok(Voice {
        id: r.get("id")?,
        name: r.get("name")?,
        file: r.get("file")?,
        ms: r.get("ms")?,
        used_at: r.get("used_at")?,
    })
}

/// The saved voices, the most recently heard first.
pub fn voices(conn: &Connection) -> Result<Vec<Voice>> {
    let mut stmt = conn.prepare("SELECT * FROM voice ORDER BY used_at DESC, id DESC")?;
    let voices = stmt
        .query_map([], map_voice)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(voices)
}

pub fn voice_named(conn: &Connection, name: &str) -> Result<Option<Voice>> {
    Ok(conn
        .query_row(
            "SELECT * FROM voice WHERE name = ?1",
            params![name.trim()],
            map_voice,
        )
        .optional()?)
}

/// Saves a voice under a name, or gives a saved one a new clip. Returns
/// the clip file it replaced, to remove.
pub fn save_voice(
    conn: &Connection,
    name: &str,
    file: &str,
    ms: i64,
    now: i64,
) -> Result<Option<String>> {
    let old = voice_named(conn, name)?;
    conn.execute(
        "INSERT INTO voice (name, file, ms, created_at, used_at) VALUES (?1, ?2, ?3, ?4, ?4)
         ON CONFLICT (name) DO UPDATE SET file = excluded.file, ms = excluded.ms, used_at = excluded.used_at",
        params![name.trim(), file, ms, now],
    )?;
    Ok(old.map(|v| v.file).filter(|f| f != file))
}

/// Forgets a saved voice; returns its clip file, to remove.
pub fn delete_voice(conn: &Connection, id: i64) -> Result<Option<String>> {
    let file = conn
        .query_row("SELECT file FROM voice WHERE id = ?1", params![id], |r| {
            r.get(0)
        })
        .optional()?;
    conn.execute("DELETE FROM voice WHERE id = ?1", params![id])?;
    conn.execute(
        "UPDATE note_speaker SET voice_id = NULL WHERE voice_id = ?1",
        params![id],
    )?;
    Ok(file)
}

/// Lines by the other speaker that overlap `[from_ms, to_ms]`.
pub fn lines_near(
    conn: &Connection,
    note_id: i64,
    who: Speaker,
    from_ms: i64,
    to_ms: i64,
) -> Result<Vec<NoteLine>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {LINE_COLUMNS} FROM note_line
         WHERE note_id = ?1 AND who = ?2 AND end_ms >= ?3 AND at_ms <= ?4"
    ))?;
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
            summary_error = NULL, outline = ?6 WHERE id = ?1",
        params![
            id,
            summary.summary,
            serde_json::to_string(&summary.decisions)?,
            model,
            at,
            serde_json::to_string(&summary.outline)?
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

/// The people invited to the meeting the note began with.
pub fn set_attendees(conn: &Connection, id: i64, people: &[String]) -> Result<()> {
    conn.execute(
        "UPDATE note SET attendees = ?2 WHERE id = ?1",
        params![id, serde_json::to_string(people)?],
    )?;
    Ok(())
}

/// Saves the summary's outline on its own: the kind you chose.
pub fn set_outline(conn: &Connection, id: i64, outline: &NoteOutline) -> Result<()> {
    conn.execute(
        "UPDATE note SET outline = ?2 WHERE id = ?1",
        params![id, serde_json::to_string(outline)?],
    )?;
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
                    &format!("SELECT {LINE_COLUMNS} FROM note_line WHERE id = ?1"),
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
    for table in [
        "note_line",
        "note_tag",
        "note_action",
        "note_failed_line",
        "note_speaker",
    ] {
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
            summary_at, summary_error, failed_lines, edited_at, outline, attendees)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18,
            ?19, ?20, ?21)",
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
            n.edited_at,
            serde_json::to_string(&n.outline)?,
            serde_json::to_string(&n.attendees)?
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
            "INSERT INTO note_line (id, note_id, at_ms, end_ms, who, text, speaker)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                l.id,
                n.id,
                l.at_ms,
                l.end_ms,
                l.who.as_str(),
                l.text,
                l.speaker
            ],
        )?;
    }
    for s in &n.speakers {
        tx.execute(
            "INSERT INTO note_speaker (note_id, speaker, name, recognised) VALUES (?1, ?2, ?3, ?4)",
            params![n.id, s.speaker, s.name, s.recognised],
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

/// Finished notes started before `before` (Unix seconds) that still have
/// audio, with its file name.
pub fn audio_started_before(conn: &Connection, before: i64) -> Result<Vec<(i64, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, audio_file FROM note
         WHERE audio_file IS NOT NULL AND status = 'ready' AND started_at < ?1",
    )?;
    let rows = stmt
        .query_map([before], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// Forgets a note's audio file; the transcript and the rest stay.
pub fn clear_audio(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("UPDATE note SET audio_file = NULL WHERE id = ?1", [id])?;
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

/// The speaker number a name for the whole other side goes under: the
/// people on a call before (or without) the speaker model tells them apart.
pub const THEM_SIDE: i64 = -1;

/// What a line's speaker is called: their name, "Speaker 2" (`numbered`
/// with `{n}`), the other side's name, or You or Them.
pub fn speaker_label(
    note: &Note,
    line: &NoteLine,
    you: &str,
    them: &str,
    numbered: &str,
) -> String {
    match (line.who, line.speaker) {
        (_, Some(n)) => note
            .speakers
            .iter()
            .find(|s| s.speaker == n)
            .map(|s| s.name.clone())
            .unwrap_or_else(|| numbered.replace("{n}", &n.to_string())),
        (Speaker::You, None) => you.to_string(),
        (Speaker::Them, None) => note
            .speakers
            .iter()
            .find(|s| s.speaker == THEM_SIDE)
            .map(|s| s.name.clone())
            .unwrap_or_else(|| them.to_string()),
    }
}

/// The note as Markdown, for Copy and Export.
/// A decision or task written "Title: what it means" as (title, rest);
/// anything else is all rest.
pub fn split_title(text: &str) -> (Option<&str>, &str) {
    match text.split_once(": ") {
        Some((title, rest))
            if !rest.trim().is_empty()
                && title.chars().count() <= 60
                && !title.contains(['.', '!', '?']) =>
        {
            (Some(title.trim()), rest.trim())
        }
        _ => (None, text.trim()),
    }
}

/// Everyone the note names: who was invited, then who the summary found.
pub fn people(note: &Note) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for name in note.attendees.iter().chain(&note.outline.people) {
        let name = name.trim();
        // "Priya" from the talk is the "Priya Shah" who was invited.
        let known = out.iter().any(|n| {
            n.eq_ignore_ascii_case(name)
                || n.split(' ')
                    .next()
                    .is_some_and(|first| first.eq_ignore_ascii_case(name))
        });
        if !name.is_empty() && !known {
            out.push(name.to_string());
        }
    }
    out
}

pub fn markdown(note: &Note, lines: &[NoteLine], you: &str, them: &str) -> String {
    let mut out = format!("# {}\n\n", note.title);
    let people = people(note);
    if !people.is_empty() {
        out.push_str(&format!("People: {}\n\n", people.join(", ")));
    }
    if !note.tags.is_empty() {
        out.push_str(&format!("Tags: {}\n\n", note.tags.join(", ")));
    }
    if !note.summary.is_empty() {
        out.push_str(&format!("## Summary\n\n{}\n\n", note.summary));
        for t in &note.outline.topics {
            out.push_str(&format!("**{}**  \n{}\n\n", t.heading, t.text));
        }
    }
    if !note.decisions.is_empty() {
        out.push_str("## Decisions\n\n");
        for d in &note.decisions {
            match split_title(d) {
                (Some(title), rest) => out.push_str(&format!("- **{title}**: {rest}\n")),
                (None, rest) => out.push_str(&format!("- {rest}\n")),
            }
        }
        out.push('\n');
    }
    for section in &note.outline.sections {
        out.push_str(&format!("## {}\n\n", section.heading));
        for point in &section.points {
            out.push_str(&format!("- {point}\n"));
        }
        out.push('\n');
    }
    if !note.actions.is_empty() {
        out.push_str("## Next steps\n\n");
        for a in &note.actions {
            let mark = if a.done { "x" } else { " " };
            let text = match split_title(&a.text) {
                (Some(title), rest) => format!("**{title}**: {rest}"),
                (None, rest) => rest.to_string(),
            };
            out.push_str(&format!("- [{mark}] [{}] {text}\n", a.owner));
        }
        out.push('\n');
    }
    if !note.outline.details.is_empty() {
        out.push_str("## Details\n\n");
        for d in &note.outline.details {
            let at = d
                .at_ms
                .map(|ms| format!(" ({})", clock(ms)))
                .unwrap_or_default();
            out.push_str(&format!("**{}**: {}{at}\n\n", d.heading, d.text));
        }
    }
    if !note.my_notes.trim().is_empty() {
        out.push_str(&format!("## My notes\n\n{}\n\n", note.my_notes.trim()));
    }
    if !lines.is_empty() {
        out.push_str("## Transcript\n\n");
        for l in lines {
            let who = speaker_label(note, l, you, them, "Speaker {n}");
            out.push_str(&format!("{} {}: {}\n", clock(l.at_ms), who, l.text));
        }
    }
    out.trim_end().to_string() + "\n"
}

/// The note as HTML for pasting into Google Docs or Word, which keep its
/// headings, lists and bold titles. The transcript stays out: it belongs
/// in the export, not in notes someone reads.
pub fn html(note: &Note, when: &str) -> String {
    document::html(&blocks(note, when, None))
}

/// The note as a Word file, transcript included, for Export and for the
/// copy kept in a folder (Google Drive's, say).
pub fn docx(note: &Note, when: &str, lines: &[NoteLine], you: &str, them: &str) -> Result<Vec<u8>> {
    document::docx(&blocks(note, when, Some((lines, you, them))))
}

/// The note's parts in reading order: the summary and its topics, then
/// decisions, next steps, details, your own notes and, when given, the
/// transcript.
fn blocks(note: &Note, when: &str, transcript: Option<(&[NoteLine], &str, &str)>) -> Vec<Block> {
    let mut out = vec![Block::Title(note.title.clone())];
    let people = people(note);
    let mut meta = vec![when.to_string()];
    if !people.is_empty() {
        meta.push(people.join(", "));
    }
    out.push(Block::Para(vec![Run::italic(meta.join(" · "))]));
    if !note.summary.is_empty() {
        out.push(Block::Heading("Summary".into()));
        out.push(Block::Para(vec![Run::plain(note.summary.as_str())]));
        for t in &note.outline.topics {
            out.push(Block::Para(vec![
                Run::bold(t.heading.as_str()),
                Run::plain(format!("\n{}", t.text)),
            ]));
        }
    }
    let item = |lead: String, text: &str| match split_title(text) {
        (Some(title), rest) => vec![
            Run::plain(lead),
            Run::bold(title),
            Run::plain(format!(": {rest}")),
        ],
        (None, rest) => vec![Run::plain(format!("{lead}{rest}"))],
    };
    let tidy =
        |runs: Vec<Run>| -> Vec<Run> { runs.into_iter().filter(|r| !r.text.is_empty()).collect() };
    if !note.decisions.is_empty() {
        out.push(Block::Heading("Decisions".into()));
        for d in &note.decisions {
            out.push(Block::Bullet(tidy(item(String::new(), d))));
        }
    }
    for section in &note.outline.sections {
        out.push(Block::Heading(section.heading.clone()));
        for point in &section.points {
            out.push(Block::Bullet(vec![Run::plain(point.as_str())]));
        }
    }
    if !note.actions.is_empty() {
        out.push(Block::Heading("Next steps".into()));
        for a in &note.actions {
            let mark = if a.done { "☑" } else { "☐" };
            out.push(Block::Bullet(tidy(item(
                format!("{mark} [{}] ", a.owner),
                &a.text,
            ))));
        }
    }
    if !note.outline.details.is_empty() {
        out.push(Block::Heading("Details".into()));
        for d in &note.outline.details {
            let at = d
                .at_ms
                .map(|ms| format!(" ({})", clock(ms)))
                .unwrap_or_default();
            out.push(Block::Para(vec![
                Run::bold(d.heading.as_str()),
                Run::plain(format!(": {}{at}", d.text)),
            ]));
        }
    }
    if !note.my_notes.trim().is_empty() {
        out.push(Block::Heading("My notes".into()));
        for para in note.my_notes.trim().split("\n\n") {
            out.push(Block::Para(vec![Run::plain(para)]));
        }
    }
    if let Some((lines, you, them)) = transcript.filter(|(lines, ..)| !lines.is_empty()) {
        out.push(Block::Heading("Transcript".into()));
        for l in lines {
            let who = speaker_label(note, l, you, them, "Speaker {n}");
            out.push(Block::Para(vec![
                Run::plain(format!("{} ", clock(l.at_ms))),
                Run::bold(format!("{who}:")),
                Run::plain(format!(" {}", l.text)),
            ]));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_split_off_decisions_and_tasks() {
        assert_eq!(
            split_title("Switch the workers: the code predates the new builds."),
            (
                Some("Switch the workers"),
                "the code predates the new builds."
            )
        );
        assert_eq!(
            split_title("Ship it. Then: rest"),
            (None, "Ship it. Then: rest")
        );
        assert_eq!(split_title("Plain task"), (None, "Plain task"));
    }

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
                ..Default::default()
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
    fn old_audio_is_found_and_forgotten() {
        let conn = db();
        let note = meeting(&conn);
        finish_recording(&conn, note.id, 60_000, Some("note-1.wav"), 0).unwrap();
        // Still recording counts as unfinished, so it isn't offered yet.
        assert!(audio_started_before(&conn, note.started_at + 1)
            .unwrap()
            .is_empty());
        recover_unfinished(&conn).unwrap();
        assert!(audio_started_before(&conn, note.started_at)
            .unwrap()
            .is_empty());
        assert_eq!(
            audio_started_before(&conn, note.started_at + 1).unwrap(),
            vec![(note.id, "note-1.wav".to_string())]
        );
        clear_audio(&conn, note.id).unwrap();
        let kept = get(&conn, note.id).unwrap().unwrap();
        assert_eq!(kept.audio_file, None);
        assert_eq!(kept.title, "Infra sync");
        assert!(audio_started_before(&conn, i64::MAX).unwrap().is_empty());
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
    fn copied_notes_keep_their_shape_and_word_files_add_the_transcript() {
        let conn = db();
        let mut note = meeting(&conn);
        let line = add_line(&conn, note.id, 65_000, 67_000, Speaker::Them, "Ship <it>.").unwrap();
        note.summary = "Workers move to Cloudflare.".into();
        note.outline.topics = vec![crate::notes_rules::OutlineTopic {
            heading: "Workers".into(),
            text: "They predate the builds.".into(),
        }];
        note.outline.details = vec![crate::notes_rules::OutlineDetail {
            heading: "Builds".into(),
            text: "Twice as fast".into(),
            at_ms: Some(65_000),
        }];
        note.decisions = vec!["Switch: on Friday.".into(), "Plain one".into()];
        note.actions = vec![NoteAction {
            id: 1,
            owner: "You".into(),
            text: "Write it up: with images.".into(),
            done: true,
        }];
        note.my_notes = "First\nline\n\nSecond".into();
        assert_eq!(
            html(&note, "Oct 17"),
            "<h1>Infra sync</h1><p><i>Oct 17</i></p><h2>Summary</h2><p>Workers move to Cloudflare.</p>\
             <p><b>Workers</b><br>They predate the builds.</p><h2>Decisions</h2>\
             <ul><li><b>Switch</b>: on Friday.</li><li>Plain one</li></ul><h2>Next steps</h2>\
             <ul><li>☑ [You] <b>Write it up</b>: with images.</li></ul><h2>Details</h2>\
             <p><b>Builds</b>: Twice as fast (1:05)</p><h2>My notes</h2><p>First<br>line</p><p>Second</p>"
        );
        let bytes = docx(&note, "Oct 17", &[line], "You", "Them").unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut doc = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("word/document.xml").unwrap(), &mut doc)
            .unwrap();
        assert!(doc.contains(">Transcript</w:t>"));
        assert!(doc.contains(">Them:</w:t>"));
        assert!(doc.contains("> Ship &lt;it&gt;.</w:t>"));
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
        let fixed =
            update_line(&conn, line.id, Speaker::You, Some(2), " ship Kubernetes ").unwrap();
        assert_eq!(fixed.text, "ship Kubernetes");
        assert_eq!(fixed.who, Speaker::You);
        assert_eq!(fixed.speaker, Some(2));
        assert!(update_line(&conn, line.id, Speaker::You, None, "  ").is_err());

        remove_line(&conn, line.id).unwrap();
        assert!(super::line(&conn, line.id).unwrap().is_none());
        restore_line(&conn, &fixed).unwrap();
        assert_eq!(lines(&conn, note.id).unwrap(), vec![fixed]);
    }

    #[test]
    fn voices_are_told_apart_named_and_survive_undo() {
        let mut conn = db();
        let note = meeting(&conn);
        let a = add_line(&conn, note.id, 0, 2_000, Speaker::Them, "shall we start").unwrap();
        let b = add_line(&conn, note.id, 2_000, 4_000, Speaker::Them, "yes please").unwrap();
        let c = add_line(&conn, note.id, 4_000, 6_000, Speaker::You, "go ahead").unwrap();
        set_speakers(&mut conn, note.id, &[(a.id, Some(1)), (b.id, Some(2))]).unwrap();
        name_speaker(&conn, note.id, 2, " Priya ").unwrap();

        let note = get(&conn, note.id).unwrap().unwrap();
        let lines = lines(&conn, note.id).unwrap();
        let label = |l: &NoteLine| speaker_label(&note, l, "You", "Them", "Speaker {n}");
        assert_eq!(
            lines.iter().map(label).collect::<Vec<_>>(),
            ["Speaker 1", "Priya", "You"]
        );
        assert_eq!(lines[2].id, c.id);

        // Undo of a delete brings the names back.
        let deleted = delete(&mut conn, note.id).unwrap().unwrap();
        assert!(get(&conn, note.id).unwrap().is_none());
        restore(&mut conn, &deleted).unwrap();
        let back = get(&conn, note.id).unwrap().unwrap();
        assert_eq!(back.speakers, note.speakers);
        assert_eq!(super::lines(&conn, note.id).unwrap(), lines);

        // A line given to someone by hand is left out of the next pass.
        update_line(&conn, a.id, Speaker::Them, Some(2), "shall we start").unwrap();
        let open: Vec<i64> = speaker_lines(&conn, note.id, Some(Speaker::Them))
            .unwrap()
            .iter()
            .map(|l| l.id)
            .collect();
        assert_eq!(open, vec![b.id]);
        // A pass that no longer hears Priya drops her name; numbers keep theirs.
        set_speakers(&mut conn, note.id, &[(b.id, Some(1))]).unwrap();
        assert_eq!(get(&conn, note.id).unwrap().unwrap().speakers.len(), 1);
        set_speakers(&mut conn, note.id, &[(a.id, None), (b.id, None)]).unwrap();
        assert!(get(&conn, note.id).unwrap().unwrap().speakers.is_empty());

        set_speakers(&mut conn, note.id, &[(b.id, Some(1))]).unwrap();
        name_speaker(&conn, note.id, 1, "Sam").unwrap();
        name_speaker(&conn, note.id, 1, "").unwrap();
        assert!(get(&conn, note.id).unwrap().unwrap().speakers.is_empty());
    }

    #[test]
    fn the_other_side_can_be_named_before_its_voices_are_told_apart() {
        let mut conn = db();
        let note = meeting(&conn);
        let a = add_line(&conn, note.id, 0, 2_000, Speaker::Them, "hi there").unwrap();
        add_line(&conn, note.id, 2_000, 4_000, Speaker::You, "hello").unwrap();
        name_speaker(&conn, note.id, THEM_SIDE, "Alex").unwrap();
        let label = |conn: &Connection, note: &Note| {
            lines(conn, note.id)
                .unwrap()
                .iter()
                .map(|l| speaker_label(note, l, "You", "Them", "Speaker {n}"))
                .collect::<Vec<_>>()
        };
        let named = get(&conn, note.id).unwrap().unwrap();
        assert_eq!(label(&conn, &named), ["Alex", "You"]);
        // A speaker pass keeps the side's name; a voice it finds is numbered.
        set_speakers(&mut conn, note.id, &[(a.id, Some(1))]).unwrap();
        let passed = get(&conn, note.id).unwrap().unwrap();
        assert!(passed.speakers.iter().any(|s| s.speaker == THEM_SIDE));
        assert_eq!(label(&conn, &passed), ["Speaker 1", "You"]);
        name_speaker(&conn, note.id, THEM_SIDE, "").unwrap();
        set_speakers(&mut conn, note.id, &[(a.id, None)]).unwrap();
        let cleared = get(&conn, note.id).unwrap().unwrap();
        assert_eq!(label(&conn, &cleared), ["Them", "You"]);
    }

    #[test]
    fn named_voices_are_saved_recognised_and_forgotten() {
        let mut conn = db();
        let note = meeting(&conn);
        let a = add_line(&conn, note.id, 0, 4_000, Speaker::Them, "morning all").unwrap();
        set_speakers(&mut conn, note.id, &[(a.id, Some(1))]).unwrap();

        assert_eq!(
            save_voice(&conn, "Priya", "v1.wav", 6_000, 100).unwrap(),
            None
        );
        // The same name, any case, gets the new clip; the old file goes.
        assert_eq!(
            save_voice(&conn, "priya", "v2.wav", 8_000, 200).unwrap(),
            Some("v1.wav".to_string())
        );
        let saved = voices(&conn).unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!((saved[0].file.as_str(), saved[0].ms), ("v2.wav", 8_000));

        recognise_speaker(&conn, note.id, 1, &saved[0], 300).unwrap();
        let named = get(&conn, note.id).unwrap().unwrap().speakers;
        assert_eq!(named[0].name, "Priya");
        assert!(named[0].recognised);
        assert_eq!(voices(&conn).unwrap()[0].used_at, 300);
        // A name you give wins over a recognised one, and isn't overwritten.
        name_speaker(&conn, note.id, 1, "Priya S").unwrap();
        recognise_speaker(&conn, note.id, 1, &saved[0], 400).unwrap();
        let named = get(&conn, note.id).unwrap().unwrap().speakers;
        assert_eq!(
            (named[0].name.as_str(), named[0].recognised),
            ("Priya S", false)
        );

        assert_eq!(
            delete_voice(&conn, saved[0].id).unwrap(),
            Some("v2.wav".to_string())
        );
        assert!(voices(&conn).unwrap().is_empty());
    }
}
