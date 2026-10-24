//! The sync ledger: the last known version of every item that syncs, each
//! with a sequence number, so a phone can ask "what changed since 120?".
//!
//! Local edits aren't hooked one by one. `refresh` compares what the stores
//! hold now against the ledger and records whatever differs, so every store
//! keeps working as it did. Changes from the phone go through `judge` (newest
//! wins) and `adopt`. Plain rusqlite on the history database; see
//! docs/phone-sync.md.

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use specta::Type;

use super::protocol::Change;

/// How many clips both sides keep.
pub const KEEP_CLIPS: usize = 20;
/// The most body bytes one `changes` answer carries, well under a frame.
const BATCH_BYTES: usize = 4 * 1024 * 1024;
/// The largest item synced: a batch just under `BATCH_BYTES` plus one of
/// these still fits a frame.
const MAX_ITEM_BYTES: usize = 8 * 1024 * 1024;

/// One item as a store holds it now.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub key: String,
    pub body: Value,
    /// When it was made or last changed, in milliseconds, if the store knows.
    /// Used the first time the ledger sees the item.
    pub at: Option<i64>,
}

/// JSON with object keys sorted, so equal bodies hash the same. A field
/// that's null reads as one that isn't there, so a field added later (sent
/// as null when unset) changes nothing for items that don't use it.
pub fn canonical(value: &Value) -> String {
    fn sorted(value: &Value) -> Value {
        match value {
            Value::Object(map) => {
                let mut keys: Vec<&String> = map.keys().filter(|k| !map[*k].is_null()).collect();
                keys.sort();
                Value::Object(
                    keys.into_iter()
                        .map(|k| (k.clone(), sorted(&map[k])))
                        .collect(),
                )
            }
            Value::Array(items) => Value::Array(items.iter().map(sorted).collect()),
            other => other.clone(),
        }
    }
    sorted(value).to_string()
}

fn hash_text(text: Option<&str>) -> String {
    match text {
        Some(text) => Sha256::digest(text.as_bytes())[..16]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
        None => "-".into(),
    }
}

/// The name of the device a row came from rides along with it (see
/// `kinds::with_origin`) but isn't part of what changed: two devices that
/// disagree on it, one of them older than the field, must not keep sending
/// the row back and forth.
pub fn hash(body: Option<&Value>) -> String {
    match body {
        Some(Value::Object(map)) if map.contains_key("device") => {
            let mut map = map.clone();
            map.remove("device");
            hash_text(Some(&canonical(&Value::Object(map))))
        }
        _ => hash_text(body.map(canonical).as_deref()),
    }
}

struct Row {
    hash: String,
    changed_at: i64,
    origin: String,
    seq: i64,
}

fn row(conn: &Connection, kind: &str, key: &str) -> Result<Option<Row>> {
    Ok(conn
        .query_row(
            "SELECT hash, changed_at, origin, seq FROM sync_item WHERE kind = ?1 AND key = ?2",
            params![kind, key],
            |r| {
                Ok(Row {
                    hash: r.get(0)?,
                    changed_at: r.get(1)?,
                    origin: r.get(2)?,
                    seq: r.get(3)?,
                })
            },
        )
        .optional()?)
}

pub fn max_seq(conn: &Connection) -> Result<u64> {
    Ok(
        conn.query_row("SELECT COALESCE(MAX(seq), 0) FROM sync_item", [], |r| {
            r.get::<_, i64>(0)
        })? as u64,
    )
}

fn put(conn: &Connection, change: &Change, seq: i64) -> Result<()> {
    let body = change.body.as_ref().map(canonical);
    // The hash `refresh` and `judge` compare with, which leaves out the
    // device; hashing the body as stored made every row from another device
    // look edited at each refresh, so the two sent it back and forth forever.
    conn.execute(
        "INSERT INTO sync_item (kind, key, hash, body, changed_at, origin, seq)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT (kind, key) DO UPDATE SET hash = ?3, body = ?4, changed_at = ?5,
            origin = ?6, seq = ?7",
        params![
            change.kind,
            change.key,
            hash(change.body.as_ref()),
            body,
            change.changed_at,
            change.origin,
            seq
        ],
    )?;
    Ok(())
}

