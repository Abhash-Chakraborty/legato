//! The PC's hub and the stand-in phone over real TLS on localhost: pairing,
//! syncing both ways, a merge, the clipboard and refusals.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use legato_phone::managers::history_store::MIGRATIONS;
use legato_phone::sync::engine::{Accepted, Engine};
use legato_phone::sync::hub::{Config, Events, Hub};
use legato_phone::sync::identity::{uuid, Identity};
use legato_phone::sync::ledger::{self, Peer};
use legato_phone::sync::link::{self, Command, Dialer, Host, Sessions, Via};
use legato_phone::sync::protocol::{AiShare, Clip, ClipKind, Refusal, SharedProvider};
use legato_phone::{PairedPc, Phone};
use rusqlite::Connection;
use rusqlite_migration::Migrations;
use tokio::sync::watch;

#[derive(Default)]
struct Heard(Mutex<Vec<String>>);

struct Recorder(Arc<Heard>);

impl Events for Recorder {
    fn paired(&self, peer: &Peer) {
        self.0
             .0
            .lock()
            .unwrap()
            .push(format!("paired {}", peer.name));
    }
    fn synced(&self, _device_id: &str, accepted: &Accepted) {
        if accepted.applied > 0 {
            self.0
                 .0
                .lock()
                .unwrap()
                .push(format!("synced {}", accepted.applied));
        }
    }
    fn clip(&self, peer: &Peer, clip: Clip) {
        self.0 .0.lock().unwrap().push(format!(
            "clip from {}: {}",
            peer.name,
            clip.text.unwrap_or_default()
        ));
    }
    fn online(&self, _device_id: &str, via: Option<Via>) {
        if via == Some(Via::Bluetooth) {
            self.0
                 .0
                .lock()
                .unwrap()
                .push("online over Bluetooth".into());
        }
    }
    fn transcribe(
        &self,
        _device_id: &str,
        samples: Vec<f32>,
        app: Option<String>,
    ) -> Option<link::TranscribeJob> {
        self.0
             .0
            .lock()
            .unwrap()
            .push(format!("transcribe {}", samples.len()));
        Some(Box::new(move || {
            Ok(format!(
                "heard {} samples for {}",
                samples.len(),
                app.as_deref().unwrap_or("other")
            ))
        }))
    }
    fn pairing_failed(&self, reason: Refusal) {
        self.0 .0.lock().unwrap().push(format!("failed {reason:?}"));
    }
    fn ai_share(&self, _device_id: &str) -> Option<AiShare> {
        Some(AiShare {
            providers: vec![SharedProvider {
                id: "openai".into(),
                label: "OpenAI".into(),
                base_url: "https://api.openai.com/v1".into(),
                api_format: "openai".into(),
                model: "gpt-5-nano".into(),
                key: Some("sk-test".into()),
            }],
            own_models: Vec::new(),
            cleanup: None,
        })
    }
    fn transform(
        &self,
        _device_id: &str,
        transform_id: String,
        text: String,
        app: Option<String>,
    ) -> Option<link::TransformJob> {
        Some(Box::new(move || match transform_id.as_str() {
            "shout" => Ok(Some(format!(
                "{} ({})",
                text.to_uppercase(),
                app.unwrap_or_default()
            ))),
            "same" => Ok(None),
            _ => Err("no such transform".into()),
        }))
    }
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("legato-e2e-{name}-{}", uuid()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn words(conn: &Connection) -> Vec<String> {
    let mut stmt = conn
        .prepare("SELECT text FROM dictionary_entry ORDER BY text")
        .unwrap();
    let rows = stmt.query_map([], |r| r.get(0)).unwrap();
    rows.collect::<rusqlite::Result<_>>().unwrap()
}

async fn until(what: &str, mut check: impl FnMut() -> bool) {
    let start = Instant::now();
    while !check() {
        assert!(
            start.elapsed() < Duration::from_secs(8),
            "timed out waiting for {what}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

struct Pc {
    hub: Arc<Hub>,
    identity: Identity,
    sessions: Sessions,
    addr: String,
    heard: Arc<Heard>,
    db: PathBuf,
}

async fn start_pc() -> (Pc, watch::Sender<bool>) {
    let dir = temp("pc");
    let identity = Identity::generate().unwrap();
    let db = dir.join("history.db");
    let mut conn = Connection::open(&db).unwrap();
    Migrations::new(MIGRATIONS.to_vec())
        .to_latest(&mut conn)
        .unwrap();
    conn.execute(
        "INSERT INTO dictionary_entry (text, added_at) VALUES ('Datev', 1)",
        [],
    )
    .unwrap();
    let heard = Arc::new(Heard::default());
    let hub = Arc::new(Hub::new(
        Engine {
            db: db.clone(),
            me: identity.device_id.clone(),
            extra: None,
        },
        Config {
            name: "Desk PC".into(),
            app_version: "0.9.8".into(),
            kinds: ["dictionary", "snippets", "history", "notes", "scratchpad"]
                .map(String::from)
                .to_vec(),
            clipboard: true,
        },
        Box::new(Recorder(heard.clone())),
    ));
    let listener = link::bind(0).await.unwrap();
    let addr = format!("127.0.0.1:{}", listener.local_addr().unwrap().port());
    let sessions = Sessions::default();
    let (stop_tx, stop) = watch::channel(false);
    let host: Arc<dyn Host> = hub.clone();
    tokio::spawn(link::serve(
        listener,
        identity.clone(),
        host,
        sessions.clone(),
        stop,
    ));
    (
        Pc {
            hub,
            identity,
            sessions,
            addr,
            heard,
            db,
        },
        stop_tx,
    )
}

fn phone(name: &str) -> Arc<Phone> {
    Arc::new(
        Phone::open(
            &temp("phone"),
            name,
            Box::new(|line| println!("phone: {line}")),
        )
        .unwrap(),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_phone_pairs_syncs_both_ways_merges_and_shares_the_clipboard() {
    let (pc, _stop_pc) = start_pc().await;
    let pc_conn = || Connection::open(&pc.db).unwrap();

    // Without a secret, an unknown phone is turned away.
    let stranger = phone("Stranger");
    let (_s, stop) = watch::channel(false);
    let refused = link::connect(
        std::slice::from_ref(&pc.addr),
        &pc.identity.fingerprint(),
        &stranger.identity,
        None,
        None,
        stranger.clone(),
        Sessions::default(),
        stop.clone(),
    )
    .await;
    assert!(refused.unwrap_err().to_string().contains("UnknownDevice"));

    // A wrong secret is counted; the right one pairs.
    let secret = pc.hub.open_pairing();
    let refused = link::connect(
        std::slice::from_ref(&pc.addr),
        &pc.identity.fingerprint(),
        &stranger.identity,
        Some("wrong".into()),
        None,
        stranger.clone(),
        Sessions::default(),
        stop.clone(),
    )
    .await;
    assert!(refused.unwrap_err().to_string().contains("BadSecret"));

    // A phone that dialled the wrong PC (another fingerprint) never gets as
    // far as hello.
    let impostor = Identity::generate().unwrap();
    let mismatch = link::connect(
        std::slice::from_ref(&pc.addr),
        &impostor.fingerprint(),
        &stranger.identity,
        Some(secret.clone()),
        None,
        stranger.clone(),
        Sessions::default(),
        stop.clone(),
    )
    .await;
    assert!(mismatch.is_err());
    assert!(pc.hub.pairing_open());

    let pixel = phone("Pixel 8");
    pixel.add_word("Netbird").unwrap();
    pixel.dictate("lunch at noon", "Chrome").unwrap();
    pixel.keep_cancelled("words cut off by ✕", "").unwrap();
    pixel.refresh().unwrap();
    pixel
        .save_pc(&PairedPc {
            device_id: pc.identity.device_id.clone(),
            name: "Desk PC".into(),
            fingerprint: pc.identity.fingerprint(),
            addrs: vec!["127.0.0.1:1".into(), pc.addr.clone()],
        })
        .unwrap();
    let phone_sessions = Sessions::default();
    let (stop_phone, stop) = watch::channel(false);
    let session = {
        let (pixel, sessions, addr, fp) = (
            pixel.clone(),
            phone_sessions.clone(),
            vec!["127.0.0.1:1".to_string(), pc.addr.clone()],
            pc.identity.fingerprint(),
        );
        tokio::spawn(async move {
            let host: Arc<dyn Host> = pixel.clone();
            link::connect(
                &addr,
                &fp,
                &pixel.identity,
                Some(secret),
                None,
                host,
                sessions,
                stop,
            )
            .await
        })
    };

    until("the first sync", || {
        words(&pc_conn()) == ["Datev", "Netbird"]
            && pixel.list("words").unwrap() == ["Datev", "Netbird"]
    })
    .await;
    assert!(!pc.hub.pairing_open());
    assert_eq!(pc.hub.peers().unwrap()[0].name, "Pixel 8");
    assert!(pc
        .heard
        .0
        .lock()
        .unwrap()
        .contains(&"paired Pixel 8".to_string()));
    // The phone's dictation is on the PC, marked with the phone's name.
    let (text, device): (String, String) = pc_conn()
        .query_row(
            "SELECT h.transcription_text, l.device FROM transcription_history h
             JOIN sync_local l ON l.kind = 'history' AND l.local_id = h.id
             WHERE h.status IS NOT 'cancelled'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    // One cancelled there stays marked cancelled.
    let cancelled: String = pc_conn()
        .query_row(
            "SELECT transcription_text FROM transcription_history WHERE status = 'cancelled'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(cancelled, "words cut off by ✕");
    assert_eq!(
        (text.as_str(), device.as_str()),
        ("lunch at noon", "Pixel 8")
    );

    // An edit on the PC reaches the phone after a nudge.
    pc_conn()
        .execute(
            "INSERT INTO snippet (trigger, text, added_at) VALUES ('brb', 'be right back', 1)",
            [],
        )
        .unwrap();
    pc.hub.engine.refresh(&pc.hub.config().kinds).unwrap();
    pc.sessions.broadcast(Command::Nudge);
    until("the snippet on the phone", || {
        pixel.list("snippets").unwrap() == ["brb = be right back"]
    })
    .await;

    // A delete on the phone reaches the PC.
    pixel.remove_word("Datev").unwrap();
    pixel.refresh().unwrap();
    phone_sessions.send(&pc.identity.device_id, Command::Nudge);
    until("the delete on the PC", || words(&pc_conn()) == ["Netbird"]).await;

    // Both edit the same note before either hears of the other: the newer
    // edit wins on both, and both log the merge.
    pc_conn()
        .execute("INSERT INTO scratch_note (title, text, created_at, updated_at) VALUES ('Ideas', 'first', 1, 1)", [])
        .unwrap();
    pc.hub.engine.refresh(&pc.hub.config().kinds).unwrap();
    pc.sessions.broadcast(Command::Nudge);
    until("the note on the phone", || {
        pixel.list("scratch").unwrap() == ["Ideas: first"]
    })
    .await;
    pc_conn()
        .execute("UPDATE scratch_note SET text = 'from the PC'", [])
        .unwrap();
    pc.hub.engine.refresh(&pc.hub.config().kinds).unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    pixel.scratch("Ideas", "from the phone").unwrap();
    pixel.refresh().unwrap();
    pc.sessions.broadcast(Command::Nudge);
    phone_sessions.send(&pc.identity.device_id, Command::Nudge);
    until("the merge", || {
        let pc_text: String = pc_conn()
            .query_row("SELECT text FROM scratch_note", [], |r| r.get(0))
            .unwrap();
        pc_text == "from the phone" && pixel.list("scratch").unwrap() == ["Ideas: from the phone"]
    })
    .await;
    let merges = ledger::merges(&pc_conn()).unwrap();
    assert_eq!(merges.len(), 1);
    assert!(merges[0].kept_theirs);
    let kept: String = pc_conn()
        .query_row(
            "SELECT text FROM scratch_version ORDER BY id DESC",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(kept, "from the PC");

    // The clipboard goes both ways.
    phone_sessions.send(
        &pc.identity.device_id,
        Command::Clip(Clip {
            id: uuid(),
            kind: ClipKind::Text,
            text: Some("tracking number 4711".into()),
            png: None,
            age_ms: 10,
            from_app: None,
        }),
    );
    until("the clip on the PC", || {
        pc.heard
            .0
            .lock()
            .unwrap()
            .iter()
            .any(|l| l == "clip from Pixel 8: tracking number 4711")
    })
    .await;
    let phone_id = pixel.identity.device_id.clone();
    assert!(pc.sessions.connected(&phone_id));
    pc.sessions.send(
        &phone_id,
        Command::Clip(Clip {
            id: uuid(),
            kind: ClipKind::Text,
            text: Some("from the PC".into()),
            png: None,
            age_ms: 0,
            from_app: Some("Chrome".into()),
        }),
    );
    until("the clip on the phone", || {
        !pixel.clips.lock().unwrap().is_empty()
    })
    .await;

    // Unpairing on the PC says goodbye; the phone forgets the PC.
    pc.sessions.send(&phone_id, Command::Bye("unpaired".into()));
    let ended = tokio::time::timeout(Duration::from_secs(5), session)
        .await
        .unwrap()
        .unwrap();
    assert!(ended.is_ok());
    assert!(pixel.paired_pc().is_none());
    let _ = stop_phone.send(true);
}

/// Stands in for the app's Bluetooth bridge: a port on 127.0.0.1 that
/// carries to `to` only the first connection that starts with `token`.
/// Another app that finds the port first is turned away.
fn bridge(to: u16, token: &str) -> u16 {
    use std::io::Read;
    use std::net::{Shutdown, TcpListener, TcpStream};
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = server.local_addr().unwrap().port();
    // A stranger gets there first and sends nothing useful.
    let mut stranger = TcpStream::connect(("127.0.0.1", port)).unwrap();
    std::io::Write::write_all(&mut stranger, b"GET / HTTP/1.1\r\n\r\n").unwrap();
    let token = token.as_bytes().to_vec();
    std::thread::spawn(move || {
        for local in server.incoming() {
            let Ok(mut local) = local else { return };
            let mut got = vec![0; token.len()];
            if local.read_exact(&mut got).is_err() || got != token {
                continue;
            }
            let Ok(remote) = TcpStream::connect(("127.0.0.1", to)) else {
                return;
            };
            let (mut up_from, mut up_to) =
                (local.try_clone().unwrap(), remote.try_clone().unwrap());
            std::thread::spawn(move || {
                let _ = std::io::copy(&mut up_from, &mut up_to);
                let _ = up_to.shutdown(Shutdown::Write);
            });
            let (mut local, mut remote) = (local, remote);
            let _ = std::io::copy(&mut remote, &mut local);
            let _ = local.shutdown(Shutdown::Both);
            return;
        }
    });
    drop(stranger);
    port
}

/// With no network address that answers, the phone dials Bluetooth through
/// the app. Here a localhost socket stands in for the radio, and the PC
/// takes what comes in on it the way it takes an RFCOMM connection.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_phone_with_no_network_in_common_pairs_over_bluetooth() {
    let (pc, _stop_pc) = start_pc().await;
    let radio = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let radio_port = radio.local_addr().unwrap().port();
    {
        let (identity, host, sessions) = (
            pc.identity.clone(),
            pc.hub.clone() as Arc<dyn Host>,
            pc.sessions.clone(),
        );
        tokio::spawn(async move {
            let (_keep, stop) = watch::channel(false);
            while let Ok((stream, _)) = radio.accept().await {
                let (identity, host, sessions, stop) = (
                    identity.clone(),
                    host.clone(),
                    sessions.clone(),
                    stop.clone(),
                );
                let Some(pending) = link::Pending::start() else {
                    continue;
                };
                tokio::spawn(async move {
                    let _ = link::serve_stream(
                        stream,
                        Via::Bluetooth,
                        &identity,
                        host,
                        sessions,
                        stop,
                        pending,
                    )
                    .await;
                });
            }
        });
    }
    let dialled = Arc::new(Mutex::new(Vec::<String>::new()));
    let dial: Dialer = {
        let dialled = dialled.clone();
        Arc::new(move |mac: &str, token: &str| {
            dialled.lock().unwrap().push(mac.to_string());
            Some(bridge(radio_port, token))
        })
    };

    let secret = pc.hub.open_pairing();
    // The PC moved to another network since: its welcome says so.
    pc.hub
        .set_addrs(vec!["10.0.0.9:47613".into(), "bt:AA:BB:CC:DD:EE:FF".into()]);
    let pixel = phone("Pixel 8");
    type Keys = Vec<(String, Option<String>)>;
    let keys: Arc<Mutex<Keys>> = Arc::default();
    {
        let keys = keys.clone();
        *pixel.on_ai_keys.lock().unwrap() = Some(Box::new(move |k| *keys.lock().unwrap() = k));
    }
    pixel.add_word("Bluetooth").unwrap();
    pixel
        .add_memo(
            "Voice memo",
            1_790_000_000,
            4_000,
            &[(0, 4_000, "Call the landlord about the boiler".into())],
        )
        .unwrap();
    pixel.refresh().unwrap();
    let addrs = vec![
        "127.0.0.1:1".to_string(),
        "bt:AA:BB:CC:DD:EE:FF".to_string(),
    ];
    pixel
        .save_pc(&PairedPc {
            device_id: pc.identity.device_id.clone(),
            name: "Desk PC".into(),
            fingerprint: pc.identity.fingerprint(),
            addrs: addrs.clone(),
        })
        .unwrap();
    let (_stop_phone, stop) = watch::channel(false);
    let phone_sessions = Sessions::default();
    {
        let (pixel, fp, sessions) = (
            pixel.clone(),
            pc.identity.fingerprint(),
            phone_sessions.clone(),
        );
        tokio::spawn(async move {
            let host: Arc<dyn Host> = pixel.clone();
            link::connect(
                &addrs,
                &fp,
                &pixel.identity,
                Some(secret),
                Some(dial),
                host,
                sessions,
                stop,
            )
            .await
        });
    }
    until("the sync over Bluetooth", || {
        words(&Connection::open(&pc.db).unwrap()) == ["Bluetooth", "Datev"]
    })
    .await;
    assert_eq!(*dialled.lock().unwrap(), ["AA:BB:CC:DD:EE:FF"]);
    // The memo recorded on the phone is a note on the PC, transcript and all.
    let memo: (String, String, String) = Connection::open(&pc.db)
        .unwrap()
        .query_row(
            "SELECT n.title, n.kind, l.text FROM note n JOIN note_line l ON l.note_id = n.id",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        memo,
        (
            "Voice memo".into(),
            "memo".into(),
            "Call the landlord about the boiler".into()
        )
    );
    assert!(pc
        .heard
        .0
        .lock()
        .unwrap()
        .contains(&"online over Bluetooth".to_string()));
    assert!(pixel.is_online());
    assert_eq!(pixel.via(), Some("bluetooth"));
    assert_eq!(
        pixel.paired_pc().unwrap().addrs,
        ["10.0.0.9:47613", "bt:AA:BB:CC:DD:EE:FF"]
    );

    // The phone has the PC transcribe a recording over the same link.
    assert!(pixel.pc_transcribes());
    let text = {
        let (pixel, sessions) = (pixel.clone(), phone_sessions.clone());
        tokio::task::spawn_blocking(move || {
            pixel.transcribe_on_pc(
                &sessions,
                &vec![0.1; 16_000],
                Duration::from_secs(5),
                Some("email"),
            )
        })
        .await
        .unwrap()
        .unwrap()
    };
    assert_eq!(text, "heard 16000 samples for email");

    // A live link answers its check at once.
    {
        let (pixel, sessions) = (pixel.clone(), phone_sessions.clone());
        assert!(tokio::task::spawn_blocking(move || {
            pixel.link_alive(&sessions, Duration::from_secs(2), Duration::ZERO)
        })
        .await
        .unwrap());
    }

    // Starting to listen asks the PC to load its model, with an empty recording.
    assert!(pixel.warm_pc(&phone_sessions));
    until("the PC is asked to load its model", || {
        pc.heard
            .0
            .lock()
            .unwrap()
            .contains(&"transcribe 0".to_string())
    })
    .await;

    // The PC shared its AI provider on connecting: the key went to the
    // app's hook, and only the rest was kept on disk.
    until("the AI provider arrives", || {
        keys.lock()
            .unwrap()
            .iter()
            .any(|(id, key)| id == "openai" && key.as_deref() == Some("sk-test"))
    })
    .await;
    let share = pixel.ai_share();
    assert_eq!(share.providers[0].model, "gpt-5-nano");
    assert_eq!(share.providers[0].key, None);
    assert!(pixel.pull_ai(&phone_sessions));

    // The PC runs its transforms on the phone's text.
    assert!(pixel.pc_transforms());
    let run = |id: &'static str| {
        let (pixel, sessions) = (pixel.clone(), phone_sessions.clone());
        tokio::task::spawn_blocking(move || {
            pixel.transform_on_pc(
                &sessions,
                id,
                "see you soon",
                Duration::from_secs(5),
                Some("email"),
            )
        })
    };
    assert_eq!(
        run("shout").await.unwrap().unwrap().as_deref(),
        Some("SEE YOU SOON (email)")
    );
    assert_eq!(run("same").await.unwrap().unwrap(), None);
    assert!(run("missing")
        .await
        .unwrap()
        .unwrap_err()
        .to_string()
        .contains("no such transform"));

    // Unpairing forgets the provider and tells the app to drop the keys.
    pixel.forget_pc().unwrap();
    assert!(pixel.ai_share().providers.is_empty());
    assert!(keys.lock().unwrap().is_empty());
}

/// A PC that's off, asleep or behind a firewall answers with "connection
/// refused" or nothing at all. The phone keeps trying and stays paired;
/// only the PC's own refusal ends a pairing.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_phone_stays_paired_while_the_pc_cannot_be_reached() {
    let pixel = phone("Pixel 8");
    pixel
        .save_pc(&PairedPc {
            device_id: "pc-that-is-off".into(),
            name: "Desk PC".into(),
            fingerprint: "00".repeat(32),
            addrs: vec!["127.0.0.1:1".into()],
        })
        .unwrap();
    let (stop_tx, stop) = watch::channel(false);
    let looping = {
        let pixel = pixel.clone();
        tokio::spawn(async move { pixel.link_loop(Sessions::default(), None, stop).await })
    };
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert!(
        pixel.paired_pc().is_some(),
        "a closed port must not unpair the phone"
    );
    stop_tx.send(true).unwrap();
    looping.abort();
}

/// Pairing while the PC can't be reached yet (Bluetooth still connecting, a
/// firewall) finishes once it can: the secret waits until the PC hears it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_pairing_finishes_once_the_pc_can_be_reached() {
    let (pc, _stop_pc) = start_pc().await;
    let port: u16 = pc.addr.rsplit(':').next().unwrap().parse().unwrap();
    let tries = Arc::new(Mutex::new(0));
    let dial: Dialer = {
        let tries = tries.clone();
        Arc::new(move |_mac: &str, token: &str| {
            let mut n = tries.lock().unwrap();
            *n += 1;
            (*n > 1).then(|| bridge(port, token))
        })
    };
    let secret = pc.hub.open_pairing();
    let pixel = phone("Pixel 8");
    *pixel.bluetooth.lock().unwrap() = Some(dial);
    pixel
        .save_pc(&PairedPc {
            device_id: pc.identity.device_id.clone(),
            name: "Desk PC".into(),
            fingerprint: pc.identity.fingerprint(),
            addrs: vec!["127.0.0.1:1".into(), "bt:AA:BB:CC:DD:EE:FF".into()],
        })
        .unwrap();
    let (stop_tx, stop) = watch::channel(false);
    let looping = {
        let pixel = pixel.clone();
        tokio::spawn(async move {
            pixel
                .link_loop(Sessions::default(), Some(secret), stop)
                .await
        })
    };
    until("the second try to pair", || {
        pc.heard
            .0
            .lock()
            .unwrap()
            .contains(&"paired Pixel 8".to_string())
    })
    .await;
    assert!(*tries.lock().unwrap() >= 2);
    assert!(pixel.paired_pc().is_some());
    stop_tx.send(true).unwrap();
    looping.abort();
}

/// A phone that had to connect over Bluetooth (the network wasn't up yet)
/// moves to the network once the PC answers there, since Bluetooth is slow
/// for recordings.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_phone_on_bluetooth_moves_to_the_network_when_it_answers() {
    let (pc, _stop_pc) = start_pc().await;
    let port: u16 = pc.addr.rsplit(':').next().unwrap().parse().unwrap();
    let dial: Dialer = Arc::new(move |_mac: &str, token: &str| Some(bridge(port, token)));
    let secret = pc.hub.open_pairing();
    // Where the PC says it is now; the phone only knew a stale address.
    pc.hub
        .set_addrs(vec![pc.addr.clone(), "bt:AA:BB:CC:DD:EE:FF".into()]);
    let pixel = phone("Pixel 8");
    pixel
        .network_check_ms
        .store(300, std::sync::atomic::Ordering::Relaxed);
    *pixel.bluetooth.lock().unwrap() = Some(dial);
    pixel
        .save_pc(&PairedPc {
            device_id: pc.identity.device_id.clone(),
            name: "Desk PC".into(),
            fingerprint: pc.identity.fingerprint(),
            addrs: vec!["127.0.0.1:1".into(), "bt:AA:BB:CC:DD:EE:FF".into()],
        })
        .unwrap();
    let (stop_tx, stop) = watch::channel(false);
    let looping = {
        let pixel = pixel.clone();
        tokio::spawn(async move {
            pixel
                .link_loop(Sessions::default(), Some(secret), stop)
                .await
        })
    };
    until("the link over Bluetooth", || {
        pixel.via() == Some("bluetooth")
    })
    .await;
    until("the move to the network", || {
        pixel.is_online() && pixel.via().is_some_and(|v| v != "bluetooth")
    })
    .await;
    assert!(pixel.paired_pc().is_some());
    stop_tx.send(true).unwrap();
    looping.abort();
}

#[test]
fn only_the_newest_image_stays_in_memory() {
    let pixel = phone("Pixel 9");
    let clip = |id: &str, png: Option<&str>, text: Option<&str>| Clip {
        id: id.into(),
        kind: if png.is_some() {
            ClipKind::Image
        } else {
            ClipKind::Text
        },
        text: text.map(Into::into),
        png: png.map(Into::into),
        age_ms: 0,
        from_app: None,
    };
    pixel.clip("pc", clip("a", Some("first screenshot"), None));
    pixel.clip("pc", clip("b", None, Some("some text")));
    pixel.clip("pc", clip("c", Some("second screenshot"), None));
    let clips = pixel.clips.lock().unwrap();
    let images: Vec<_> = clips.iter().filter_map(|c| c.png.as_deref()).collect();
    assert_eq!(images, ["second screenshot"]);
    assert_eq!(clips.len(), 3);
}
