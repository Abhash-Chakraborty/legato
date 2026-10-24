//! The Scratchpad's tables: notes, which of them are open as tabs in the
//! window, and earlier versions of each. Plain rusqlite, like
//! `snippet_store`; `ScratchpadManager` wraps it with events.

use anyhow::{anyhow, Result};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use specta::Type;

/// Tabs the window holds; opening another closes the leftmost.
pub const MAX_TABS: usize = 5;
/// Versions kept per note; older ones are dropped.
pub const MAX_VERSIONS: usize = 50;
/// A title made from the text is cut here.
const TITLE_CHARS: usize = 60;

/// Where a note came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum NoteSource {
    /// Dictated with the Scratchpad shortcut.
    Dictation,
    /// A paste that failed, saved here instead.
    FailedPaste,
}

impl NoteSource {
    fn as_str(self) -> &'static str {
        match self {
            NoteSource::Dictation => "dictation",
            NoteSource::FailedPaste => "failed_paste",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "dictation" => Some(NoteSource::Dictation),
            "failed_paste" => Some(NoteSource::FailedPaste),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ScratchNote {
    pub id: i64,
    /// Empty until the note has text; the window shows "Untitled".
    pub title: String,
    /// Whether the title follows the first line.
    pub auto_title: bool,
    pub text: String,
    pub pinned: bool,
    /// Position among the window's tabs, or None when closed.
    pub tab: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub source: Option<NoteSource>,
    /// For a failed paste: the app it was going to, and why it failed.
    pub source_app: Option<String>,
    pub source_app_path: Option<String>,
    pub reason: Option<String>,
    /// Versions kept, the current text included.
    pub versions: i64,
}

#[derive(Clone, Debug, Default)]
pub struct NewNote {
    pub text: String,
    pub source: Option<NoteSource>,
    pub source_app: Option<String>,
    pub source_app_path: Option<String>,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ScratchVersion {
    pub id: i64,
    pub text: String,
    pub at: i64,
}

/// A deleted note with its versions, for Undo.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct DeletedNote {
    pub note: ScratchNote,
    pub versions: Vec<ScratchVersion>,
}

const COLUMNS: &str = "id, title, auto_title, text, pinned, tab, created_at, updated_at, source,
    source_app, source_app_path, reason,
    (SELECT COUNT(*) FROM scratch_version v WHERE v.note_id = scratch_note.id) + 1 AS versions";

fn map(row: &Row<'_>) -> rusqlite::Result<ScratchNote> {
    let source: Option<String> = row.get("source")?;
    Ok(ScratchNote {
        id: row.get("id")?,
        title: row.get("title")?,
        auto_title: row.get("auto_title")?,
        text: row.get("text")?,
        pinned: row.get("pinned")?,
        tab: row.get("tab")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        source: source.as_deref().and_then(NoteSource::parse),
        source_app: row.get("source_app")?,
        source_app_path: row.get("source_app_path")?,
        reason: row.get("reason")?,
        versions: row.get("versions")?,
    })
}

/// The first line with text, without Markdown heading or list marks.
pub fn title_from(text: &str) -> String {
    let line = text
        .lines()
        .map(|l| l.trim().trim_start_matches(['#', '-', '*', '>']).trim())
        .find(|l| !l.is_empty())
        .unwrap_or("");
    if line.chars().count() <= TITLE_CHARS {
        return line.to_string();
    }
    let cut: String = line.chars().take(TITLE_CHARS).collect();
    let cut = match cut.rfind(' ') {
        Some(i) if i > TITLE_CHARS / 2 => cut[..i].to_string(),
        _ => cut,
    };
    format!("{}…", cut.trim_end())
}

pub fn get(conn: &Connection, id: i64) -> Result<Option<ScratchNote>> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM scratch_note WHERE id = ?1"),
            [id],
            map,
        )
        .optional()?)
}

fn must(conn: &Connection, id: i64) -> Result<ScratchNote> {
    get(conn, id)?.ok_or_else(|| anyhow!("That note was deleted."))
}

/// Last edited first.
pub fn list(conn: &Connection) -> Result<Vec<ScratchNote>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM scratch_note ORDER BY updated_at DESC, id DESC"
    ))?;
    let rows = stmt.query_map([], map)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// The window's tabs, left to right.
pub fn tabs(conn: &Connection) -> Result<Vec<ScratchNote>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM scratch_note WHERE tab IS NOT NULL ORDER BY tab, id"
    ))?;
    let rows = stmt.query_map([], map)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn renumber(conn: &Connection, ids: &[i64]) -> Result<()> {
    conn.execute(
        "UPDATE scratch_note SET tab = NULL WHERE tab IS NOT NULL",
        [],
    )?;
    for (i, id) in ids.iter().enumerate() {
        conn.execute(
            "UPDATE scratch_note SET tab = ?1 WHERE id = ?2",
            params![i as i64, id],
        )?;
    }
    Ok(())
}