/// Records what changed in one kind since the last refresh: new and edited
/// items, and deletes. With `infer_deletes` an item missing from `items` is a
/// delete; without, only `gone` keys are (history, which also loses old
/// dictations to the history limit). Returns how many items changed.
pub fn refresh(
    conn: &mut Connection,
    kind: &str,
    items: &[Item],
    infer_deletes: bool,
    gone: &[String],
    me: &str,
    now: i64,
) -> Result<usize> {
    let tx = conn.transaction()?;
    let known: HashMap<String, (String, i64)> = {
        let mut stmt = tx.prepare("SELECT key, hash, changed_at FROM sync_item WHERE kind = ?1")?;
        let rows = stmt.query_map([kind], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    let mut seq = max_seq(&tx)? as i64;
    let mut changed = 0;
    // A later edit must beat the version it replaced, even with a clock
    // that runs behind the other device's.
    let at = |before: Option<i64>, hint: Option<i64>| match before {
        Some(before) => now.max(before + 1),
        None => hint.unwrap_or(now),
    };
    let mut present = HashSet::new();
    for item in items {
        present.insert(item.key.as_str());
        let h = hash(Some(&item.body));
        let before = known.get(&item.key);
        if before.is_some_and(|(old, _)| *old == h) {
            continue;
        }
        seq += 1;
        changed += 1;
        let change = Change {
            kind: kind.into(),
            key: item.key.clone(),
            body: Some(item.body.clone()),
            changed_at: at(before.map(|(_, t)| *t), item.at),
            origin: me.into(),
        };
        put(&tx, &change, seq)?;
    }
    let deleted: Vec<&String> = if infer_deletes {
        known
            .keys()
            .filter(|k| !present.contains(k.as_str()))
            .collect()
    } else {
        gone.iter().filter(|k| known.contains_key(*k)).collect()
    };
    for key in deleted {
        let (old, before) = &known[key];
        if old == "-" {
            continue;
        }
        seq += 1;
        changed += 1;
        let change = Change {
            kind: kind.into(),
            key: key.clone(),
            body: None,
            changed_at: at(Some(*before), None),
            origin: me.into(),
        };
        put(&tx, &change, seq)?;
    }
    tx.commit()?;
    Ok(changed)
}

/// Changes after `after` in `kinds`, oldest first, leaving out versions
/// that came from `skip_origin` (the device asking already has them).
/// Returns the items, the sequence number to ask from next, and whether
/// there are more.
pub fn changes_since(
    conn: &Connection,
    after: u64,
    kinds: &[String],
    skip_origin: &str,
    limit: usize,
) -> Result<(Vec<Change>, u64, bool)> {
    let mut stmt = conn.prepare(
        "SELECT kind, key, body, changed_at, origin, seq FROM sync_item
         WHERE seq > ?1 ORDER BY seq LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![after as i64, limit as i64 + 1], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, i64>(5)?,
        ))
    })?;
    let mut items = Vec::new();
    let mut upto = after;
    let mut bytes = 0;
    let mut more = false;
    for row in rows {
        let (kind, key, body, changed_at, origin, seq) = row?;
        if items.len() >= limit || (bytes >= BATCH_BYTES && !items.is_empty()) {
            more = true;
            break;
        }
        upto = seq as u64;
        if origin == skip_origin || !kinds.contains(&kind) {
            continue;
        }
        // One item over the frame limit would fail every batch it's in,
        // and the link would drop and reconnect forever. It stays here.
        if body.as_ref().is_some_and(|b| b.len() > MAX_ITEM_BYTES) {
            log::warn!("Not syncing {kind} item {key}: it's over {MAX_ITEM_BYTES} bytes");
            continue;
        }
        bytes += body.as_ref().map_or(0, String::len);
        items.push(Change {
            kind,
            key,
            body: body.map(|b| serde_json::from_str(&b)).transpose()?,
            changed_at,
            origin,
        });
    }
    Ok((items, upto, more))
}

