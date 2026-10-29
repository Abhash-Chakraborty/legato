//! The kinds that live in the history database, as phone sync sees them:
//! each item's key and body (docs/phone-sync.md, What syncs), and applying a
//! version from another device. Transforms and Style live in the settings
//! and are handled by the app (sync/mod.rs).

use std::collections::HashMap;

use anyhow::{bail, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

use super::ledger::{self, Item};

/// The kinds kept in the database, and so handled here.
pub const DB_KINDS: [&str; 5] = ["dictionary", "snippets", "history", "notes", "scratchpad"];

/// Whether an item missing from `export` counts as deleted. History loses
/// old dictations to the history limit, so only deletes the person made
/// (`ledger::forget`) spread.
pub fn infer_deletes(kind: &str) -> bool {
    kind != "history"
}

/// What applying a change left to clean up.
#[derive(Debug, Default, PartialEq)]
pub struct Applied {
    /// A recording file that belonged to a deleted row.
    pub removed_audio: Option<String>,
}

fn seconds_to_ms(at: i64) -> Option<i64> {
    (at > 0).then_some(at * 1000)
}

fn text(body: &Value, field: &str) -> String {
    body[field].as_str().unwrap_or_default().to_string()
}

fn opt_text(body: &Value, field: &str) -> Option<String> {
    body[field].as_str().map(str::to_string)
}

fn flag(body: &Value, field: &str) -> bool {
    body[field].as_bool().unwrap_or(false)
}

fn int(body: &Value, field: &str) -> Option<i64> {
    body[field].as_i64()
}

/// Names the device a row came from, when it came from another one, so a
/// third device passed it on by this one doesn't credit this one. Only
/// when set, so this device's own rows hash as they always did.
fn with_origin(body: &mut Value, device: Option<String>) {
    if let Some(device) = device {
        body["device"] = json!(device);
    }
}

/// Keeps the name of the device another device's row came from current.
fn relabel(conn: &Connection, kind: &str, key: &str, device: &str, me: &str) -> Result<()> {
    if !is_own(key, me) {
        conn.execute(
            "UPDATE sync_local SET device = ?1 WHERE kind = ?2 AND key = ?3",
            params![device, kind, key],
        )?;
    }
    Ok(())
}

/// Every item of a database kind as it is now.
pub fn export(conn: &Connection, kind: &str, me: &str) -> Result<Vec<Item>> {
    match kind {
        "dictionary" => export_dictionary(conn),
        "snippets" => export_snippets(conn),
        "history" => export_history(conn, me),
        "notes" => export_notes(conn, me),
        "scratchpad" => export_scratchpad(conn, me),
        _ => bail!("{kind} isn't kept in the database"),
    }
}

/// Puts another device's version of an item here; `body` None deletes it.
/// `device` is that device's name, for rows it adds.
pub fn apply(
    conn: &mut Connection,
    kind: &str,
    key: &str,
    body: Option<&Value>,
    device: &str,
    me: &str,
    now: i64,
) -> Result<Applied> {
    let tx = conn.transaction()?;
    let applied = match kind {
        "dictionary" => apply_dictionary(&tx, key, body, now),
        "snippets" => apply_snippet(&tx, key, body, now),
        "history" => apply_history(&tx, key, body, device, me),
        "notes" => apply_note(&tx, key, body, device, me),
        "scratchpad" => apply_scratch(&tx, key, body, device, me, now),
        _ => bail!("{kind} isn't kept in the database"),
    }?;
    tx.commit()?;
    Ok(applied)
}

/// A few words naming an item, for See merges.
pub fn label(kind: &str, body: Option<&Value>) -> String {
    let Some(body) = body else {
        return String::new();
    };
    let field = match kind {
        "dictionary" => "text",
        "snippets" => "trigger",
        "history" => "text",
        "notes" | "scratchpad" | "transforms" => "title",
        _ => "",
    };
    let mut label = text(body, field);
    if kind == "scratchpad" && label.is_empty() {
        label = text(body, "text");
    }
    let short: String = label.chars().take(60).collect();
    if short.len() < label.len() {
        format!("{short}…")
    } else {
        short
    }
}

/// How many items of a kind there are, for Settings › Devices.
pub fn count(conn: &Connection, kind: &str) -> Result<i64> {
    let sql = match kind {
        "dictionary" => "SELECT COUNT(*) FROM dictionary_entry",
        "snippets" => "SELECT COUNT(*) FROM snippet",
        "history" => "SELECT COUNT(*) FROM transcription_history WHERE status IS NOT 'failed'",
        "notes" => "SELECT COUNT(*) FROM note WHERE status = 'ready'",
        "scratchpad" => "SELECT COUNT(*) FROM scratch_note",
        _ => bail!("{kind} isn't kept in the database"),
    };
    Ok(conn.query_row(sql, [], |r| r.get(0))?)
}

/* -- dictionary -------------------------------------------------------------- */

fn export_dictionary(conn: &Connection) -> Result<Vec<Item>> {
    let mut heard: HashMap<i64, Vec<String>> = HashMap::new();
    {
        let mut stmt = conn.prepare("SELECT entry_id, heard FROM dictionary_heard")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (id, form) = row?;
            heard.entry(id).or_default().push(form);
        }
    }
    let mut stmt = conn.prepare(
        "SELECT id, text, case_sensitive, pinned, added_at, kind FROM dictionary_entry ORDER BY id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, bool>(2)?,
            r.get::<_, bool>(3)?,
            r.get::<_, i64>(4)?,
            r.get::<_, Option<String>>(5)?,
        ))
    })?;
    let mut items = Vec::new();
    for row in rows {
        let (id, text, case_sensitive, pinned, added_at, kind) = row?;
        let mut forms = heard.remove(&id).unwrap_or_default();
        forms.sort_by_key(|f| f.to_lowercase());
        // An unset kind goes as null, which hashes like no kind at all, so
        // words without one read as they did before kinds, and clearing one
        // reaches the other side.
        let body = json!({"text": text, "case_sensitive": case_sensitive, "pinned": pinned, "heard": forms, "kind": kind});
        items.push(Item {
            key: text.to_lowercase(),
            body,
            at: seconds_to_ms(added_at),
        });
    }
    Ok(items)
}

