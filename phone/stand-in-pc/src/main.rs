//! stand-in-pc: a PC for a phone to pair with when no Windows machine is
//! around. It runs the hub Legato on Windows runs, over a database shaped
//! like the PC's, so the phone apps and their tests can pair and sync on
//! one machine (the iPhone simulator shares its Mac's network).
//!
//!   stand-in-pc [--dir <folder>] [--name <name>] [--host <address>]
//!               [--port <first port>] [--link-file <file>] [--for <seconds>]
//!
//! It prints the pairing link, then a line for each thing that happens: a
//! phone pairing, changes taken, and what the phone can change (words,
//! snippets, ticked action items, Scratchpad notes) whenever that differs.
//!
//! A new folder starts with a few words, dictations, a Scratchpad note and
//! a meeting with a summary and action items, all made up.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use legato_phone::managers::history_store::MIGRATIONS;
use legato_phone::sync::engine::{now_s, Accepted, Engine};
use legato_phone::sync::hub::{Config, Events, Hub};
use legato_phone::sync::identity::Identity;
use legato_phone::sync::ledger::Peer;
use legato_phone::sync::link::{self, Host, Sessions, Via};
use legato_phone::sync::protocol::{PairLink, PORT};
use rusqlite::{params, Connection};
use rusqlite_migration::Migrations;
use tokio::sync::watch;

/// Says what the hub hears, one line each.
struct Says;

impl Events for Says {
    fn paired(&self, peer: &Peer) {
        println!("paired with {} ({})", peer.name, peer.platform);
    }

    fn online(&self, _device_id: &str, via: Option<Via>) {
        match via {
            Some(_) => println!("the phone connected"),
            None => println!("the phone disconnected"),
        }
    }

    fn synced(&self, _device_id: &str, accepted: &Accepted) {
        if accepted.applied > 0 {
            println!(
                "took {} changes ({})",
                accepted.applied,
                accepted
                    .kinds
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }

    fn forgotten(&self, _device_id: &str) {
        println!("the phone unpaired");
    }
}

/// What a new stand-in PC starts with.
fn seed(conn: &Connection) -> Result<()> {
    let words: i64 = conn.query_row("SELECT COUNT(*) FROM dictionary_entry", [], |r| r.get(0))?;
    if words > 0 {
        return Ok(());
    }
    let now = now_s();
    for (word, heard) in [
        ("Okonkwo", Some("oh conk woe")),
        ("Solveig", Some("sol vague")),
        ("Tallinn", None),
    ] {
        conn.execute(
            "INSERT INTO dictionary_entry (text, added_at) VALUES (?1, ?2)",
            params![word, now],
        )?;
        if let Some(heard) = heard {
            conn.execute(
                "INSERT INTO dictionary_heard (entry_id, heard) VALUES (?1, ?2)",
                params![conn.last_insert_rowid(), heard],
            )?;
        }
    }
    conn.execute(
        "INSERT INTO snippet (trigger, text, added_at) VALUES ('my address', '14 Linden Road, Bristol', ?1)",
        [now],
    )?;
    for (ago, app, text) in [
        (
            25 * 60,
            "Mail",
            "Thanks for the draft. I have two small changes and then it can go out.",
        ),
        (
            3 * 3600,
            "Slack",
            "Running ten minutes late, start without me.",
        ),
        (
            27 * 3600,
            "Word",
            "The second chapter needs a shorter opening and one more example.",
        ),
    ] {
        conn.execute(
            "INSERT INTO transcription_history (file_name, timestamp, title, transcription_text,
                app_name, word_count, status)
             VALUES ('', ?1, '', ?2, ?3, ?4, 'pasted')",
            params![now - ago, text, app, text.split_whitespace().count() as i64],
        )?;
    }
    conn.execute(
        "INSERT INTO scratch_note (title, text, created_at, updated_at)
         VALUES ('Reading list', 'The Overstory\nA Field Guide to Getting Lost', ?1, ?1)",
        [now - 2 * 3600],
    )?;
    conn.execute(
        "INSERT INTO note (title, kind, status, started_at, duration_ms, summary)
         VALUES ('Garden club planning', 'meeting', 'ready', ?1, ?2, ?3)",
        params![
            now - 5 * 3600,
            24 * 60_000,
            "The spring plant sale moves to the second Saturday of April. Ines books the hall \
             and Tomas asks the nursery about seedlings. Posters go up two weeks before."
        ],
    )?;
    let note = conn.last_insert_rowid();
    for (position, (owner, text)) in [
        ("Ines", "Book the community hall for the sale"),
        ("Tomas", "Ask the nursery for a price on seedlings"),
        ("You", "Draft the poster"),
    ]
    .into_iter()
    .enumerate()
    {
        conn.execute(
            "INSERT INTO note_action (note_id, owner, text, done, position) VALUES (?1, ?2, ?3, 0, ?4)",
            params![note, owner, text, position as i64],
        )?;
    }
    for (at_ms, who, text) in [
        (
            0,
            "you",
            "Shall we settle the date for the plant sale first?",
        ),
        (
            6_000,
            "them",
            "The second Saturday of April works for the hall, I checked last week.",
        ),
        (
            14_000,
            "you",
            "Good. I can draft the poster if someone asks the nursery about seedlings.",
        ),
        (
            22_000,
            "them",
            "I will ask them tomorrow and send round the prices.",
        ),
    ] {
        conn.execute(
            "INSERT INTO note_line (note_id, at_ms, end_ms, who, text) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![note, at_ms, at_ms + 5_000, who, text],
        )?;
    }
    Ok(())
}

fn column(conn: &Connection, sql: &str) -> Result<String> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?.join(", "))
}