/// What to do with a change from another device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Already the same here.
    Same,
    /// Theirs is newer: apply it, then `adopt` it.
    Theirs { merged: bool },
    /// Ours is newer: keep it; they get it on their next pull.
    Ours { merged: bool },
}

/// Newest wins; on a tie the larger origin, so both sides agree. `merged`
/// is set when this device also changed the item since `sent_upto`, the
/// last version the other device confirmed it holds.
pub fn judge(conn: &Connection, me: &str, sent_upto: u64, change: &Change) -> Result<Verdict> {
    let Some(local) = row(conn, &change.kind, &change.key)? else {
        return Ok(Verdict::Theirs { merged: false });
    };
    if local.hash == hash(change.body.as_ref()) {
        return Ok(Verdict::Same);
    }
    let merged = local.origin == me && local.seq as u64 > sent_upto;
    let theirs_newer =
        (change.changed_at, change.origin.as_str()) > (local.changed_at, local.origin.as_str());
    Ok(if theirs_newer {
        Verdict::Theirs { merged }
    } else {
        Verdict::Ours { merged }
    })
}

/// Takes another device's version into the ledger once it's applied, with a
/// new sequence number so other devices can pull it.
pub fn adopt(conn: &Connection, change: &Change) -> Result<()> {
    let seq = max_seq(conn)? as i64 + 1;
    put(conn, change, seq)
}

/// Gives every item of `kind` a new sequence number, so devices that skipped
/// it while it was switched off get all of it.
pub fn resequence(conn: &mut Connection, kind: &str) -> Result<()> {
    let tx = conn.transaction()?;
    let start = max_seq(&tx)? as i64;
    tx.execute(
        "UPDATE sync_item SET seq = ?2 + (SELECT COUNT(*) FROM sync_item AS s
            WHERE s.kind = ?1 AND s.rowid <= sync_item.rowid)
         WHERE kind = ?1",
        params![kind, start],
    )?;
    tx.commit()?;
    Ok(())
}

/* -- local rows and their keys ------------------------------------------- */

/// The key of a local row: the one it came with from another device, else
/// `<this device>:<id>`.
pub fn key_of(conn: &Connection, kind: &str, local_id: i64, me: &str) -> Result<String> {
    Ok(conn
        .query_row(
            "SELECT key FROM sync_local WHERE kind = ?1 AND local_id = ?2",
            params![kind, local_id],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or_else(|| format!("{me}:{local_id}")))
}

/// The local row for a key, if there is one.
pub fn local_id(conn: &Connection, kind: &str, key: &str, me: &str) -> Result<Option<i64>> {
    if let Some(id) = conn
        .query_row(
            "SELECT local_id FROM sync_local WHERE kind = ?1 AND key = ?2",
            params![kind, key],
            |r| r.get(0),
        )
        .optional()?
    {
        return Ok(Some(id));
    }
    Ok(key
        .strip_prefix(me)
        .and_then(|rest| rest.strip_prefix(':'))
        .and_then(|id| id.parse().ok()))
}

/// Remembers that a local row came from another device, as `key`, made on
/// the device called `device`.
pub fn map_local(
    conn: &Connection,
    kind: &str,
    local_id: i64,
    key: &str,
    device: &str,
) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO sync_local (kind, local_id, key, device) VALUES (?1, ?2, ?3, ?4)",
        params![kind, local_id, key, device],
    )?;
    Ok(())
}

/// Marks a row the person deleted, for kinds that don't infer deletes.
pub fn forget(conn: &Connection, kind: &str, local_id: i64, now: i64) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO sync_gone (kind, local_id, at) VALUES (?1, ?2, ?3)",
        params![kind, local_id, now],
    )?;
    Ok(())
}