fn apply_dictionary(
    conn: &Connection,
    key: &str,
    body: Option<&Value>,
    now: i64,
) -> Result<Applied> {
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM dictionary_entry WHERE text = ?1 COLLATE NOCASE",
            [key],
            |r| r.get(0),
        )
        .optional()?;
    let Some(body) = body else {
        if let Some(id) = existing {
            conn.execute("DELETE FROM dictionary_heard WHERE entry_id = ?1", [id])?;
            conn.execute("DELETE FROM dictionary_entry WHERE id = ?1", [id])?;
        }
        return Ok(Applied::default());
    };
    let word = text(body, "text");
    if word.is_empty() {
        bail!("a word with no text");
    }
    // A side that doesn't know kinds sends none and leaves the one here
    // alone; null clears it.
    let (set_kind, kind) = match body.get("kind") {
        None => (false, None),
        Some(Value::Null) => (true, None),
        Some(k) => {
            let kind = k
                .as_str()
                .filter(|k| crate::managers::dictionary_store::WordKind::parse(k).is_some());
            (kind.is_some(), kind)
        }
    };
    let id = match existing {
        Some(id) => {
            conn.execute(
                "UPDATE dictionary_entry SET text = ?2, case_sensitive = ?3, pinned = ?4,
                    kind = CASE WHEN ?6 THEN ?5 ELSE kind END WHERE id = ?1",
                params![
                    id,
                    word,
                    flag(body, "case_sensitive"),
                    flag(body, "pinned"),
                    kind,
                    set_kind
                ],
            )?;
            conn.execute("DELETE FROM dictionary_heard WHERE entry_id = ?1", [id])?;
            id
        }
        None => {
            conn.execute(
                "INSERT INTO dictionary_entry (text, source, pinned, case_sensitive, added_at, kind)
                 VALUES (?1, 'manual', ?2, ?3, ?4, ?5)",
                params![
                    word,
                    flag(body, "pinned"),
                    flag(body, "case_sensitive"),
                    now,
                    kind
                ],
            )?;
            conn.last_insert_rowid()
        }
    };
    for form in body["heard"].as_array().into_iter().flatten() {
        if let Some(form) = form.as_str() {
            // A heard form belongs to one word; one taken here stays put.
            conn.execute(
                "INSERT OR IGNORE INTO dictionary_heard (entry_id, heard) VALUES (?1, ?2)",
                params![id, form],
            )?;
        }
    }
    Ok(Applied::default())
}

/* -- snippets ---------------------------------------------------------------- */

fn export_snippets(conn: &Connection) -> Result<Vec<Item>> {
    let mut stmt =
        conn.prepare("SELECT trigger, text, alone, added_at FROM snippet ORDER BY id")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, bool>(2)?,
            r.get::<_, i64>(3)?,
        ))
    })?;
    let mut items = Vec::new();
    for row in rows {
        let (trigger, text, alone, added_at) = row?;
        items.push(Item {
            key: trigger.to_lowercase(),
            body: json!({"trigger": trigger, "text": text, "alone": alone}),
            at: seconds_to_ms(added_at),
        });
    }
    Ok(items)
}