/// What a phone can change, on one line.
fn picture(conn: &Connection) -> Result<String> {
    Ok(format!(
        "words: {} | snippets: {} | done: {} | scratchpad: {} | dictations: {}",
        column(
            conn,
            "SELECT text FROM dictionary_entry ORDER BY text COLLATE NOCASE"
        )?,
        column(conn, "SELECT trigger FROM snippet ORDER BY trigger")?,
        column(
            conn,
            "SELECT text FROM note_action WHERE done = 1 ORDER BY id"
        )?,
        column(
            conn,
            "SELECT CASE WHEN title = '' THEN text ELSE title END FROM scratch_note ORDER BY id"
        )?,
        conn.query_row("SELECT COUNT(*) FROM transcription_history", [], |r| r
            .get::<_, i64>(0))?,
    ))
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut take = |flag: &str| -> Option<String> {
        let at = args.iter().position(|a| a == flag)?;
        let value = args.get(at + 1).cloned();
        args.drain(at..(at + 2).min(args.len()));
        value
    };
    let dir = PathBuf::from(take("--dir").unwrap_or_else(|| "pc".into()));
    let name = take("--name").unwrap_or_else(|| "Stand-in PC".into());
    let host = take("--host").unwrap_or_else(|| "127.0.0.1".into());
    let port = take("--port");
    let link_file = take("--link-file").map(PathBuf::from);
    let seconds = take("--for");
    if !args.is_empty() {
        eprintln!(
            "usage: stand-in-pc [--dir <folder>] [--name <name>] [--host <address>] \
             [--port <first port>] [--link-file <file>] [--for <seconds>]"
        );
        std::process::exit(2);
    }
    let port: u16 = match port {
        Some(port) => port.parse().context("--port takes a number")?,
        None => PORT,
    };
    let seconds: Option<u64> = match seconds {
        Some(seconds) => Some(seconds.parse().context("--for takes seconds")?),
        None => None,
    };

    std::fs::create_dir_all(&dir)?;
    let identity = Identity::load_or_create(&dir.join("identity.json"))?;
    let db = dir.join("history.db");
    let mut conn = Connection::open(&db)?;
    Migrations::new(MIGRATIONS.to_vec())
        .to_latest(&mut conn)
        .context("setting up the stand-in PC's database")?;
    seed(&conn)?;

    let kinds: Vec<String> = ["dictionary", "snippets", "history", "notes", "scratchpad"]
        .map(String::from)
        .to_vec();
    let hub = Arc::new(Hub::new(
        Engine {
            db: db.clone(),
            me: identity.device_id.clone(),
            extra: None,
        },
        Config {
            name: name.clone(),
            app_version: env!("CARGO_PKG_VERSION").into(),
            kinds: kinds.clone(),
            clipboard: false,
        },
        Box::new(Says),
    ));
    hub.engine.refresh(&kinds)?;

    let listener = link::bind(port).await?;
    let addr = format!("{host}:{}", listener.local_addr()?.port());
    hub.set_addrs(vec![addr.clone()]);
    println!("{name} listens on {addr}");

    let sessions = Sessions::default();
    let (_stop_tx, stop) = watch::channel(false);
    let as_host: Arc<dyn Host> = hub.clone();
    tokio::spawn(link::serve(
        listener,
        identity.clone(),
        as_host,
        sessions.clone(),
        stop,
    ));

    let started = Instant::now();
    let mut last = String::new();
    loop {
        // A pairing link works once and for ten minutes: while no phone is
        // paired, there is always a fresh one to use.
        if !hub.pairing_open() && hub.peers().map(|p| p.is_empty()).unwrap_or(false) {
            let uri = PairLink {
                device_id: identity.device_id.clone(),
                name: name.clone(),
                fingerprint: identity.fingerprint(),
                addrs: vec![addr.clone()],
                secret: hub.open_pairing(),
            }
            .to_uri();
            println!("{uri}");
            if let Some(path) = &link_file {
                std::fs::write(path, &uri)?;
            }
        }
        if let Ok(now) = picture(&conn) {
            if now != last {
                println!("now {now}");
                last = now;
            }
        }
        if seconds.is_some_and(|s| started.elapsed() >= Duration::from_secs(s)) {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}
