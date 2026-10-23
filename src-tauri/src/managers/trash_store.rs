//! Recently deleted: notes and Scratchpad notes kept whole for a while after
//! they're deleted, so they can be restored with their ids. Each kind's
//! manager says what it keeps (`payload`, JSON) and how to put it back.
//! Plain rusqlite, so it can be tested on its own.

use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use specta::Type;

/// How long a deleted item can be restored.
pub const KEEP_DAYS: i64 = 30;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum TrashKind {
    /// A Notes recording, with its transcript and audio.
    Note,
    /// A Scratchpad note, with its versions.
    Scratch,
}

impl TrashKind {
    pub fn as_str(self) -> &'static str {
        match self {
            TrashKind::Note => "note",
            TrashKind::Scratch => "scratch",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "note" => Some(TrashKind::Note),
            "scratch" => Some(TrashKind::Scratch),
            _ => None,
        }
    }
}

/// A row of Recently deleted, without what it holds.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Type)]
pub struct TrashItem {
    pub id: i64,
    pub kind: TrashKind,
    /// The note's id, which it gets back on restore.
    pub item_id: i64,
    pub title: String,
    /// The start of its text.
    pub preview: String,
    /// A recording's length.
    pub duration_ms: Option<i64>,
    /// Unix seconds.
    pub deleted_at: i64,
}

/// What `put` stores.
pub struct NewTrash<'a> {
    pub kind: TrashKind,
    pub item_id: i64,
    pub title: &'a str,
    pub preview: &'a str,
    pub duration_ms: Option<i64>,
    pub payload: String,
}

/// A row taken out, with what it holds.
#[derive(Debug, PartialEq, Eq)]
pub struct Taken {
    pub kind: TrashKind,
    pub item_id: i64,
    pub payload: String,
}

const PREVIEW_CHARS: usize = 140;
const COLUMNS: &str = "id, kind, item_id, title, preview, duration_ms, deleted_at";

fn map(row: &Row) -> rusqlite::Result<Option<TrashItem>> {
    let kind: String = row.get(1)?;
    Ok(TrashKind::parse(&kind).map(|kind| TrashItem {
        id: row.get(0).unwrap_or_default(),
        kind,
        item_id: row.get(2).unwrap_or_default(),
        title: row.get(3).unwrap_or_default(),
        preview: row.get(4).unwrap_or_default(),
        duration_ms: row.get(5).unwrap_or_default(),
        deleted_at: row.get(6).unwrap_or_default(),
    }))
}

/// The preview shown for a deleted item: its first words on one line.
pub fn preview(text: &str) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    match line.char_indices().nth(PREVIEW_CHARS) {
        Some((cut, _)) => format!("{}…", line[..cut].trim_end()),
        None => line,
    }
}

/// Keeps a deleted item. An earlier copy of the same item goes.
pub fn put(conn: &Connection, item: &NewTrash, at: i64) -> Result<i64> {
    conn.execute(
        "DELETE FROM trash WHERE kind = ?1 AND item_id = ?2",
        params![item.kind.as_str(), item.item_id],
    )?;
    conn.execute(
        "INSERT INTO trash (kind, item_id, title, preview, duration_ms, deleted_at, payload)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            item.kind.as_str(),
            item.item_id,
            item.title,
            item.preview,
            item.duration_ms,
            at,
            item.payload
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Recently deleted, newest first.
pub fn list(conn: &Connection) -> Result<Vec<TrashItem>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM trash ORDER BY deleted_at DESC, id DESC"
    ))?;
    let rows = stmt.query_map([], map)?;
    Ok(rows.filter_map(|r| r.ok().flatten()).collect())
}