fn apply_snippet(conn: &Connection, key: &str, body: Option<&Value>, now: i64) -> Result<Applied> {
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM snippet WHERE trigger = ?1 COLLATE NOCASE ORDER BY id LIMIT 1",
            [key],
            |r| r.get(0),
        )
        .optional()?;
    match (body, existing) {
        (None, Some(id)) => {
            conn.execute("DELETE FROM snippet WHERE id = ?1", [id])?;
        }
        (None, None) => {}
        (Some(body), Some(id)) => {
            conn.execute(
                "UPDATE snippet SET trigger = ?2, text = ?3, alone = ?4 WHERE id = ?1",
                params![
                    id,
                    text(body, "trigger"),
                    text(body, "text"),
                    flag(body, "alone")
                ],
            )?;
        }
        (Some(body), None) => {
            conn.execute(
                "INSERT INTO snippet (trigger, text, alone, added_at) VALUES (?1, ?2, ?3, ?4)",
                params![
                    text(body, "trigger"),
                    text(body, "text"),
                    flag(body, "alone"),
                    now
                ],
            )?;
        }
    }
    Ok(Applied::default())
}

/* -- history ----------------------------------------------------------------- */

fn export_history(conn: &Connection, me: &str) -> Result<Vec<Item>> {
    let mut stmt = conn.prepare(
        "SELECT h.id, l.key, h.transcription_text, h.post_processed_text, h.heard_text,
            h.timestamp, h.title, h.app_name, h.category, h.word_count, h.speech_ms,
            h.model_name, h.transform, l.device, h.status
         FROM transcription_history h
         LEFT JOIN sync_local l ON l.kind = 'history' AND l.local_id = h.id
         WHERE h.status IS NOT 'failed'
         ORDER BY h.id",
    )?;
    let rows = stmt.query_map([], |r| {
        let id: i64 = r.get(0)?;
        let key: Option<String> = r.get(1)?;
        let at: i64 = r.get(5)?;
        let mut body = json!({
                "text": r.get::<_, String>(2)?,
                "processed": r.get::<_, Option<String>>(3)?,
                "heard": r.get::<_, Option<String>>(4)?,
                "at": at,
                "title": r.get::<_, String>(6)?,
                "app": r.get::<_, Option<String>>(7)?,
                "category": r.get::<_, Option<String>>(8)?,
                "words": r.get::<_, Option<i64>>(9)?,
                "speech_ms": r.get::<_, Option<i64>>(10)?,
                "model": r.get::<_, Option<String>>(11)?,
                "transform": r.get::<_, Option<String>>(12)?,
        });
        with_origin(&mut body, r.get(13)?);
        // Only when set, so other rows hash as they always did.
        if r.get::<_, Option<String>>(14)?.as_deref() == Some("cancelled") {
            body["cancelled"] = json!(true);
        }
        Ok(Item {
            key: key.unwrap_or_else(|| format!("{me}:{id}")),
            body,
            at: seconds_to_ms(at),
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn apply_history(
    conn: &Connection,
    key: &str,
    body: Option<&Value>,
    device: &str,
    me: &str,
) -> Result<Applied> {
    let local = ledger::local_id(conn, "history", key, me)?;
    let origin = body.and_then(|b| opt_text(b, "device"));
    let device = origin.as_deref().unwrap_or(device);
    let row: Option<(i64, String)> = match local {
        Some(id) => conn
            .query_row(
                "SELECT id, file_name FROM transcription_history WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?,
        None => None,
    };
    let Some(body) = body else {
        let Some((id, file)) = row else {
            return Ok(Applied::default());
        };
        conn.execute("DELETE FROM transcription_history WHERE id = ?1", [id])?;
        return Ok(Applied {
            removed_audio: (!file.is_empty()).then_some(file),
        });
    };
    let (said, processed, heard, at, title) = (
        text(body, "text"),
        opt_text(body, "processed"),
        opt_text(body, "heard"),
        int(body, "at").unwrap_or_default(),
        text(body, "title"),
    );
    let (app, category, words, speech_ms, model, transform) = (
        opt_text(body, "app"),
        opt_text(body, "category"),
        int(body, "words"),
        int(body, "speech_ms"),
        opt_text(body, "model"),
        opt_text(body, "transform"),
    );
    let fields = params![
        said, processed, heard, at, title, app, category, words, speech_ms, model, transform
    ];
    match row {
        Some((id, _)) => {
            conn.execute(
                &format!(
                    "UPDATE transcription_history SET transcription_text = ?1,
                        post_processed_text = ?2, heard_text = ?3, timestamp = ?4, title = ?5,
                        app_name = ?6, category = ?7, word_count = ?8, speech_ms = ?9,
                        model_name = ?10, transform = ?11 WHERE id = {id}"
                ),
                fields,
            )?;
            relabel(conn, "history", key, device, me)?;
        }
        // One of ours that's gone here (deleted, or past the history limit)
        // doesn't come back.
        None if local.is_some() => {}
        None => {
            let status = if body.get("cancelled").and_then(Value::as_bool) == Some(true) {
                "cancelled"
            } else {
                "not_pasted"
            };
            conn.execute(
                "INSERT INTO transcription_history (file_name, transcription_text,
                    post_processed_text, heard_text, timestamp, title, app_name, category,
                    word_count, speech_ms, model_name, transform, status)
                 VALUES ('', ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    said, processed, heard, at, title, app, category, words, speech_ms, model,
                    transform, status
                ],
            )?;
            ledger::map_local(conn, "history", conn.last_insert_rowid(), key, device)?;
        }
    }
    Ok(Applied::default())
}

/// A key this device made (`me:<id>`), as opposed to one mapped from
/// another device's.
fn is_own(key: &str, me: &str) -> bool {
    key.strip_prefix(me)
        .is_some_and(|rest| rest.starts_with(':'))
}

/* -- notes ------------------------------------------------------------------- */

fn export_notes(conn: &Connection, me: &str) -> Result<Vec<Item>> {
    let mut lines: HashMap<i64, Vec<Value>> = HashMap::new();
    {
        let mut stmt = conn.prepare(
            "SELECT note_id, at_ms, end_ms, who, text, speaker FROM note_line
             ORDER BY note_id, at_ms, id",
        )?;
        let rows = stmt.query_map([], |r| {
            let mut line = json!({"at_ms": r.get::<_, i64>(1)?, "end_ms": r.get::<_, i64>(2)?,
                       "who": r.get::<_, String>(3)?, "text": r.get::<_, String>(4)?});
            // Only when set, so a line without one hashes as it always did.
            if let Some(speaker) = r.get::<_, Option<i64>>(5)? {
                line["speaker"] = json!(speaker);
            }
            Ok((r.get::<_, i64>(0)?, line))
        })?;
        for row in rows {
            let (id, line) = row?;
            lines.entry(id).or_default().push(line);
        }
    }
    let mut tags: HashMap<i64, Vec<String>> = HashMap::new();
    {
        let mut stmt =
            conn.prepare("SELECT note_id, tag FROM note_tag ORDER BY tag COLLATE NOCASE")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (id, tag) = row?;
            tags.entry(id).or_default().push(tag);
        }
    }
    let mut speakers: HashMap<i64, Vec<Value>> = HashMap::new();
    {
        let mut stmt = conn
            .prepare("SELECT note_id, speaker, name FROM note_speaker ORDER BY note_id, speaker")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                json!({"speaker": r.get::<_, i64>(1)?, "name": r.get::<_, String>(2)?}),
            ))
        })?;
        for row in rows {
            let (id, speaker) = row?;
            speakers.entry(id).or_default().push(speaker);
        }
    }
    let mut actions: HashMap<i64, Vec<Value>> = HashMap::new();
    {
        let mut stmt = conn.prepare(
            "SELECT note_id, owner, text, done FROM note_action ORDER BY note_id, position, id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                json!({"owner": r.get::<_, String>(1)?, "text": r.get::<_, String>(2)?,
                       "done": r.get::<_, bool>(3)?}),
            ))
        })?;
        for row in rows {
            let (id, action) = row?;
            actions.entry(id).or_default().push(action);
        }
    }
    let mut stmt = conn.prepare(
        "SELECT n.id, l.key, n.title, n.kind, n.started_at, n.duration_ms, n.app, n.my_notes,
            n.summary, n.decisions, l.device, n.folder
         FROM note n LEFT JOIN sync_local l ON l.kind = 'notes' AND l.local_id = n.id
         WHERE n.status = 'ready' ORDER BY n.id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, i64>(4)?,
            r.get::<_, i64>(5)?,
            r.get::<_, Option<String>>(6)?,
            r.get::<_, String>(7)?,
            r.get::<_, String>(8)?,
            r.get::<_, String>(9)?,
            r.get::<_, Option<String>>(10)?,
            r.get::<_, Option<String>>(11)?,
        ))
    })?;
    let mut items = Vec::new();
    for row in rows {
        let (
            id,
            key,
            title,
            kind,
            started_at,
            duration_ms,
            app,
            my_notes,
            summary,
            decisions,
            origin,
            folder,
        ) = row?;
        let decisions: Value = serde_json::from_str(&decisions).unwrap_or_else(|_| json!([]));
        let mut body = json!({
            "title": title, "kind": kind, "started_at": started_at,
            "duration_ms": duration_ms, "app": app, "my_notes": my_notes,
            "summary": summary, "decisions": decisions,
            "tags": tags.remove(&id).unwrap_or_default(),
            "actions": actions.remove(&id).unwrap_or_default(),
            "lines": lines.remove(&id).unwrap_or_default(),
        });
        // Names given to voices, only when there are any (see the lines).
        if let Some(names) = speakers.remove(&id) {
            body["speakers"] = json!(names);
        }
        // Likewise the folder.
        if let Some(folder) = folder {
            body["folder"] = json!(folder);
        }
        with_origin(&mut body, origin);
        items.push(Item {
            key: key.unwrap_or_else(|| format!("{me}:{id}")),
            body,
            at: seconds_to_ms(started_at),
        });
    }
    Ok(items)
}

