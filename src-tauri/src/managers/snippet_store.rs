//! The snippets table: a trigger phrase, the text it pastes, whether it only
//! expands when said alone, and how often it was used. Plain rusqlite, like
//! `dictionary_store`; `SnippetManager` wraps it with events and the snapshot
//! the pipeline reads.

use crate::snippet_rules::{self as rules, MAX_TEXT_CHARS, MAX_TRIGGER_CHARS};
use anyhow::{anyhow, Result};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Snippet {
    pub id: i64,
    pub trigger: String,
    pub text: String,
    /// Only when the trigger is the whole dictation.
    pub alone: bool,
    pub uses: i64,
    pub added_at: i64,
    pub last_used_at: Option<i64>,
    /// The app the snippet was last pasted into.
    pub last_app: Option<String>,
    pub last_app_path: Option<String>,
}

/// A snippet as the editor saves it, and as files import it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SnippetInput {
    pub trigger: String,
    pub text: String,
    #[serde(default)]
    pub alone: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SnippetImport {
    pub added: u32,
    pub updated: u32,
    pub skipped: u32,
}

const COLUMNS: &str =
    "id, trigger, text, alone, uses, added_at, last_used_at, last_app, last_app_path";

fn map(row: &Row<'_>) -> rusqlite::Result<Snippet> {
    Ok(Snippet {
        id: row.get("id")?,
        trigger: row.get("trigger")?,
        text: row.get("text")?,
        alone: row.get("alone")?,
        uses: row.get("uses")?,
        added_at: row.get("added_at")?,
        last_used_at: row.get("last_used_at")?,
        last_app: row.get("last_app")?,
        last_app_path: row.get("last_app_path")?,
    })
}

/// Checks a snippet before it is saved; the error is shown as written.
pub fn validate(input: &SnippetInput) -> Result<SnippetInput> {
    let trigger = rules::clean_trigger(&input.trigger);
    if rules::normalize(&trigger).is_empty() {
        return Err(anyhow!("Say what triggers the snippet."));
    }
    if trigger.chars().count() > MAX_TRIGGER_CHARS {
        return Err(anyhow!(
            "Keep the trigger under {MAX_TRIGGER_CHARS} characters."
        ));
    }
    if input.text.trim().is_empty() {
        return Err(anyhow!("Write the text the snippet pastes."));
    }
    if input.text.chars().count() > MAX_TEXT_CHARS {
        return Err(anyhow!("Keep the text under {MAX_TEXT_CHARS} characters."));
    }
    Ok(SnippetInput {
        trigger,
        text: input.text.trim_end().to_string(),
        alone: input.alone,
    })
}

pub fn get(conn: &Connection, id: i64) -> Result<Option<Snippet>> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM snippet WHERE id = ?1"),
            [id],
            map,
        )
        .optional()?)
}

/// Most used first, then newest.
pub fn list(conn: &Connection) -> Result<Vec<Snippet>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM snippet ORDER BY uses DESC, added_at DESC, id DESC"
    ))?;
    let rows = stmt.query_map([], map)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// A snippet whose trigger reads the same, other than `except`.
pub fn find_trigger(
    conn: &Connection,
    trigger: &str,
    except: Option<i64>,
) -> Result<Option<Snippet>> {
    let wanted = rules::normalize(trigger);
    Ok(list(conn)?
        .into_iter()
        .find(|s| Some(s.id) != except && rules::normalize(&s.trigger) == wanted))
}

pub fn add(conn: &mut Connection, input: &SnippetInput, now: i64) -> Result<Snippet> {
    let input = validate(input)?;
    if find_trigger(conn, &input.trigger, None)?.is_some() {
        return Err(anyhow!("Another snippet has that trigger."));
    }
    conn.execute(
        "INSERT INTO snippet (trigger, text, alone, added_at) VALUES (?1, ?2, ?3, ?4)",
        params![input.trigger, input.text, input.alone, now],
    )?;
    let id = conn.last_insert_rowid();
    get(conn, id)?.ok_or_else(|| anyhow!("The snippet wasn't saved"))
}

