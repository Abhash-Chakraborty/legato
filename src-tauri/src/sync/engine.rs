//! The sync work a device does with its database, shared by the PC and the
//! stand-in phone: recording local changes, answering pulls and taking in
//! the other device's changes.

use std::collections::BTreeSet;
use std::path::PathBuf;

use anyhow::Result;
use rusqlite::Connection;
use serde_json::Value;

use super::kinds::{self, DB_KINDS};
use super::ledger::{self, Item, Peer, Verdict};
use super::protocol::Change;

/// Kinds kept outside the database (on the PC, Transforms and Style live in
/// the settings).
pub trait Extra: Send + Sync {
    fn handles(&self, kind: &str) -> bool;
    fn export(&self, kind: &str) -> Result<Vec<Item>>;
    fn apply(&self, kind: &str, key: &str, body: Option<&Value>) -> Result<()>;
}

/// What taking in a batch of changes did.
#[derive(Debug, Default)]
pub struct Accepted {
    pub applied: usize,
    pub merges: usize,
    /// Kinds that changed here, so the app can reload them.
    pub kinds: BTreeSet<String>,
    /// Recordings of rows the other device deleted, with their kind.
    pub removed_audio: Vec<(String, String)>,
}

pub struct Engine {
    pub db: PathBuf,
    /// This device's ID.
    pub me: String,
    pub extra: Option<Box<dyn Extra>>,
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

pub fn now_s() -> i64 {
    now_ms() / 1000
}

impl Engine {
    pub fn conn(&self) -> Result<Connection> {
        let conn = Connection::open(&self.db)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        Ok(conn)
    }

    fn extra_for(&self, kind: &str) -> Option<&dyn Extra> {
        self.extra.as_deref().filter(|e| e.handles(kind))
    }

    /// Records local changes in `kinds`. Returns how many items changed.
    pub fn refresh(&self, kinds: &[String]) -> Result<usize> {
        let mut conn = self.conn()?;
        let now = now_ms();
        let mut changed = 0;
        for kind in kinds {
            let (items, gone) = if DB_KINDS.contains(&kind.as_str()) {
                let gone = if kinds::infer_deletes(kind) {
                    Vec::new()
                } else {
                    ledger::take_gone(&conn, kind, &self.me)?
                };
                (kinds::export(&conn, kind, &self.me)?, gone)
            } else if let Some(extra) = self.extra_for(kind) {
                (extra.export(kind)?, Vec::new())
            } else {
                continue;
            };
            changed += ledger::refresh(
                &mut conn,
                kind,
                &items,
                kinds::infer_deletes(kind),
                &gone,
                &self.me,
                now,
            )?;
        }
        Ok(changed)
    }

    /// Answers a pull from `peer`: only kinds both sides sync.
    pub fn changes(
        &self,
        peer: &str,
        after: u64,
        asked: &[String],
        enabled: &[String],
        limit: usize,
    ) -> Result<(Vec<Change>, u64, bool)> {
        let conn = self.conn()?;
        ledger::set_sent_upto(&conn, peer, after, now_s())?;
        let kinds: Vec<String> = asked
            .iter()
            .filter(|k| enabled.contains(k))
            .cloned()
            .collect();
        ledger::changes_since(&conn, after, &kinds, peer, limit)
    }

    /// Takes in `peer`'s changes for the kinds this device syncs, newest
    /// version winning, then records `upto` as held.
    pub fn accept(
        &self,
        peer: &Peer,
        items: Vec<Change>,
        upto: u64,
        enabled: &[String],
    ) -> Result<Accepted> {
        let mut conn = self.conn()?;
        let mut done = Accepted::default();
        for mut change in items {
            if !enabled.contains(&change.kind) {
                continue;
            }
            // A dictation or note keeps the name of the device it came
            // from when it's passed on to a third one.
            if matches!(change.kind.as_str(), "history" | "notes") {
                if let Some(Value::Object(body)) = change.body.as_mut() {
                    body.entry("device")
                        .or_insert_with(|| Value::String(peer.name.clone()));
                }
            }
            let verdict = ledger::judge(&conn, &self.me, peer.sent_upto, &change)?;
            let merged = match verdict {
                Verdict::Same => continue,
                Verdict::Ours { merged } => merged.then_some(false),
                Verdict::Theirs { merged } => {
                    let applied = if DB_KINDS.contains(&change.kind.as_str()) {
                        kinds::apply(
                            &mut conn,
                            &change.kind,
                            &change.key,
                            change.body.as_ref(),
                            &peer.name,
                            &self.me,
                            now_s(),
                        )
                        .map(|a| {
                            if let Some(file) = a.removed_audio {
                                done.removed_audio.push((change.kind.clone(), file));
                            }
                        })
                    } else if let Some(extra) = self.extra_for(&change.kind) {
                        extra.apply(&change.kind, &change.key, change.body.as_ref())
                    } else {
                        continue;
                    };
                    if let Err(e) = applied {
                        log::warn!("Couldn't apply {} {}: {e:#}", change.kind, change.key);
                        continue;
                    }
                    ledger::adopt(&conn, &change)?;
                    done.applied += 1;
                    done.kinds.insert(change.kind.clone());
                    merged.then_some(true)
                }
            };
            if let Some(kept_theirs) = merged {
                ledger::log_merge(
                    &conn,
                    now_s(),
                    &peer.name,
                    &change.kind,
                    &change.key,
                    &kinds::label(&change.kind, change.body.as_ref()),
                    kept_theirs,
                )?;
                done.merges += 1;
            }
        }
        ledger::set_recv_upto(&conn, &peer.device_id, upto, now_s())?;
        Ok(done)
    }

    /// Starts a kind that was switched back on over: its items get new
    /// sequence numbers and every device sends all of its own again.
    pub fn restart_kind(&self, kind: &str) -> Result<()> {
        self.refresh(&[kind.to_string()])?;
        let mut conn = self.conn()?;
        ledger::resequence(&mut conn, kind)?;
        ledger::restart_peers(&conn)?;
        Ok(())
    }
}
