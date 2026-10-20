//! `legato --mcp`: Legato as a Model Context Protocol server for AI apps on
//! this PC (Claude, Cursor, ...). It runs as its own short-lived process,
//! started by the AI app over stdio, and reads Legato's database; it never
//! starts the app. Nothing answers until "Let AI apps read Legato" is on in
//! Settings › Connections. The protocol is in `mcp_rules`.

use crate::managers::{dictionary_store, history_store, notes_store, scratch_store};
use crate::mcp_rules::{self, arg_str, arg_u32, ToolSpec, Tools};
use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::time::Duration;

/// Tauri's identifier, which names the data folder under %APPDATA%.
const IDENTIFIER: &str = "io.github.abhash-chakraborty.legato";

/// Whether this process was started as the MCP server.
pub fn is_invocation() -> bool {
    std::env::args().skip(1).any(|a| a == "--mcp")
}

fn data_dir() -> Option<PathBuf> {
    crate::portable::init();
    if let Some(dir) = crate::portable::data_dir() {
        return Some(dir.clone());
    }
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join(IDENTIFIER))
}

/// Reads the switch from the settings file each time, so turning it off in
/// Legato takes effect without restarting the AI app.
fn enabled(dir: &std::path::Path) -> bool {
    std::fs::read_to_string(dir.join(crate::settings::SETTINGS_STORE_PATH))
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v.get("settings")?.get("mcp_enabled")?.as_bool())
        .unwrap_or(false)
}

/// "2026-10-20 14:52", local time, from seconds (or milliseconds).
fn when(ts: i64) -> String {
    use chrono::TimeZone;
    let secs = if ts > 100_000_000_000 { ts / 1000 } else { ts };
    chrono::Local
        .timestamp_opt(secs, 0)
        .single()
        .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_default()
}

fn pretty(v: Value) -> String {
    serde_json::to_string_pretty(&v).unwrap_or_default()
}

struct Legato {
    dir: PathBuf,
}

impl Legato {
    fn open(&self, write: bool) -> Result<Connection, String> {
        let path = self.dir.join("history.db");
        if !path.exists() {
            return Err("Legato has no data yet: open it once and dictate something.".into());
        }
        let flags = if write {
            OpenFlags::SQLITE_OPEN_READ_WRITE
        } else {
            OpenFlags::SQLITE_OPEN_READ_ONLY
        };
        let conn = Connection::open_with_flags(&path, flags | OpenFlags::SQLITE_OPEN_NO_MUTEX)
            .map_err(|e| format!("Couldn't open Legato's data: {e}"))?;
        // The app may be writing; wait for it rather than failing.
        let _ = conn.busy_timeout(Duration::from_secs(3));
        Ok(conn)
    }
}

fn schema(props: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": props, "required": required })
}

impl Tools for Legato {
    fn specs(&self) -> Vec<ToolSpec> {
        vec![
            ToolSpec {
                name: "search_dictations",
                description: "Search what the person dictated with Legato, newest first. Without a query, the latest dictations. Each has the text as pasted, the app it went into and when.",
                input_schema: schema(
                    json!({
                        "query": { "type": "string", "description": "Words to find; every word must appear (prefix match)." },
                        "app": { "type": "string", "description": "Only dictations into this app, by name, e.g. Slack." },
                        "limit": { "type": "integer", "minimum": 1, "maximum": 100, "default": 20 }
                    }),
                    &[],
                ),
            },
            ToolSpec {
                name: "list_notes",
                description: "List the person's meeting and voice notes recorded with Legato, newest first, with title, date, length, tags and whether a summary exists. With a query, only notes whose title or transcript matches.",
                input_schema: schema(
                    json!({
                        "query": { "type": "string" },
                        "limit": { "type": "integer", "minimum": 1, "maximum": 100, "default": 20 }
                    }),
                    &[],
                ),
            },
            ToolSpec {
                name: "get_note",
                description: "One note in full: its summary, decisions, action items with owners, the person's own notes, and the transcript with who spoke when.",
                input_schema: schema(
                    json!({ "id": { "type": "integer", "description": "The note's id from list_notes." } }),
                    &["id"],
                ),
            },
            ToolSpec {
                name: "list_dictionary",
                description: "The person's Dictionary: names and terms Legato spells their way, with the ways the speech model misheard them. Useful for spelling names right.",
                input_schema: schema(json!({}), &[]),
            },
            ToolSpec {
                name: "upcoming_meetings",
                description: "The person's meetings from the calendars they connected in Legato, starting with any going on now, with times, calendar and join link.",
                input_schema: schema(
                    json!({ "days": { "type": "integer", "minimum": 1, "maximum": 14, "default": 1, "description": "How many days ahead." } }),
                    &[],
                ),
            },
            ToolSpec {
                name: "list_scratchpad",
                description: "The person's Scratchpad notes (quick notes they type or dictate), pinned first, with their text.",
                input_schema: schema(
                    json!({ "limit": { "type": "integer", "minimum": 1, "maximum": 50, "default": 20 } }),
                    &[],
                ),
            },
            ToolSpec {
                name: "add_scratchpad_note",
                description: "Save text as a new note in the person's Legato Scratchpad, where they can edit it and send it to any app. Markdown is fine.",
                input_schema: schema(
                    json!({ "text": { "type": "string", "description": "The note's text; its first line becomes the title." } }),
                    &["text"],
                ),
            },
        ]
    }

