//! Transform runs: which transform ran, where, on how many words, how long
//! it took and whether the result was kept. The texts themselves are not
//! stored. Plain rusqlite, so it can be tested on its own.

use anyhow::Result;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use specta::Type;

/// What a transform ran on.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum RunSource {
    Selection,
    /// Nothing was selected, so it ran on the last dictation.
    LastDictation,
    /// Dictate then transform, or the transform after every dictation.
    Dictation,
    Ask,
}

impl RunSource {
    fn as_str(self) -> &'static str {
        match self {
            RunSource::Selection => "selection",
            RunSource::LastDictation => "last_dictation",
            RunSource::Dictation => "dictation",
            RunSource::Ask => "ask",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "selection" => RunSource::Selection,
            "last_dictation" => RunSource::LastDictation,
            "ask" => RunSource::Ask,
            _ => RunSource::Dictation,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum RunResult {
    Kept,
    Undone,
    /// The result went to the clipboard instead of being pasted.
    Copied,
    Failed,
}

impl RunResult {
    fn as_str(self) -> &'static str {
        match self {
            RunResult::Kept => "kept",
            RunResult::Undone => "undone",
            RunResult::Copied => "copied",
            RunResult::Failed => "failed",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "undone" => RunResult::Undone,
            "copied" => RunResult::Copied,
            "failed" => RunResult::Failed,
            _ => RunResult::Kept,
        }
    }
}

#[derive(Serialize, Clone, Debug, Type)]
pub struct TransformRun {
    pub id: i64,
    pub at: i64,
    pub transform_id: String,
    /// The transform's name when it ran.
    pub name: String,
    pub source: RunSource,
    pub app_name: Option<String>,
    pub app_path: Option<String>,
    pub in_words: i64,
    pub out_words: i64,
    pub ms: i64,
    pub result: RunResult,
    /// Why it failed, in the status line's words.
    pub error: Option<String>,
    pub history_id: Option<i64>,
}

#[derive(Default)]
pub struct NewRun {
    pub at: i64,
    pub transform_id: String,
    pub name: String,
    pub source: Option<RunSource>,
    pub app_name: Option<String>,
    pub app_path: Option<String>,
    pub in_words: i64,
    pub out_words: i64,
    pub ms: i64,
    pub result: Option<RunResult>,
    pub error: Option<String>,
    pub history_id: Option<i64>,
}

/// How much a transform is used, for its card.
#[derive(Serialize, Clone, Debug, PartialEq, Type)]
pub struct TransformUsage {
    pub transform_id: String,
    /// Runs that didn't fail.
    pub uses: i64,
    pub avg_ms: i64,
    pub last_at: i64,
}

/// Runs older than the newest this many are dropped.
pub const KEEP_RUNS: i64 = 1000;

const COLUMNS: &str = "id, at, transform_id, name, source, app_name, app_path, in_words, \
                       out_words, ms, result, error, history_id";

fn map_run(row: &Row) -> rusqlite::Result<TransformRun> {
    Ok(TransformRun {
        id: row.get(0)?,
        at: row.get(1)?,
        transform_id: row.get(2)?,
        name: row.get(3)?,
        source: RunSource::parse(&row.get::<_, String>(4)?),
        app_name: row.get(5)?,
        app_path: row.get(6)?,
        in_words: row.get(7)?,
        out_words: row.get(8)?,
        ms: row.get(9)?,
        result: RunResult::parse(&row.get::<_, String>(10)?),
        error: row.get(11)?,
        history_id: row.get(12)?,
    })
}

pub fn insert(conn: &Connection, run: &NewRun) -> Result<i64> {
    conn.execute(
        "INSERT INTO transform_run (at, transform_id, name, source, app_name, app_path,
            in_words, out_words, ms, result, error, history_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            run.at,
            run.transform_id,
            run.name,
            run.source.unwrap_or(RunSource::Dictation).as_str(),
            run.app_name,
            run.app_path,
            run.in_words,
            run.out_words,
            run.ms,
            run.result.unwrap_or(RunResult::Kept).as_str(),
            run.error,
            run.history_id,
        ],
    )?;
    let id = conn.last_insert_rowid();
    conn.execute(
        "DELETE FROM transform_run WHERE id <= ?1 - ?2",
        params![id, KEEP_RUNS],
    )?;
    Ok(id)
}

pub fn set_result(conn: &Connection, id: i64, result: RunResult) -> Result<()> {
    conn.execute(
        "UPDATE transform_run SET result = ?1 WHERE id = ?2",
        params![result.as_str(), id],
    )?;
    Ok(())
}

/// The newest runs first.
pub fn recent(conn: &Connection, limit: i64) -> Result<Vec<TransformRun>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM transform_run ORDER BY at DESC, id DESC LIMIT ?1"
    ))?;
    let runs = stmt
        .query_map(params![limit], map_run)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(runs)
}

pub fn usage(conn: &Connection) -> Result<Vec<TransformUsage>> {
    let mut stmt = conn.prepare(
        "SELECT transform_id, COUNT(*), CAST(IFNULL(AVG(ms), 0) AS INTEGER), MAX(at)
         FROM transform_run WHERE result != 'failed'
         GROUP BY transform_id",
    )?;
    let usage = stmt
        .query_map([], |row| {
            Ok(TransformUsage {
                transform_id: row.get(0)?,
                uses: row.get(1)?,
                avg_ms: row.get(2)?,
                last_at: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(usage)
}

/// Clears every run.
pub fn clear(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM transform_run", [])?;
    Ok(())
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

    fn run(at: i64, id: &str, ms: i64, result: RunResult) -> NewRun {
        NewRun {
            at,
            transform_id: id.into(),
            name: id.into(),
            source: Some(RunSource::Selection),
            in_words: 10,
            out_words: 8,
            ms,
            result: Some(result),
            ..Default::default()
        }
    }

    #[test]
    fn runs_are_listed_newest_first_and_counted_per_transform() {
        let conn = conn();
        insert(&conn, &run(100, "polish", 1000, RunResult::Kept)).unwrap();
        insert(&conn, &run(200, "polish", 2000, RunResult::Undone)).unwrap();
        let failed = insert(&conn, &run(300, "polish", 0, RunResult::Failed)).unwrap();
        insert(&conn, &run(150, "german", 900, RunResult::Copied)).unwrap();

        let runs = recent(&conn, 10).unwrap();
        assert_eq!(
            runs.iter().map(|r| r.at).collect::<Vec<_>>(),
            [300, 200, 150, 100]
        );
        assert_eq!(runs[0].id, failed);
        assert_eq!(runs[0].source, RunSource::Selection);

        let mut usage = usage(&conn).unwrap();
        usage.sort_by(|a, b| a.transform_id.cmp(&b.transform_id));
        assert_eq!(
            usage,
            vec![
                TransformUsage {
                    transform_id: "german".into(),
                    uses: 1,
                    avg_ms: 900,
                    last_at: 150
                },
                TransformUsage {
                    transform_id: "polish".into(),
                    uses: 2,
                    avg_ms: 1500,
                    last_at: 200
                },
            ]
        );
    }

    #[test]
    fn undo_marks_the_run() {
        let conn = conn();
        let id = insert(&conn, &run(100, "polish", 1000, RunResult::Kept)).unwrap();
        set_result(&conn, id, RunResult::Undone).unwrap();
        assert_eq!(recent(&conn, 1).unwrap()[0].result, RunResult::Undone);
        clear(&conn).unwrap();
        assert!(recent(&conn, 1).unwrap().is_empty());
    }
}