/// The keys of rows deleted since the last call, clearing the list.
pub fn take_gone(conn: &Connection, kind: &str, me: &str) -> Result<Vec<String>> {
    let ids: Vec<i64> = {
        let mut stmt = conn.prepare("SELECT local_id FROM sync_gone WHERE kind = ?1")?;
        let rows = stmt.query_map([kind], |r| r.get(0))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    let keys = ids
        .iter()
        .map(|&id| key_of(conn, kind, id, me))
        .collect::<Result<_>>()?;
    conn.execute("DELETE FROM sync_gone WHERE kind = ?1", [kind])?;
    Ok(keys)
}

/* -- paired devices ------------------------------------------------------- */

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct Peer {
    pub device_id: String,
    pub name: String,
    pub platform: String,
    pub app_version: String,
    pub fingerprint: String,
    /// Seconds since 1970, like every time below.
    pub paired_at: i64,
    pub last_seen: Option<i64>,
    pub last_sync: Option<i64>,
    /// The newest of our changes it confirmed it holds.
    #[specta(type = f64)]
    pub sent_upto: u64,
    /// The newest of its changes we hold.
    #[specta(type = f64)]
    pub recv_upto: u64,
}

const PEER_COLUMNS: &str = "device_id, name, platform, app_version, fingerprint, paired_at,
    last_seen, last_sync, sent_upto, recv_upto";

fn map_peer(r: &rusqlite::Row<'_>) -> rusqlite::Result<Peer> {
    Ok(Peer {
        device_id: r.get(0)?,
        name: r.get(1)?,
        platform: r.get(2)?,
        app_version: r.get(3)?,
        fingerprint: r.get(4)?,
        paired_at: r.get(5)?,
        last_seen: r.get(6)?,
        last_sync: r.get(7)?,
        sent_upto: r.get::<_, i64>(8)? as u64,
        recv_upto: r.get::<_, i64>(9)? as u64,
    })
}

pub fn add_peer(conn: &Connection, peer: &Peer) -> Result<()> {
    conn.execute(
        &format!("INSERT OR REPLACE INTO sync_peer ({PEER_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"),
        params![
            peer.device_id,
            peer.name,
            peer.platform,
            peer.app_version,
            peer.fingerprint,
            peer.paired_at,
            peer.last_seen,
            peer.last_sync,
            peer.sent_upto as i64,
            peer.recv_upto as i64
        ],
    )?;
    Ok(())
}

pub fn peers(conn: &Connection) -> Result<Vec<Peer>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {PEER_COLUMNS} FROM sync_peer ORDER BY paired_at"
    ))?;
    let rows = stmt.query_map([], map_peer)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn peer(conn: &Connection, device_id: &str) -> Result<Option<Peer>> {
    Ok(conn
        .query_row(
            &format!("SELECT {PEER_COLUMNS} FROM sync_peer WHERE device_id = ?1"),
            [device_id],
            map_peer,
        )
        .optional()?)
}

pub fn remove_peer(conn: &Connection, device_id: &str) -> Result<()> {
    conn.execute("DELETE FROM sync_peer WHERE device_id = ?1", [device_id])?;
    Ok(())
}

pub fn rename_peer(conn: &Connection, device_id: &str, name: &str) -> Result<()> {
    conn.execute(
        "UPDATE sync_peer SET name = ?2 WHERE device_id = ?1",
        params![device_id, name],
    )?;
    Ok(())
}

/// What the device says about itself each time it connects.
pub fn update_peer(conn: &Connection, device_id: &str, app_version: &str, now: i64) -> Result<()> {
    conn.execute(
        "UPDATE sync_peer SET app_version = ?2, last_seen = ?3 WHERE device_id = ?1",
        params![device_id, app_version, now],
    )?;
    Ok(())
}

pub fn set_sent_upto(conn: &Connection, device_id: &str, upto: u64, now: i64) -> Result<()> {
    conn.execute(
        "UPDATE sync_peer SET sent_upto = MAX(sent_upto, ?2), last_seen = ?3, last_sync = ?3
         WHERE device_id = ?1",
        params![device_id, upto as i64, now],
    )?;
    Ok(())
}

pub fn set_recv_upto(conn: &Connection, device_id: &str, upto: u64, now: i64) -> Result<()> {
    conn.execute(
        "UPDATE sync_peer SET recv_upto = ?2, last_seen = ?3, last_sync = ?3
         WHERE device_id = ?1",
        params![device_id, upto as i64, now],
    )?;
    Ok(())
}