/// Opens a note as the rightmost tab, closing the leftmost when the window
/// is full. A note already open stays where it is.
pub fn open_tab(conn: &mut Connection, id: i64) -> Result<()> {
    let tx = conn.transaction()?;
    must(&tx, id)?;
    let mut ids: Vec<i64> = tabs(&tx)?.into_iter().map(|n| n.id).collect();
    if !ids.contains(&id) {
        ids.push(id);
        while ids.len() > MAX_TABS {
            ids.remove(0);
        }
        renumber(&tx, &ids)?;
    }
    tx.commit()?;
    Ok(())
}

pub fn close_tab(conn: &mut Connection, id: i64) -> Result<()> {
    let tx = conn.transaction()?;
    let ids: Vec<i64> = tabs(&tx)?
        .into_iter()
        .map(|n| n.id)
        .filter(|&n| n != id)
        .collect();
    renumber(&tx, &ids)?;
    tx.commit()?;
    Ok(())
}

/// Removes notes nobody wrote in: no text, no name, not pinned, no versions
/// and not open in a tab. Kept, they pile up as "Untitled" rows in the list.
pub fn prune_blank(conn: &Connection) -> Result<usize> {
    Ok(conn.execute(
        "DELETE FROM scratch_note
         WHERE tab IS NULL AND pinned = 0 AND title = ''
           AND trim(text, ' ' || char(9) || char(10) || char(13)) = ''
           AND id NOT IN (SELECT note_id FROM scratch_version)",
        [],
    )?)
}

/// Moves an open tab to a position, counted from the left.
pub fn move_tab(conn: &mut Connection, id: i64, to: usize) -> Result<()> {
    let tx = conn.transaction()?;
    let mut ids: Vec<i64> = tabs(&tx)?.into_iter().map(|n| n.id).collect();
    if let Some(from) = ids.iter().position(|&n| n == id) {
        ids.remove(from);
        ids.insert(to.min(ids.len()), id);
        renumber(&tx, &ids)?;
    }
    tx.commit()?;
    Ok(())
}

/// Adds a note and opens it as a tab.
pub fn create(conn: &mut Connection, new: &NewNote, now: i64) -> Result<ScratchNote> {
    let id = {
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO scratch_note (title, text, created_at, updated_at, source, source_app,
                source_app_path, reason)
             VALUES (?1, ?2, ?3, ?3, ?4, ?5, ?6, ?7)",
            params![
                title_from(&new.text),
                new.text,
                now,
                new.source.map(NoteSource::as_str),
                new.source_app,
                new.source_app_path,
                new.reason,
            ],
        )?;
        let id = tx.last_insert_rowid();
        tx.commit()?;
        id
    };
    open_tab(conn, id)?;
    must(conn, id)
}

fn keep_version(conn: &Connection, id: i64, text: &str, at: i64) -> Result<()> {
    conn.execute(
        "INSERT INTO scratch_version (note_id, text, at) VALUES (?1, ?2, ?3)",
        params![id, text, at],
    )?;
    // The current text counts as one of the versions kept.
    conn.execute(
        "DELETE FROM scratch_version WHERE note_id = ?1 AND id NOT IN (
            SELECT id FROM scratch_version WHERE note_id = ?1 ORDER BY at DESC, id DESC LIMIT ?2
         )",
        params![id, (MAX_VERSIONS - 1) as i64],
    )?;
    Ok(())
}

/// Saves a note's text, keeping what it said before as a version.
pub fn save_text(conn: &mut Connection, id: i64, text: &str, now: i64) -> Result<ScratchNote> {
    let tx = conn.transaction()?;
    let before = must(&tx, id)?;
    if before.text != text {
        if !before.text.trim().is_empty() {
            keep_version(&tx, id, &before.text, before.updated_at)?;
        }
        let title = if before.auto_title {
            title_from(text)
        } else {
            before.title.clone()
        };
        tx.execute(
            "UPDATE scratch_note SET text = ?1, title = ?2, updated_at = ?3 WHERE id = ?4",
            params![text, title, now, id],
        )?;
    }
    let note = must(&tx, id)?;
    tx.commit()?;
    Ok(note)
}

/// Adds dictated text at the end, on a new line when the note has text.
pub fn append(conn: &mut Connection, id: i64, text: &str, now: i64) -> Result<ScratchNote> {
    let before = must(conn, id)?;
    let joined = if before.text.trim().is_empty() {
        text.to_string()
    } else if before.text.ends_with('\n') {
        format!("{}{}", before.text, text)
    } else {
        format!("{}\n{}", before.text, text)
    };
    save_text(conn, id, &joined, now)
}