    fn call(&self, name: &str, args: &Value) -> Result<String, String> {
        if !enabled(&self.dir) {
            return Err("Legato isn't sharing with AI apps. Turn on \"Let AI apps read Legato\" in Legato › Settings › Connections.".into());
        }
        match name {
            "search_dictations" => {
                let conn = self.open(false)?;
                let q = history_store::HistoryQuery {
                    limit: Some(arg_u32(args, "limit", 20, 100)),
                    search: arg_str(args, "query").map(str::to_string),
                    app: arg_str(args, "app").map(str::to_string),
                    ..Default::default()
                };
                let page = history_store::query(&conn, &q).map_err(|e| e.to_string())?;
                let rows: Vec<Value> = page
                    .entries
                    .iter()
                    .map(|e| {
                        json!({
                            "id": e.id,
                            "when": when(e.timestamp),
                            "app": e.paste_app_name.as_ref().or(e.app_name.as_ref()),
                            "text": e.post_processed_text.as_ref().unwrap_or(&e.transcription_text),
                        })
                    })
                    .collect();
                Ok(pretty(json!({ "dictations": rows, "more": page.has_more })))
            }
            "list_notes" => {
                let conn = self.open(false)?;
                let limit = arg_u32(args, "limit", 20, 100) as usize;
                let mut notes = notes_store::list(&conn).map_err(|e| e.to_string())?;
                if let Some(query) = arg_str(args, "query") {
                    let hits: Vec<i64> = notes_store::search(&conn, query)
                        .map_err(|e| e.to_string())?
                        .into_iter()
                        .map(|m| m.id)
                        .collect();
                    notes.retain(|n| hits.contains(&n.id));
                }
                notes.sort_by_key(|n| std::cmp::Reverse(n.started_at));
                let rows: Vec<Value> = notes
                    .iter()
                    .take(limit)
                    .map(|n| {
                        json!({
                            "id": n.id,
                            "title": n.title,
                            "kind": n.kind.as_str(),
                            "when": when(n.started_at),
                            "minutes": (n.duration_ms as f64 / 60000.0).round(),
                            "app": n.app,
                            "tags": n.tags,
                            "has_summary": !n.summary.trim().is_empty(),
                        })
                    })
                    .collect();
                Ok(pretty(json!({ "notes": rows })))
            }
            "get_note" => {
                let id = args
                    .get("id")
                    .and_then(Value::as_i64)
                    .ok_or("Give the note's id from list_notes.")?;
                let conn = self.open(false)?;
                let note = notes_store::get(&conn, id)
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| format!("No note with id {id}."))?;
                let lines = notes_store::lines(&conn, id).map_err(|e| e.to_string())?;
                let transcript: Vec<String> = lines
                    .iter()
                    .map(|l| {
                        let s = l.at_ms / 1000;
                        format!(
                            "[{}:{:02}] {}: {}",
                            s / 60,
                            s % 60,
                            if l.who.as_str() == "you" {
                                "You"
                            } else {
                                "Them"
                            },
                            l.text
                        )
                    })
                    .collect();
                Ok(pretty(json!({
                    "id": note.id,
                    "title": note.title,
                    "kind": note.kind.as_str(),
                    "when": when(note.started_at),
                    "minutes": (note.duration_ms as f64 / 60000.0).round(),
                    "app": note.app,
                    "tags": note.tags,
                    "summary": note.summary,
                    "decisions": note.decisions,
                    "action_items": note.actions.iter().map(|a| json!({ "owner": a.owner, "text": a.text, "done": a.done })).collect::<Vec<_>>(),
                    "my_notes": note.my_notes,
                    "transcript": transcript.join("\n"),
                })))
            }
            "list_dictionary" => {
                let conn = self.open(false)?;
                let words: Vec<Value> = dictionary_store::list(&conn)
                    .map_err(|e| e.to_string())?
                    .iter()
                    .map(|w| json!({ "word": w.text, "heard_as": w.heard }))
                    .collect();
                Ok(pretty(json!({ "words": words })))
            }
            "upcoming_meetings" => {
                let days = arg_u32(args, "days", 1, 14) as i64;
                let cache: crate::calendar::Cache = std::fs::read_to_string(
                    self.dir.join(crate::calendar::CACHE_FILE),
                )
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .ok_or("No calendars are connected. Add one in Legato › Settings › Connections.")?;
                let now = chrono::Utc::now().timestamp();
                let rows: Vec<Value> = cache
                    .meetings()
                    .iter()
                    .filter(|m| m.end > now && m.start < now + days * 86_400)
                    .map(|m| {
                        json!({
                            "title": m.title,
                            "start": when(m.start),
                            "end": when(m.end),
                            "all_day": m.all_day,
                            "now": m.start <= now && now < m.end,
                            "calendar": m.calendar,
                            "link": m.link,
                            "location": m.location,
                        })
                    })
                    .collect();
                Ok(pretty(json!({ "meetings": rows })))
            }
            "list_scratchpad" => {
                let conn = self.open(false)?;
                let limit = arg_u32(args, "limit", 20, 50) as usize;
                let mut notes = scratch_store::list(&conn).map_err(|e| e.to_string())?;
                notes.sort_by(|a, b| {
                    b.pinned
                        .cmp(&a.pinned)
                        .then(b.updated_at.cmp(&a.updated_at))
                });
                let rows: Vec<Value> = notes
                    .iter()
                    .take(limit)
                    .map(|n| json!({ "id": n.id, "title": n.title, "pinned": n.pinned, "edited": when(n.updated_at), "text": n.text }))
                    .collect();
                Ok(pretty(json!({ "notes": rows })))
            }
            "add_scratchpad_note" => {
                let text = arg_str(args, "text").ok_or("Give the note's text.")?;
                let mut conn = self.open(true)?;
                let now = chrono::Utc::now().timestamp();
                let note = scratch_store::create(
                    &mut conn,
                    &scratch_store::NewNote {
                        text: text.to_string(),
                        source: None,
                        source_app: None,
                        source_app_path: None,
                        reason: None,
                    },
                    now,
                )
                .map_err(|e| e.to_string())?;
                Ok(format!(
                    "Saved to the Scratchpad as \"{}\" (id {}).",
                    note.title, note.id
                ))
            }
            other => Err(format!("Unknown tool: {other}")),
        }
    }
}

/// Serves MCP on stdin and stdout until the AI app closes stdin.
pub fn run() -> i32 {
    let Some(dir) = data_dir() else {
        eprintln!("Couldn't find Legato's data folder");
        return 1;
    };
    let legato = Legato { dir };
    let version = env!("CARGO_PKG_VERSION");
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else {
            break;
        };
        if let Some(answer) = mcp_rules::handle(&line, &legato, version) {
            if writeln!(stdout, "{answer}").is_err() || stdout.flush().is_err() {
                break;
            }
        }
    }
    0
}