/// Starts every device over from the beginning of the other's changes, for
/// a kind that was just switched back on.
pub fn restart_peers(conn: &Connection) -> Result<()> {
    conn.execute("UPDATE sync_peer SET recv_upto = 0", [])?;
    Ok(())
}

/* -- merges ---------------------------------------------------------------- */

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct Merge {
    pub id: i64,
    pub at: i64,
    /// The device name at the time.
    pub peer: String,
    pub kind: String,
    pub label: String,
    /// True when the other device's version was kept.
    pub kept_theirs: bool,
}

pub fn log_merge(
    conn: &Connection,
    at: i64,
    peer: &str,
    kind: &str,
    key: &str,
    label: &str,
    kept_theirs: bool,
) -> Result<()> {
    conn.execute(
        "INSERT INTO sync_merge (at, peer, kind, key, label, kept_theirs)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![at, peer, kind, key, label, kept_theirs],
    )?;
    conn.execute(
        "DELETE FROM sync_merge WHERE id NOT IN (SELECT id FROM sync_merge ORDER BY id DESC LIMIT 50)",
        [],
    )?;
    Ok(())
}

pub fn merges(conn: &Connection) -> Result<Vec<Merge>> {
    let mut stmt = conn.prepare(
        "SELECT id, at, peer, kind, label, kept_theirs FROM sync_merge ORDER BY id DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Merge {
            id: r.get(0)?,
            at: r.get(1)?,
            peer: r.get(2)?,
            kind: r.get(3)?,
            label: r.get(4)?,
            kept_theirs: r.get(5)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/* -- recent clips ---------------------------------------------------------- */

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct ClipRow {
    pub id: String,
    pub at: i64,
    /// The phone's name.
    pub peer: String,
    /// True for clips from the phone, false for clips sent to it.
    pub incoming: bool,
    /// "text" or "image".
    pub kind: String,
    pub text: Option<String>,
    /// The PNG's file name in the clips folder.
    pub image: Option<String>,
    #[specta(type = f64)]
    pub bytes: u64,
    /// Arrived too late for the clipboard.
    pub late: bool,
    pub from_app: Option<String>,
}

/// Keeps a clip and drops the oldest past [`KEEP_CLIPS`]. Returns the image
/// files of dropped clips, to delete.
pub fn add_clip(conn: &Connection, clip: &ClipRow) -> Result<Vec<String>> {
    conn.execute(
        "INSERT OR REPLACE INTO sync_clip (id, at, peer, incoming, kind, text, image, bytes, late, from_app)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            clip.id,
            clip.at,
            clip.peer,
            clip.incoming,
            clip.kind,
            clip.text,
            clip.image,
            clip.bytes as i64,
            clip.late,
            clip.from_app
        ],
    )?;
    let dropped: Vec<Option<String>> = {
        let mut stmt = conn.prepare(
            "SELECT image FROM sync_clip WHERE id NOT IN
                (SELECT id FROM sync_clip ORDER BY at DESC, rowid DESC LIMIT ?1)",
        )?;
        let rows = stmt.query_map([KEEP_CLIPS as i64], |r| r.get(0))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    conn.execute(
        "DELETE FROM sync_clip WHERE id NOT IN
            (SELECT id FROM sync_clip ORDER BY at DESC, rowid DESC LIMIT ?1)",
        [KEEP_CLIPS as i64],
    )?;
    Ok(dropped.into_iter().flatten().collect())
}

pub fn clips(conn: &Connection) -> Result<Vec<ClipRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, at, peer, incoming, kind, text, image, bytes, late, from_app
         FROM sync_clip ORDER BY at DESC, rowid DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(ClipRow {
            id: r.get(0)?,
            at: r.get(1)?,
            peer: r.get(2)?,
            incoming: r.get(3)?,
            kind: r.get(4)?,
            text: r.get(5)?,
            image: r.get(6)?,
            bytes: r.get::<_, i64>(7)? as u64,
            late: r.get(8)?,
            from_app: r.get(9)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Forgets every clip. Returns their image files, to delete.
pub fn clear_clips(conn: &Connection) -> Result<Vec<String>> {
    let images: Vec<Option<String>> = {
        let mut stmt = conn.prepare("SELECT image FROM sync_clip")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    conn.execute("DELETE FROM sync_clip", [])?;
    Ok(images.into_iter().flatten().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::managers::history_store::MIGRATIONS;
    use rusqlite_migration::Migrations;
    use serde_json::json;

    fn db() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        Migrations::new(MIGRATIONS.to_vec())
            .to_latest(&mut conn)
            .unwrap();
        conn
    }

    fn item(key: &str, body: Value) -> Item {
        Item {
            key: key.into(),
            body,
            at: None,
        }
    }

    fn change(key: &str, body: Option<Value>, changed_at: i64, origin: &str) -> Change {
        Change {
            kind: "dictionary".into(),
            key: key.into(),
            body,
            changed_at,
            origin: origin.into(),
        }
    }

    const ALL: &[&str] = &["dictionary"];

    fn kinds() -> Vec<String> {
        ALL.iter().map(|k| k.to_string()).collect()
    }

    #[test]
    fn the_device_a_row_came_from_is_not_a_change() {
        let row = json!({"text": "Book the room", "at": 1});
        let named = json!({"text": "Book the room", "at": 1, "device": "Pixel 8"});
        assert_eq!(hash(Some(&row)), hash(Some(&named)));
    }

    #[test]
    fn canonical_json_ignores_key_order() {
        let a = json!({"b": 1, "a": {"y": [1, 2], "x": null}});
        let b: Value = serde_json::from_str(r#"{"a":{"x":null,"y":[1,2]},"b":1}"#).unwrap();
        assert_eq!(canonical(&a), canonical(&b));
        assert_eq!(hash(Some(&a)), hash(Some(&b)));
        assert_eq!(hash(None), "-");
        // Null and missing are the same body.
        assert_eq!(
            hash(Some(&json!({"text": "AIPS", "kind": null}))),
            hash(Some(&json!({"text": "AIPS"})))
        );
        assert_ne!(
            hash(Some(&json!({"text": "AIPS", "kind": "project"}))),
            hash(Some(&json!({"text": "AIPS"})))
        );
    }

    #[test]
    fn refresh_records_new_edited_and_deleted_items_once() {
        let mut conn = db();
        let words = vec![
            item("datev", json!({"text": "Datev"})),
            item("netbird", json!({"text": "Netbird"})),
        ];
        assert_eq!(
            refresh(&mut conn, "dictionary", &words, true, &[], "pc", 1000).unwrap(),
            2
        );
        assert_eq!(
            refresh(&mut conn, "dictionary", &words, true, &[], "pc", 2000).unwrap(),
            0
        );
        let edited = vec![item("datev", json!({"text": "DATEV"}))];
        assert_eq!(
            refresh(&mut conn, "dictionary", &edited, true, &[], "pc", 3000).unwrap(),
            2
        );
        let (items, upto, more) = changes_since(&conn, 2, &kinds(), "phone", 100).unwrap();
        assert_eq!(upto, 4);
        assert!(!more);
        assert_eq!(items.len(), 2);
        let deleted = items.iter().find(|c| c.key == "netbird").unwrap();
        assert_eq!(deleted.body, None);
        assert_eq!(deleted.changed_at, 3000);
        // A tombstone isn't recorded twice.
        assert_eq!(
            refresh(&mut conn, "dictionary", &edited, true, &[], "pc", 4000).unwrap(),
            0
        );
    }

    #[test]
    fn a_row_from_another_device_isnt_recorded_again() {
        let mut conn = db();
        let said = json!({"text": "Ship it", "device": "Pixel"});
        adopt(&conn, &change("pixel:7", Some(said.clone()), 1000, "phone")).unwrap();
        let rows = vec![item("pixel:7", said)];
        assert_eq!(
            refresh(&mut conn, "dictionary", &rows, false, &[], "pc", 2000).unwrap(),
            0
        );
        let mut renamed = rows.clone();
        renamed[0].body["device"] = json!("Pixel 9");
        assert_eq!(
            refresh(&mut conn, "dictionary", &renamed, false, &[], "pc", 3000).unwrap(),
            0
        );
    }

    #[test]
    fn a_later_edit_beats_the_version_it_replaced_even_with_a_slow_clock() {
        let mut conn = db();
        let theirs = change("datev", Some(json!({"text": "Datev"})), 50_000, "phone");
        adopt(&conn, &theirs).unwrap();
        // This PC's clock runs behind the phone's.
        refresh(
            &mut conn,
            "dictionary",
            &[item("datev", json!({"text": "DATEV"}))],
            true,
            &[],
            "pc",
            10_000,
        )
        .unwrap();
        let (items, ..) = changes_since(&conn, 1, &kinds(), "", 10).unwrap();
        assert_eq!(items[0].changed_at, 50_001);
    }

    #[test]
    fn without_infer_deletes_only_gone_keys_become_tombstones() {
        let mut conn = db();
        let all = vec![
            item("pc:1", json!({"text": "one"})),
            item("pc:2", json!({"text": "two"})),
        ];
        refresh(&mut conn, "history", &all, false, &[], "pc", 1).unwrap();
        // Entry 1 was trimmed by the history limit, entry 2 deleted.
        assert_eq!(
            refresh(&mut conn, "history", &[], false, &["pc:2".into()], "pc", 2).unwrap(),
            1
        );
        let (items, ..) = changes_since(&conn, 2, &["history".into()], "", 10).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].key, "pc:2");
        assert_eq!(items[0].body, None);
    }

    #[test]
    fn changes_skip_what_came_from_the_asker_and_kinds_it_did_not_ask_for() {
        let mut conn = db();
        adopt(&conn, &change("from-phone", Some(json!(1)), 5, "phone")).unwrap();
        refresh(
            &mut conn,
            "snippets",
            &[item("brb", json!(2))],
            true,
            &[],
            "pc",
            6,
        )
        .unwrap();
        refresh(
            &mut conn,
            "dictionary",
            &[item("from-phone", json!(1)), item("ours", json!(3))],
            true,
            &[],
            "pc",
            7,
        )
        .unwrap();
        let (items, upto, _) = changes_since(&conn, 0, &kinds(), "phone", 10).unwrap();
        assert_eq!(upto, 3);
        assert_eq!(
            items.iter().map(|c| c.key.as_str()).collect::<Vec<_>>(),
            ["ours"]
        );
        let (first, upto, more) =
            changes_since(&conn, 0, &["dictionary".into(), "snippets".into()], "", 2).unwrap();
        assert_eq!((first.len(), upto, more), (2, 2, true));
    }

    #[test]
    fn newest_wins_ties_go_to_the_larger_origin_and_merges_are_spotted() {
        let mut conn = db();
        refresh(
            &mut conn,
            "dictionary",
            &[item("datev", json!({"text": "Datev"}))],
            true,
            &[],
            "pc",
            1000,
        )
        .unwrap();
        let same = change("datev", Some(json!({"text": "Datev"})), 9000, "phone");
        assert_eq!(judge(&conn, "pc", 0, &same).unwrap(), Verdict::Same);
        let newer = change("datev", Some(json!({"text": "DATEV"})), 2000, "phone");
        // The phone hasn't confirmed our version yet, so both changed it.
        assert_eq!(
            judge(&conn, "pc", 0, &newer).unwrap(),
            Verdict::Theirs { merged: true }
        );
        assert_eq!(
            judge(&conn, "pc", 1, &newer).unwrap(),
            Verdict::Theirs { merged: false }
        );
        let older = change("datev", Some(json!({"text": "DATEV"})), 500, "phone");
        assert_eq!(
            judge(&conn, "pc", 0, &older).unwrap(),
            Verdict::Ours { merged: true }
        );
        let tie_low = change("datev", Some(json!({"text": "x"})), 1000, "a-phone");
        let tie_high = change("datev", Some(json!({"text": "x"})), 1000, "zz-phone");
        assert!(matches!(
            judge(&conn, "pc", 1, &tie_low).unwrap(),
            Verdict::Ours { .. }
        ));
        assert!(matches!(
            judge(&conn, "pc", 1, &tie_high).unwrap(),
            Verdict::Theirs { .. }
        ));
        let unknown = change("new", None, 1, "phone");
        assert_eq!(
            judge(&conn, "pc", 0, &unknown).unwrap(),
            Verdict::Theirs { merged: false }
        );
    }

    #[test]
    fn resequencing_a_kind_moves_it_after_everything_else() {
        let mut conn = db();
        refresh(
            &mut conn,
            "dictionary",
            &[item("a", json!(1)), item("b", json!(2))],
            true,
            &[],
            "pc",
            1,
        )
        .unwrap();
        refresh(
            &mut conn,
            "snippets",
            &[item("c", json!(3))],
            true,
            &[],
            "pc",
            1,
        )
        .unwrap();
        resequence(&mut conn, "dictionary").unwrap();
        let (items, upto, _) = changes_since(&conn, 3, &kinds(), "", 10).unwrap();
        assert_eq!(upto, 5);
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn local_rows_keep_their_keys_and_gone_rows_are_taken_once() {
        let conn = db();
        assert_eq!(key_of(&conn, "history", 7, "pc").unwrap(), "pc:7");
        map_local(&conn, "history", 8, "phone:3", "Pixel 8").unwrap();
        assert_eq!(key_of(&conn, "history", 8, "pc").unwrap(), "phone:3");
        assert_eq!(
            local_id(&conn, "history", "phone:3", "pc").unwrap(),
            Some(8)
        );
        assert_eq!(local_id(&conn, "history", "pc:7", "pc").unwrap(), Some(7));
        assert_eq!(local_id(&conn, "history", "phone:9", "pc").unwrap(), None);
        forget(&conn, "history", 8, 1).unwrap();
        assert_eq!(take_gone(&conn, "history", "pc").unwrap(), ["phone:3"]);
        assert!(take_gone(&conn, "history", "pc").unwrap().is_empty());
    }

    #[test]
    fn peers_merges_and_clips_are_kept() {
        let conn = db();
        let phone = Peer {
            device_id: "phone".into(),
            name: "Pixel 8".into(),
            platform: "android".into(),
            app_version: "0.9.8".into(),
            fingerprint: "fp".into(),
            paired_at: 1,
            last_seen: None,
            last_sync: None,
            sent_upto: 0,
            recv_upto: 0,
        };
        add_peer(&conn, &phone).unwrap();
        set_sent_upto(&conn, "phone", 9, 5).unwrap();
        set_sent_upto(&conn, "phone", 4, 6).unwrap();
        set_recv_upto(&conn, "phone", 3, 7).unwrap();
        rename_peer(&conn, "phone", "Work phone").unwrap();
        let got = peer(&conn, "phone").unwrap().unwrap();
        assert_eq!(
            (got.sent_upto, got.recv_upto, got.last_sync),
            (9, 3, Some(7))
        );
        assert_eq!(got.name, "Work phone");
        remove_peer(&conn, "phone").unwrap();
        assert!(peers(&conn).unwrap().is_empty());

        log_merge(
            &conn,
            1,
            "Pixel 8",
            "scratchpad",
            "pc:1",
            "Open questions",
            true,
        )
        .unwrap();
        assert_eq!(merges(&conn).unwrap()[0].label, "Open questions");

        let clip = |i: usize| ClipRow {
            id: format!("c{i}"),
            at: i as i64,
            peer: "Pixel 8".into(),
            incoming: true,
            kind: "image".into(),
            text: None,
            image: Some(format!("c{i}.png")),
            bytes: 10,
            late: false,
            from_app: None,
        };
        for i in 0..KEEP_CLIPS {
            assert!(add_clip(&conn, &clip(i)).unwrap().is_empty());
        }
        assert_eq!(add_clip(&conn, &clip(KEEP_CLIPS)).unwrap(), ["c0.png"]);
        assert_eq!(clips(&conn).unwrap()[0].id, format!("c{KEEP_CLIPS}"));
        assert_eq!(clear_clips(&conn).unwrap().len(), KEEP_CLIPS);
    }
}