pub fn update(conn: &mut Connection, id: i64, input: &SnippetInput) -> Result<Snippet> {
    let input = validate(input)?;
    if find_trigger(conn, &input.trigger, Some(id))?.is_some() {
        return Err(anyhow!("Another snippet has that trigger."));
    }
    let changed = conn.execute(
        "UPDATE snippet SET trigger = ?1, text = ?2, alone = ?3 WHERE id = ?4",
        params![input.trigger, input.text, input.alone, id],
    )?;
    if changed == 0 {
        return Err(anyhow!("That snippet no longer exists."));
    }
    get(conn, id)?.ok_or_else(|| anyhow!("That snippet no longer exists."))
}

/// Deletes snippets and returns them, so Undo can put them back.
pub fn delete(conn: &mut Connection, ids: &[i64]) -> Result<Vec<Snippet>> {
    let tx = conn.transaction()?;
    let mut gone = Vec::new();
    for id in ids {
        if let Some(s) = get(&tx, *id)? {
            tx.execute("DELETE FROM snippet WHERE id = ?1", [id])?;
            gone.push(s);
        }
    }
    tx.commit()?;
    Ok(gone)
}

/// Puts deleted snippets back as they were, ids included.
pub fn restore(conn: &mut Connection, snippets: &[Snippet]) -> Result<()> {
    let tx = conn.transaction()?;
    for s in snippets {
        tx.execute(
            &format!("INSERT OR REPLACE INTO snippet ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"),
            params![
                s.id,
                s.trigger,
                s.text,
                s.alone,
                s.uses,
                s.added_at,
                s.last_used_at,
                s.last_app,
                s.last_app_path
            ],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// Counts a paste of each snippet, once per dictation.
pub fn record_use(
    conn: &mut Connection,
    ids: &[i64],
    app: Option<(&str, Option<&str>)>,
    now: i64,
) -> Result<()> {
    let mut ids = ids.to_vec();
    ids.sort_unstable();
    ids.dedup();
    let tx = conn.transaction()?;
    for id in ids {
        tx.execute(
            "UPDATE snippet SET uses = uses + 1, last_used_at = ?1,
                last_app = COALESCE(?2, last_app), last_app_path = COALESCE(?3, last_app_path)
             WHERE id = ?4",
            params![now, app.map(|a| a.0), app.and_then(|a| a.1), id],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// Adds new triggers and updates the text of ones that exist.
pub fn import(conn: &mut Connection, inputs: &[SnippetInput], now: i64) -> Result<SnippetImport> {
    let mut summary = SnippetImport::default();
    for input in inputs {
        let Ok(input) = validate(input) else {
            summary.skipped += 1;
            continue;
        };
        match find_trigger(conn, &input.trigger, None)? {
            Some(existing) if existing.text == input.text && existing.alone == input.alone => {
                summary.skipped += 1;
            }
            Some(existing) => {
                update(conn, existing.id, &input)?;
                summary.updated += 1;
            }
            None => {
                add(conn, &input, now)?;
                summary.added += 1;
            }
        }
    }
    Ok(summary)
}

#[derive(Deserialize)]
struct FileSnippet {
    #[serde(alias = "phrase", alias = "shortcut", alias = "name")]
    trigger: String,
    #[serde(
        alias = "body",
        alias = "expansion",
        alias = "content",
        alias = "replacement"
    )]
    text: String,
    #[serde(default)]
    alone: Option<bool>,
    #[serde(default, rename = "match")]
    matching: Option<String>,
}

/// Reads a JSON list of snippets (Legato's export, or objects with trigger
/// and text under common names) or a CSV of trigger,text[,alone].
pub fn parse_import(content: &str) -> Result<Vec<SnippetInput>> {
    let trimmed = content.trim_start_matches('\u{feff}').trim();
    if trimmed.starts_with('[') || trimmed.starts_with('{') {
        let value: serde_json::Value = serde_json::from_str(trimmed)?;
        let list = match value {
            serde_json::Value::Array(items) => items,
            serde_json::Value::Object(mut map) => match map.remove("snippets") {
                Some(serde_json::Value::Array(items)) => items,
                _ => return Err(anyhow!("No snippets in that file")),
            },
            _ => return Err(anyhow!("No snippets in that file")),
        };
        return Ok(list
            .into_iter()
            .filter_map(|item| serde_json::from_value::<FileSnippet>(item).ok())
            .map(|f| SnippetInput {
                trigger: f.trigger,
                text: f.text,
                alone: f.alone.unwrap_or(f.matching.as_deref() == Some("alone")),
            })
            .collect());
    }
    let mut out = Vec::new();
    for (n, line) in csv_rows(trimmed).into_iter().enumerate() {
        if line.len() < 2 {
            continue;
        }
        if n == 0 && line[0].eq_ignore_ascii_case("trigger") {
            continue;
        }
        out.push(SnippetInput {
            trigger: line[0].clone(),
            text: line[1].clone(),
            alone: line.get(2).is_some_and(|v| {
                matches!(
                    v.trim().to_lowercase().as_str(),
                    "true" | "yes" | "1" | "alone"
                )
            }),
        });
    }
    Ok(out)
}

/// CSV rows with quoted fields, which may hold commas and line breaks.
fn csv_rows(content: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = content.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            ('"', true) => quoted = false,
            ('"', false) if field.is_empty() => quoted = true,
            (',', false) => row.push(std::mem::take(&mut field)),
            ('\r', false) => {}
            ('\n', false) => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            _ => field.push(c),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

pub fn export_json(snippets: &[Snippet]) -> Result<String> {
    let list: Vec<SnippetInput> = snippets
        .iter()
        .map(|s| SnippetInput {
            trigger: s.trigger.clone(),
            text: s.text.clone(),
            alone: s.alone,
        })
        .collect();
    Ok(serde_json::to_string_pretty(&list)?)
}

/// What the matcher needs.
pub fn rules_of(snippets: &[Snippet]) -> Vec<rules::SnippetRule> {
    snippets
        .iter()
        .map(|s| rules::SnippetRule {
            id: s.id,
            trigger: s.trigger.clone(),
            alone: s.alone,
        })
        .collect()
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

    fn input(trigger: &str, text: &str, alone: bool) -> SnippetInput {
        SnippetInput {
            trigger: trigger.into(),
            text: text.into(),
            alone,
        }
    }

    #[test]
    fn add_edit_delete_restore() {
        let mut conn = db();
        let s = add(
            &mut conn,
            &input("  sign   off ", "Best regards,\nAbhash", true),
            10,
        )
        .unwrap();
        assert_eq!(s.trigger, "sign off");
        assert!(add(&mut conn, &input("Sign off.", "x", false), 11).is_err());
        let s = update(&mut conn, s.id, &input("sign off", "Best,\nAbhash", true)).unwrap();
        assert_eq!(s.text, "Best,\nAbhash");
        record_use(&mut conn, &[s.id, s.id], Some(("Outlook", None)), 20).unwrap();
        let s = get(&conn, s.id).unwrap().unwrap();
        assert_eq!((s.uses, s.last_app.as_deref()), (1, Some("Outlook")));
        let gone = delete(&mut conn, &[s.id]).unwrap();
        assert!(list(&conn).unwrap().is_empty());
        restore(&mut conn, &gone).unwrap();
        assert_eq!(list(&conn).unwrap(), gone);
    }

    #[test]
    fn imports_json_and_csv() {
        let json = r#"[{"trigger":"my LinkedIn","body":"https://linkedin.com/in/x"},
                       {"phrase":"sign off","text":"Best","match":"alone"}]"#;
        let parsed = parse_import(json).unwrap();
        assert_eq!(parsed[1], input("sign off", "Best", true));
        let csv = "trigger,text,alone\n\"review prompt\",\"Review this,\nplease\",yes\n";
        let parsed_csv = parse_import(csv).unwrap();
        assert_eq!(
            parsed_csv,
            vec![input("review prompt", "Review this,\nplease", true)]
        );

        let mut conn = db();
        let summary = import(&mut conn, &parsed, 5).unwrap();
        assert_eq!((summary.added, summary.updated, summary.skipped), (2, 0, 0));
        let again = import(&mut conn, &[input("sign off", "Best regards", true)], 6).unwrap();
        assert_eq!((again.added, again.updated), (0, 1));
        let exported = export_json(&list(&conn).unwrap()).unwrap();
        assert_eq!(parse_import(&exported).unwrap().len(), 2);
    }
}