fn apply_note(
    conn: &Connection,
    key: &str,
    body: Option<&Value>,
    device: &str,
    me: &str,
) -> Result<Applied> {
    let local = ledger::local_id(conn, "notes", key, me)?;
    let origin = body.and_then(|b| opt_text(b, "device"));
    let device = origin.as_deref().unwrap_or(device);
    let row: Option<(i64, Option<String>)> = match local {
        Some(id) => conn
            .query_row("SELECT id, audio_file FROM note WHERE id = ?1", [id], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()?,
        None => None,
    };
    let clear = |id: i64| -> Result<()> {
        for table in [
            "note_line",
            "note_tag",
            "note_action",
            "note_failed_line",
            "note_speaker",
        ] {
            conn.execute(&format!("DELETE FROM {table} WHERE note_id = ?1"), [id])?;
        }
        Ok(())
    };
    let Some(body) = body else {
        let Some((id, audio)) = row else {
            return Ok(Applied::default());
        };
        clear(id)?;
        conn.execute("DELETE FROM note WHERE id = ?1", [id])?;
        return Ok(Applied {
            removed_audio: audio,
        });
    };
    let kind = if text(body, "kind") == "meeting" {
        "meeting"
    } else {
        "memo"
    };
    let decisions =
        serde_json::to_string(&body["decisions"].as_array().cloned().unwrap_or_default())?;
    let (title, started_at, duration_ms) = (
        text(body, "title"),
        int(body, "started_at").unwrap_or_default(),
        int(body, "duration_ms").unwrap_or_default(),
    );
    let (app, my_notes, summary) = (
        opt_text(body, "app"),
        text(body, "my_notes"),
        text(body, "summary"),
    );
    let folder = opt_text(body, "folder").filter(|f| !f.trim().is_empty());
    if let Some(folder) = &folder {
        conn.execute(
            "INSERT OR IGNORE INTO note_folder (name, created_at) VALUES (?1, ?2)",
            params![folder, started_at],
        )?;
    }
    let fields = params![
        title,
        kind,
        started_at,
        duration_ms,
        app,
        my_notes,
        summary,
        decisions,
        folder
    ];
    let id = match row {
        Some((id, _)) => {
            conn.execute(
                &format!(
                    "UPDATE note SET title = ?1, kind = ?2, started_at = ?3, duration_ms = ?4,
                        app = ?5, my_notes = ?6, summary = ?7, decisions = ?8, folder = ?9
                     WHERE id = {id}"
                ),
                fields,
            )?;
            relabel(conn, "notes", key, device, me)?;
            clear(id)?;
            id
        }
        // One of ours that's gone here stays gone. Another device's that we
        // removed comes back: this version is newer than the removal (a
        // note restored from Recently deleted there).
        None if local.is_some() && is_own(key, me) => return Ok(Applied::default()),
        None => {
            conn.execute(
                "INSERT INTO note (title, kind, status, started_at, duration_ms, app, my_notes,
                    summary, decisions, folder)
                 VALUES (?1, ?2, 'ready', ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                fields,
            )?;
            let id = conn.last_insert_rowid();
            ledger::map_local(conn, "notes", id, key, device)?;
            id
        }
    };
    for line in body["lines"].as_array().into_iter().flatten() {
        let who = if text(line, "who") == "them" {
            "them"
        } else {
            "you"
        };
        conn.execute(
            "INSERT INTO note_line (note_id, at_ms, end_ms, who, text, speaker)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id,
                int(line, "at_ms").unwrap_or_default(),
                int(line, "end_ms").unwrap_or_default(),
                who,
                text(line, "text"),
                int(line, "speaker")
            ],
        )?;
    }
    for speaker in body["speakers"].as_array().into_iter().flatten() {
        if let Some(n) = int(speaker, "speaker") {
            conn.execute(
                "INSERT OR REPLACE INTO note_speaker (note_id, speaker, name) VALUES (?1, ?2, ?3)",
                params![id, n, text(speaker, "name")],
            )?;
        }
    }
    for tag in body["tags"].as_array().into_iter().flatten() {
        if let Some(tag) = tag.as_str() {
            conn.execute(
                "INSERT OR IGNORE INTO note_tag (note_id, tag) VALUES (?1, ?2)",
                params![id, tag],
            )?;
        }
    }
    for (position, action) in body["actions"].as_array().into_iter().flatten().enumerate() {
        conn.execute(
            "INSERT INTO note_action (note_id, owner, text, done, position) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, text(action, "owner"), text(action, "text"), flag(action, "done"), position as i64],
        )?;
    }
    Ok(Applied::default())
}