/// Renames a note; an empty name goes back to following the first line.
pub fn rename(conn: &Connection, id: i64, title: &str) -> Result<ScratchNote> {
    let note = must(conn, id)?;
    let title = title.trim();
    let (title, auto) = if title.is_empty() {
        (title_from(&note.text), true)
    } else {
        (title.chars().take(120).collect::<String>(), false)
    };
    conn.execute(
        "UPDATE scratch_note SET title = ?1, auto_title = ?2 WHERE id = ?3",
        params![title, auto, id],
    )?;
    must(conn, id)
}

pub fn set_pinned(conn: &Connection, id: i64, pinned: bool) -> Result<ScratchNote> {
    conn.execute(
        "UPDATE scratch_note SET pinned = ?1 WHERE id = ?2",
        params![pinned, id],
    )?;
    must(conn, id)
}

/// Clears a failed paste's message once it's been read.
pub fn dismiss_reason(conn: &Connection, id: i64) -> Result<ScratchNote> {
    conn.execute("UPDATE scratch_note SET reason = NULL WHERE id = ?1", [id])?;
    must(conn, id)
}

/// Earlier versions, newest first. The current text isn't among them.
pub fn versions(conn: &Connection, id: i64) -> Result<Vec<ScratchVersion>> {
    let mut stmt = conn.prepare(
        "SELECT id, text, at FROM scratch_version WHERE note_id = ?1 ORDER BY at DESC, id DESC",
    )?;
    let rows = stmt.query_map([id], |r| {
        Ok(ScratchVersion {
            id: r.get(0)?,
            text: r.get(1)?,
            at: r.get(2)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Puts an earlier version back; the text it replaces becomes a version.
pub fn restore_version(
    conn: &mut Connection,
    id: i64,
    version_id: i64,
    now: i64,
) -> Result<ScratchNote> {
    let text: String = conn
        .query_row(
            "SELECT text FROM scratch_version WHERE id = ?1 AND note_id = ?2",
            params![version_id, id],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(|| anyhow!("That version is gone."))?;
    save_text(conn, id, &text, now)
}

pub fn delete(conn: &mut Connection, ids: &[i64]) -> Result<Vec<DeletedNote>> {
    let tx = conn.transaction()?;
    let mut gone = Vec::new();
    for &id in ids {
        if let Some(note) = get(&tx, id)? {
            gone.push(DeletedNote {
                versions: versions(&tx, id)?,
                note,
            });
        }
        tx.execute("DELETE FROM scratch_version WHERE note_id = ?1", [id])?;
        tx.execute("DELETE FROM scratch_note WHERE id = ?1", [id])?;
    }
    let open: Vec<i64> = tabs(&tx)?.into_iter().map(|n| n.id).collect();
    renumber(&tx, &open)?;
    tx.commit()?;
    Ok(gone)
}

/// Undo for `delete`: the notes come back with their ids and versions. Tabs
/// they had come back where there's room.
pub fn restore(conn: &mut Connection, deleted: &[DeletedNote]) -> Result<()> {
    let tx = conn.transaction()?;
    for d in deleted {
        let n = &d.note;
        tx.execute(
            "INSERT OR REPLACE INTO scratch_note (id, title, auto_title, text, pinned, tab,
                created_at, updated_at, source, source_app, source_app_path, reason)
             VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                n.id,
                n.title,
                n.auto_title,
                n.text,
                n.pinned,
                n.created_at,
                n.updated_at,
                n.source.map(NoteSource::as_str),
                n.source_app,
                n.source_app_path,
                n.reason,
            ],
        )?;
        for v in &d.versions {
            tx.execute(
                "INSERT OR REPLACE INTO scratch_version (id, note_id, text, at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![v.id, n.id, v.text, v.at],
            )?;
        }
    }
    let mut open: Vec<(i64, i64)> = tabs(&tx)?
        .into_iter()
        .map(|n| (n.tab.unwrap_or(0), n.id))
        .collect();
    for d in deleted {
        if let Some(tab) = d.note.tab {
            open.push((tab, d.note.id));
        }
    }
    open.sort();
    let ids: Vec<i64> = open.into_iter().map(|(_, id)| id).take(MAX_TABS).collect();
    renumber(&tx, &ids)?;
    tx.commit()?;
    Ok(())
}

/// The most recently edited pinned note.
pub fn last_pinned(conn: &Connection) -> Result<Option<ScratchNote>> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT {COLUMNS} FROM scratch_note WHERE pinned = 1
                 ORDER BY updated_at DESC, id DESC LIMIT 1"
            ),
            [],
            map,
        )
        .optional()?)
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

    fn note(conn: &mut Connection, text: &str, now: i64) -> ScratchNote {
        create(
            conn,
            &NewNote {
                text: text.into(),
                ..Default::default()
            },
            now,
        )
        .unwrap()
    }

    #[test]
    fn closed_blank_notes_are_pruned() {
        let mut conn = db();
        let blank = note(&mut conn, "", 1);
        let spaces = note(&mut conn, " \n ", 2);
        let open = note(&mut conn, "", 3);
        let written = note(&mut conn, "Datev integration", 4);
        let pinned = note(&mut conn, "", 5);
        set_pinned(&conn, pinned.id, true).unwrap();
        for id in [blank.id, spaces.id, written.id, pinned.id] {
            close_tab(&mut conn, id).unwrap();
        }
        assert_eq!(prune_blank(&conn).unwrap(), 2);
        let left: Vec<i64> = list(&conn).unwrap().into_iter().map(|n| n.id).collect();
        assert!(left.contains(&open.id) && left.contains(&written.id) && left.contains(&pinned.id));
        assert!(!left.contains(&blank.id) && !left.contains(&spaces.id));
    }

    #[test]
    fn titles_follow_the_first_line() {
        assert_eq!(title_from("\n## Open questions\n- CSV?"), "Open questions");
        assert_eq!(title_from(""), "");
        let long = "word ".repeat(30);
        assert!(title_from(&long).ends_with('…'));
        assert!(title_from(&long).chars().count() <= TITLE_CHARS + 1);
    }

    #[test]
    fn saving_keeps_versions_and_titles() {
        let mut conn = db();
        let n = note(&mut conn, "", 10);
        assert_eq!(n.versions, 1);
        let n = save_text(&mut conn, n.id, "# Datev\nopen questions", 20).unwrap();
        assert_eq!(n.title, "Datev");
        // An empty note leaves no version behind.
        assert_eq!(n.versions, 1);
        let n = save_text(&mut conn, n.id, "# Datev export\nCSV or EXTF", 30).unwrap();
        assert_eq!(n.versions, 2);
        assert_eq!(n.title, "Datev export");
        // The same text again isn't a new version.
        let n = save_text(&mut conn, n.id, "# Datev export\nCSV or EXTF", 40).unwrap();
        assert_eq!(n.versions, 2);
        let n = rename(&conn, n.id, "Karol").unwrap();
        let n = save_text(&mut conn, n.id, "# Something else", 50).unwrap();
        assert_eq!(n.title, "Karol");
        let v = versions(&conn, n.id).unwrap();
        assert_eq!(v[0].text, "# Datev export\nCSV or EXTF");
        let n = restore_version(&mut conn, n.id, v[0].id, 60).unwrap();
        assert_eq!(n.text, "# Datev export\nCSV or EXTF");
        assert_eq!(versions(&conn, n.id).unwrap()[0].text, "# Something else");
    }

    #[test]
    fn versions_are_capped() {
        let mut conn = db();
        let n = note(&mut conn, "v0", 0);
        for i in 1..(MAX_VERSIONS as i64 + 10) {
            save_text(&mut conn, n.id, &format!("v{i}"), i).unwrap();
        }
        assert_eq!(must(&conn, n.id).unwrap().versions, MAX_VERSIONS as i64);
    }

    #[test]
    fn tabs_stay_at_five() {
        let mut conn = db();
        let ids: Vec<i64> = (0..6)
            .map(|i| note(&mut conn, &format!("n{i}"), i).id)
            .collect();
        let open: Vec<i64> = tabs(&conn).unwrap().iter().map(|n| n.id).collect();
        assert_eq!(open, ids[1..].to_vec());
        move_tab(&mut conn, ids[5], 0).unwrap();
        close_tab(&mut conn, ids[2]).unwrap();
        let open: Vec<i64> = tabs(&conn).unwrap().iter().map(|n| n.id).collect();
        assert_eq!(open, vec![ids[5], ids[1], ids[3], ids[4]]);
    }

    #[test]
    fn delete_and_undo() {
        let mut conn = db();
        let a = note(&mut conn, "a", 1);
        let a = save_text(&mut conn, a.id, "a2", 2).unwrap();
        let b = note(&mut conn, "b", 3);
        let gone = delete(&mut conn, &[a.id]).unwrap();
        assert_eq!(list(&conn).unwrap().len(), 1);
        assert_eq!(tabs(&conn).unwrap()[0].id, b.id);
        restore(&mut conn, &gone).unwrap();
        let back = must(&conn, a.id).unwrap();
        assert_eq!(back.text, "a2");
        assert_eq!(back.versions, 2);
        let open: Vec<i64> = tabs(&conn).unwrap().iter().map(|n| n.id).collect();
        assert_eq!(open, vec![a.id, b.id]);
    }

    #[test]
    fn append_adds_a_line() {
        let mut conn = db();
        let n = note(&mut conn, "first", 1);
        let n = append(&mut conn, n.id, "second", 2).unwrap();
        assert_eq!(n.text, "first\nsecond");
    }
}