/// A row with what it holds, left in place: restoring removes it once the
/// item is back.
pub fn peek(conn: &Connection, id: i64) -> Result<Option<Taken>> {
    let row = conn
        .query_row(
            "SELECT kind, item_id, payload FROM trash WHERE id = ?1",
            params![id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?;
    Ok(row.and_then(|(kind, item_id, payload)| {
        TrashKind::parse(&kind).map(|kind| Taken {
            kind,
            item_id,
            payload,
        })
    }))
}

/// Takes a row out, to delete it for good.
pub fn take(conn: &Connection, id: i64) -> Result<Option<Taken>> {
    let taken = peek(conn, id)?;
    conn.execute("DELETE FROM trash WHERE id = ?1", params![id])?;
    Ok(taken)
}

/// Forgets an item that came back some other way (the toast's Undo).
pub fn forget(conn: &Connection, kind: TrashKind, item_id: i64) -> Result<()> {
    conn.execute(
        "DELETE FROM trash WHERE kind = ?1 AND item_id = ?2",
        params![kind.as_str(), item_id],
    )?;
    Ok(())
}

/// The ids of the items of a kind that are kept, so their files stay.
pub fn item_ids(conn: &Connection, kind: TrashKind) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare("SELECT item_id FROM trash WHERE kind = ?1")?;
    let rows = stmt.query_map(params![kind.as_str()], |r| r.get(0))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

/// Takes out everything deleted before `before` (Unix seconds), or all of
/// it with `None`, for the caller to remove for good.
pub fn take_older(conn: &Connection, before: Option<i64>) -> Result<Vec<Taken>> {
    let cutoff = before.unwrap_or(i64::MAX);
    let mut stmt =
        conn.prepare("SELECT kind, item_id, payload FROM trash WHERE deleted_at < ?1")?;
    let rows = stmt.query_map(params![cutoff], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;
    let taken = rows
        .filter_map(|r| r.ok())
        .filter_map(|(kind, item_id, payload)| {
            TrashKind::parse(&kind).map(|kind| Taken {
                kind,
                item_id,
                payload,
            })
        })
        .collect();
    conn.execute("DELETE FROM trash WHERE deleted_at < ?1", params![cutoff])?;
    Ok(taken)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite_migration::Migrations;

    fn conn() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        Migrations::new(super::super::history_store::MIGRATIONS.to_vec())
            .to_latest(&mut conn)
            .unwrap();
        conn
    }

    fn item(kind: TrashKind, item_id: i64, title: &str) -> NewTrash<'_> {
        NewTrash {
            kind,
            item_id,
            title,
            preview: "",
            duration_ms: None,
            payload: format!("{{\"id\":{item_id}}}"),
        }
    }

    #[test]
    fn deleted_items_come_back_once() {
        let c = conn();
        put(&c, &item(TrashKind::Note, 4, "Weekly sync"), 100).unwrap();
        let id = put(&c, &item(TrashKind::Scratch, 9, "Ideas"), 200).unwrap();
        let listed = list(&c).unwrap();
        assert_eq!(
            listed.iter().map(|i| i.title.as_str()).collect::<Vec<_>>(),
            ["Ideas", "Weekly sync"]
        );
        let seen = peek(&c, id).unwrap().unwrap();
        assert_eq!((seen.kind, seen.item_id), (TrashKind::Scratch, 9));
        assert_eq!(list(&c).unwrap().len(), 2);
        let taken = take(&c, id).unwrap().unwrap();
        assert_eq!(taken.payload, "{\"id\":9}");
        assert!(take(&c, id).unwrap().is_none());
        assert_eq!(list(&c).unwrap().len(), 1);
    }

    #[test]
    fn deleting_again_keeps_one_copy_and_undo_forgets_it() {
        let c = conn();
        put(&c, &item(TrashKind::Note, 4, "First"), 100).unwrap();
        put(&c, &item(TrashKind::Note, 4, "Second"), 200).unwrap();
        assert_eq!(list(&c).unwrap().len(), 1);
        assert_eq!(item_ids(&c, TrashKind::Note).unwrap(), [4]);
        assert!(item_ids(&c, TrashKind::Scratch).unwrap().is_empty());
        forget(&c, TrashKind::Note, 4).unwrap();
        assert!(list(&c).unwrap().is_empty());
    }

    #[test]
    fn old_items_are_taken_out_for_good() {
        let c = conn();
        put(&c, &item(TrashKind::Note, 1, "Old"), 100).unwrap();
        put(&c, &item(TrashKind::Note, 2, "New"), 900).unwrap();
        let old = take_older(&c, Some(500)).unwrap();
        assert_eq!(old.iter().map(|t| t.item_id).collect::<Vec<_>>(), [1]);
        assert_eq!(list(&c).unwrap().len(), 1);
        assert_eq!(take_older(&c, None).unwrap().len(), 1);
        assert!(list(&c).unwrap().is_empty());
    }

    #[test]
    fn previews_are_one_short_line() {
        assert_eq!(preview("  Morning\n everyone  "), "Morning everyone");
        let long = "word ".repeat(60);
        let p = preview(&long);
        assert!(p.ends_with('…') && p.chars().count() <= PREVIEW_CHARS + 1);
    }
}