/* -- scratchpad -------------------------------------------------------------- */

fn export_scratchpad(conn: &Connection, me: &str) -> Result<Vec<Item>> {
    let mut stmt = conn.prepare(
        "SELECT s.id, l.key, s.title, s.auto_title, s.text, s.pinned, s.created_at, s.updated_at
         FROM scratch_note s LEFT JOIN sync_local l ON l.kind = 'scratchpad' AND l.local_id = s.id
         ORDER BY s.id",
    )?;
    let rows = stmt.query_map([], |r| {
        let id: i64 = r.get(0)?;
        let key: Option<String> = r.get(1)?;
        let updated_at: i64 = r.get(7)?;
        Ok(Item {
            key: key.unwrap_or_else(|| format!("{me}:{id}")),
            body: json!({
                "title": r.get::<_, String>(2)?,
                "auto_title": r.get::<_, bool>(3)?,
                "text": r.get::<_, String>(4)?,
                "pinned": r.get::<_, bool>(5)?,
                "created_at": r.get::<_, i64>(6)?,
                "updated_at": updated_at,
            }),
            at: seconds_to_ms(updated_at),
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn apply_scratch(
    conn: &Connection,
    key: &str,
    body: Option<&Value>,
    device: &str,
    me: &str,
    now: i64,
) -> Result<Applied> {
    let local = ledger::local_id(conn, "scratchpad", key, me)?;
    let row: Option<(i64, String)> = match local {
        Some(id) => conn
            .query_row(
                "SELECT id, text FROM scratch_note WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?,
        None => None,
    };
    let Some(body) = body else {
        if let Some((id, _)) = row {
            conn.execute("DELETE FROM scratch_version WHERE note_id = ?1", [id])?;
            conn.execute("DELETE FROM scratch_note WHERE id = ?1", [id])?;
        }
        return Ok(Applied::default());
    };
    let new_text = text(body, "text");
    match row {
        Some((id, old_text)) => {
            // The text it replaces stays in the note's versions.
            if old_text != new_text && !old_text.trim().is_empty() {
                conn.execute(
                    "INSERT INTO scratch_version (note_id, text, at) VALUES (?1, ?2, ?3)",
                    params![id, old_text, now],
                )?;
            }
            conn.execute(
                "UPDATE scratch_note SET title = ?2, auto_title = ?3, text = ?4, pinned = ?5,
                    updated_at = ?6 WHERE id = ?1",
                params![
                    id,
                    text(body, "title"),
                    flag(body, "auto_title"),
                    new_text,
                    flag(body, "pinned"),
                    int(body, "updated_at").unwrap_or(now)
                ],
            )?;
        }
        None if local.is_some() && is_own(key, me) => {}
        None => {
            conn.execute(
                "INSERT INTO scratch_note (title, auto_title, text, pinned, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    text(body, "title"),
                    flag(body, "auto_title"),
                    new_text,
                    flag(body, "pinned"),
                    int(body, "created_at").unwrap_or(now),
                    int(body, "updated_at").unwrap_or(now)
                ],
            )?;
            ledger::map_local(conn, "scratchpad", conn.last_insert_rowid(), key, device)?;
        }
    }
    Ok(Applied::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::managers::history_store::MIGRATIONS;
    use rusqlite_migration::Migrations;

    fn db() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        Migrations::new(MIGRATIONS.to_vec())
            .to_latest(&mut conn)
            .unwrap();
        conn
    }

    /// Exporting on one device and applying on another gives the same
    /// export, for every database kind.
    #[test]
    fn every_kind_survives_the_trip_to_another_device() {
        let pc = db();
        pc.execute_batch(
            "INSERT INTO dictionary_entry (text, pinned, case_sensitive, added_at) VALUES ('Datev', 1, 0, 100);
             INSERT INTO dictionary_heard (entry_id, heard) VALUES (1, 'date of'), (1, 'Data V');
             INSERT INTO snippet (trigger, text, alone, added_at) VALUES ('my address', 'Hauptstr. 1', 1, 100);
             INSERT INTO transcription_history (file_name, timestamp, title, transcription_text, app_name, word_count, status)
                VALUES ('a.wav', 1700000000, 'Mon', 'send the invoice', 'Slack', 3, 'pasted'),
                       ('b.wav', 1700000100, 'Mon', '', NULL, 0, 'failed');
             INSERT INTO note (title, kind, status, started_at, duration_ms, decisions)
                VALUES ('Standup', 'meeting', 'ready', 1700000000, 60000, '[\"Ship Friday\"]'),
                       ('Live', 'memo', 'recording', 1700000500, 0, '[]');
             INSERT INTO note_line (note_id, at_ms, end_ms, who, text) VALUES (1, 0, 900, 'them', 'Morning');
             INSERT INTO note_tag (note_id, tag) VALUES (1, 'team');
             INSERT INTO note_action (note_id, owner, text, done, position) VALUES (1, 'Abhash', 'Book room', 0, 0);
             INSERT INTO scratch_note (title, text, created_at, updated_at) VALUES ('Ideas', 'Datev integration', 1, 2);",
        )
        .unwrap();
        let mut phone = db();
        for kind in DB_KINDS {
            let items = export(&pc, kind, "pc").unwrap();
            for item in &items {
                apply(
                    &mut phone,
                    kind,
                    &item.key,
                    Some(&item.body),
                    "Desk PC",
                    "phone",
                    5,
                )
                .unwrap();
            }
            let back = export(&phone, kind, "phone").unwrap();
            // Rows from the PC name it on the phone; the hash ignores that.
            let strip = |v: &[Item]| {
                v.iter()
                    .map(|i| {
                        let mut body = i.body.clone();
                        if let Some(map) = body.as_object_mut() {
                            map.remove("device");
                        }
                        (i.key.clone(), body)
                    })
                    .collect::<Vec<_>>()
            };
            assert_eq!(strip(&back), strip(&items), "{kind}");
        }
        assert_eq!(count(&phone, "history").unwrap(), 1);
        assert_eq!(count(&phone, "notes").unwrap(), 1);
        let device: String = phone
            .query_row(
                "SELECT device FROM sync_local WHERE kind = 'history'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(device, "Desk PC");
        // Applying the same version again changes nothing.
        let items = export(&pc, "notes", "pc").unwrap();
        apply(
            &mut phone,
            "notes",
            &items[0].key,
            Some(&items[0].body),
            "Desk PC",
            "phone",
            6,
        )
        .unwrap();
        assert_eq!(count(&phone, "notes").unwrap(), 1);
        let lines: i64 = phone
            .query_row("SELECT COUNT(*) FROM note_line", [], |r| r.get(0))
            .unwrap();
        assert_eq!(lines, 1);
    }

    /// A phone's dictation the PC passes on to a tablet is the phone's on
    /// the tablet too, not the PC's.
    #[test]
    fn a_passed_on_row_keeps_the_device_it_came_from() {
        let mut pc = db();
        let body = json!({"text": "Book the room", "at": 1, "title": "t", "device": "Pixel 8"});
        apply(
            &mut pc,
            "history",
            "phone:7",
            Some(&body),
            "Pixel 8",
            "pc",
            2,
        )
        .unwrap();
        let items = export(&pc, "history", "pc").unwrap();
        assert_eq!(items[0].body["device"], "Pixel 8");
        let mut tablet = db();
        apply(
            &mut tablet,
            "history",
            &items[0].key,
            Some(&items[0].body),
            "Desk PC",
            "tablet",
            3,
        )
        .unwrap();
        let device: String = tablet
            .query_row(
                "SELECT device FROM sync_local WHERE kind = 'history'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(device, "Pixel 8");
        // The PC's own rows go out unnamed, so their hash is what it was.
        pc.execute(
            "INSERT INTO transcription_history (file_name, transcription_text, timestamp, title)
             VALUES ('', 'Mine', 4, 't')",
            [],
        )
        .unwrap();
        let mine = export(&pc, "history", "pc").unwrap();
        assert!(mine
            .iter()
            .any(|i| i.key.starts_with("pc:") && i.body.get("device").is_none()));
    }

    #[test]
    fn deletes_remove_rows_and_hand_back_their_recordings() {
        let mut conn = db();
        conn.execute_batch(
            "INSERT INTO transcription_history (file_name, timestamp, title, transcription_text)
                VALUES ('a.wav', 1, 't', 'hello');
             INSERT INTO dictionary_entry (text, added_at) VALUES ('Datev', 1);
             INSERT INTO dictionary_heard (entry_id, heard) VALUES (1, 'date of');",
        )
        .unwrap();
        let gone = apply(&mut conn, "history", "pc:1", None, "Pixel 8", "pc", 2).unwrap();
        assert_eq!(gone.removed_audio.as_deref(), Some("a.wav"));
        apply(&mut conn, "dictionary", "datev", None, "Pixel 8", "pc", 2).unwrap();
        let heard: i64 = conn
            .query_row("SELECT COUNT(*) FROM dictionary_heard", [], |r| r.get(0))
            .unwrap();
        assert_eq!(heard, 0);
        // Deleting what isn't here is fine.
        apply(&mut conn, "scratchpad", "phone:4", None, "Pixel 8", "pc", 2).unwrap();
    }

    #[test]
    fn an_edit_for_one_of_ours_that_is_gone_does_not_bring_it_back() {
        let mut conn = db();
        let body = json!({"text": "old dictation", "at": 1, "title": "t"});
        apply(
            &mut conn,
            "history",
            "pc:41",
            Some(&body),
            "Pixel 8",
            "pc",
            2,
        )
        .unwrap();
        assert_eq!(count(&conn, "history").unwrap(), 0);
        apply(
            &mut conn,
            "history",
            "phone:41",
            Some(&body),
            "Pixel 8",
            "pc",
            2,
        )
        .unwrap();
        assert_eq!(count(&conn, "history").unwrap(), 1);
    }

    #[test]
    fn a_word_kind_set_kept_and_cleared() {
        let mut conn = db();
        let kind = |conn: &Connection| -> Option<String> {
            conn.query_row("SELECT kind FROM dictionary_entry", [], |r| r.get(0))
                .unwrap()
        };
        let word = |kind: Value| {
            let mut body = json!({"text": "AIPS", "heard": ["IPES"]});
            if !kind.is_string() || kind != "absent" {
                body["kind"] = kind;
            }
            body
        };
        let mut send = |body: Value, at| {
            apply(
                &mut conn,
                "dictionary",
                "aips",
                Some(&body),
                "PC",
                "phone",
                at,
            )
            .unwrap();
        };
        send(word(json!("project")), 1);
        // A device that doesn't know kinds leaves it; null clears it.
        send(word(json!("absent")), 2);
        send(word(Value::Null), 3);
        drop(send);
        assert_eq!(kind(&conn), None);
        apply(
            &mut conn,
            "dictionary",
            "aips",
            Some(&word(json!("project"))),
            "PC",
            "phone",
            4,
        )
        .unwrap();
        apply(
            &mut conn,
            "dictionary",
            "aips",
            Some(&word(json!("absent"))),
            "PC",
            "phone",
            5,
        )
        .unwrap();
        assert_eq!(kind(&conn).as_deref(), Some("project"));
    }

    #[test]
    fn a_note_restored_on_the_other_device_comes_back() {
        let mut conn = db();
        let body = json!({"title": "Weekly sync", "kind": "meeting", "lines": []});
        for kind in ["notes", "scratchpad"] {
            // The PC's note arrives on the phone, is deleted there by the
            // PC's delete, then restored on the PC with a newer version.
            apply(&mut conn, kind, "pc:12", Some(&body), "PC", "phone", 1).unwrap();
            assert_eq!(count(&conn, kind).unwrap(), 1);
            apply(&mut conn, kind, "pc:12", None, "PC", "phone", 2).unwrap();
            assert_eq!(count(&conn, kind).unwrap(), 0);
            apply(&mut conn, kind, "pc:12", Some(&body), "PC", "phone", 3).unwrap();
            assert_eq!(count(&conn, kind).unwrap(), 1, "{kind}");
            // And it's that row an edit updates, not a second one.
            apply(&mut conn, kind, "pc:12", Some(&body), "PC", "phone", 4).unwrap();
            assert_eq!(count(&conn, kind).unwrap(), 1, "{kind}");
        }
    }

    #[test]
    fn a_scratchpad_note_keeps_the_text_another_device_replaced() {
        let mut conn = db();
        conn.execute(
            "INSERT INTO scratch_note (title, text, created_at, updated_at) VALUES ('Ideas', 'mine', 1, 1)",
            [],
        )
        .unwrap();
        let theirs = json!({"title": "Ideas", "auto_title": true, "text": "theirs", "pinned": false, "created_at": 1, "updated_at": 9});
        apply(
            &mut conn,
            "scratchpad",
            "pc:1",
            Some(&theirs),
            "Pixel 8",
            "pc",
            9,
        )
        .unwrap();
        let (text, kept): (String, String) = conn
            .query_row(
                "SELECT s.text, v.text FROM scratch_note s JOIN scratch_version v ON v.note_id = s.id",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((text.as_str(), kept.as_str()), ("theirs", "mine"));
    }

    #[test]
    fn labels_name_the_item() {
        assert_eq!(
            label("dictionary", Some(&json!({"text": "Datev"}))),
            "Datev"
        );
        assert_eq!(
            label("scratchpad", Some(&json!({"title": "", "text": "a note"}))),
            "a note"
        );
        let long = "x".repeat(80);
        assert_eq!(
            label("history", Some(&json!({"text": long})))
                .chars()
                .count(),
            61
        );
        assert_eq!(label("notes", None), "");
    }
}
