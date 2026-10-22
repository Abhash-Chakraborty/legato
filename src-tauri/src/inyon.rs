//! Inyon, the agent hub, with Legato as one of its companion apps: pairing
//! with a code, a WebSocket to the hub while paired, and the agents waiting
//! for an answer shown on the bar or the island, with their options. The
//! rules (frames, the queue, addresses) are in `inyon_rules.rs`; the
//! contract is Inyon's companion contract, version 1.

use crate::inyon_rules::{self as rules, Frame, PaneRef, VoiceAnswer, Waiting};
use crate::settings::{get_settings, write_settings};
use futures_util::{SinkExt, StreamExt};
use log::{debug, info, warn};
use serde::Serialize;
use specta::Type;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{mpsc, Notify};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

/// Where the hub's token lives in Credential Manager ("Legato/inyon-token").
const TOKEN: &str = "inyon-token";
const CONTRACT_HEADER: &str = "X-Inyon-Contract";
/// A ping this often keeps the socket alive through NATs and proxies.
const PING_EVERY: Duration = Duration::from_secs(30);

/// The binding a dictation for an agent runs under; it has no shortcut.
pub const DICTATE_BINDING: &str = "inyon_dictate";

static WAITING: Mutex<Option<Waiting>> = Mutex::new(None);
/// Where the dictation running for an agent goes.
static VOICE: Mutex<Option<VoiceTarget>> = Mutex::new(None);

#[derive(Debug, Clone)]
struct VoiceTarget {
    pane: PaneRef,
    agent: String,
    /// The question it answers; with no options the words are a message.
    key: Option<String>,
    options: Vec<rules::PromptOption>,
}
/// Frames for the hub, while connected.
static OUTBOX: Mutex<Option<mpsc::UnboundedSender<String>>> = Mutex::new(None);
static CONNECTED: AtomicBool = AtomicBool::new(false);
/// Legato was removed in Inyon's Paired devices (close code 4001 or a 401):
/// the token is dead, so no reconnecting until it's paired again.
static REMOVED: AtomicBool = AtomicBool::new(false);
/// Bumped when the pairing or the alerts switch changes: the connection
/// ends and starts again from the settings.
static GENERATION: AtomicU64 = AtomicU64::new(0);
static CHANGED: Notify = Notify::const_new();

fn with_waiting<R>(f: impl FnOnce(&mut Waiting) -> R) -> R {
    let mut guard = WAITING.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(Waiting::default))
}

fn token() -> Option<String> {
    match crate::secrets::read_secret(TOKEN) {
        Ok(token) => token.filter(|t| !t.is_empty()),
        Err(e) => {
            warn!("Couldn't read Inyon's token from Credential Manager: {e}");
            None
        }
    }
}

/// Settings › Connections › Inyon, for the page.
#[derive(Debug, Clone, Serialize, Type)]
pub struct InyonStatus {
    pub paired: bool,
    pub hub: Option<String>,
    pub hub_name: Option<String>,
    pub alerts: bool,
    pub connected: bool,
    /// Agents waiting for an answer now.
    pub waiting: u32,
    /// Removed in Inyon: pair again with a new code.
    pub removed: bool,
}

pub fn status(app: &AppHandle) -> InyonStatus {
    let settings = get_settings(app).inyon;
    let waiting = with_waiting(|w| w.current().map(|a| a.more + 1).unwrap_or(0));
    InyonStatus {
        paired: settings.hub.is_some(),
        hub: settings.hub,
        hub_name: settings.hub_name,
        alerts: settings.alerts,
        connected: CONNECTED.load(Ordering::SeqCst),
        waiting,
        removed: REMOVED.load(Ordering::SeqCst),
    }
}

fn status_changed(app: &AppHandle) {
    let _ = app.emit("inyon-status", status(app));
}

/// Starts the connection for as long as Legato runs; it sleeps while not
/// paired or with alerts off.
pub fn init(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move { run(app).await });
}

/// The pairing or the alerts switch changed: connect again from scratch.
fn restart() {
    GENERATION.fetch_add(1, Ordering::SeqCst);
    CHANGED.notify_waiters();
}

async fn run(app: AppHandle) {
    let mut failures = 0u32;
    loop {
        let generation = GENERATION.load(Ordering::SeqCst);
        let settings = get_settings(&app).inyon;
        let ready = (settings.alerts && !REMOVED.load(Ordering::SeqCst))
            .then_some(())
            .and(settings.hub.clone())
            .zip(token());
        let Some((hub, token)) = ready else {
            CHANGED.notified().await;
            continue;
        };
        let started = std::time::Instant::now();
        match connect(&app, &hub, &token, generation).await {
            // A connection that lasted a while starts the backoff over; one
            // the hub closed at once (a removed token) doesn't.
            Ok(()) if started.elapsed() > Duration::from_secs(30) => failures = 0,
            Ok(()) => failures += 1,
            Err(e) => {
                failures += 1;
                debug!("Inyon: {e}");
            }
        }
        CONNECTED.store(false, Ordering::SeqCst);
        *OUTBOX.lock().unwrap_or_else(|e| e.into_inner()) = None;
        with_waiting(|w| w.clear());
        show(&app);
        status_changed(&app);
        if GENERATION.load(Ordering::SeqCst) == generation {
            let wait = Duration::from_secs(rules::backoff_secs(failures));
            tokio::select! {
                _ = tokio::time::sleep(wait) => {}
                _ = CHANGED.notified() => {}
            }
        }
    }
}

/// One connection, until it drops or the settings change. Ok when it ran
/// and ended, Err when it couldn't start.
async fn connect(app: &AppHandle, hub: &str, token: &str, generation: u64) -> Result<(), String> {
    let mut request = rules::ws_url(hub)
        .into_client_request()
        .map_err(|e| e.to_string())?;
    let headers = request.headers_mut();
    headers.insert(
        "Authorization",
        format!("Bearer {token}").parse().map_err(|_| "bad token")?,
    );
    headers.insert(CONTRACT_HEADER, rules::CONTRACT.into());
    let (socket, _) = tokio::time::timeout(
        Duration::from_secs(15),
        tokio_tungstenite::connect_async(request),
    )
    .await
    .map_err(|_| "the hub didn't answer".to_string())?
    .map_err(|e| {
        if let tokio_tungstenite::tungstenite::Error::Http(response) = &e {
            if response.status() == 401 {
                removed(app);
            }
        }
        format!("couldn't reach the hub: {e}")
    })?;
    info!("Connected to Inyon at {hub}");
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    *OUTBOX.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);
    CONNECTED.store(true, Ordering::SeqCst);
    status_changed(app);
    send(r#"{"t":"sub.overview"}"#.to_string());

    let mut ping = tokio::time::interval(PING_EVERY);
    ping.tick().await;
    loop {
        if GENERATION.load(Ordering::SeqCst) != generation {
            let _ = sink.close().await;
            return Ok(());
        }
        tokio::select! {
            frame = stream.next() => match frame {
                Some(Ok(Message::Text(text))) => {
                    if let Some(frame) = rules::parse_frame(&text) {
                        handle(app, frame);
                    }
                }
                Some(Ok(Message::Close(close))) => {
                    let why = close.map(|c| format!("{} {}", c.code, c.reason)).unwrap_or_default();
                    info!("Inyon closed the connection {why}");
                    // 4001: removed in Inyon, so the token is no good any more.
                    if why.starts_with("4001 ") {
                        removed(app);
                    }
                    return Ok(());
                }
                Some(Ok(_)) => {}
                Some(Err(e)) => return Err(format!("connection lost: {e}")),
                None => return Ok(()),
            },
            Some(out) = rx.recv() => {
                sink.send(Message::Text(out)).await.map_err(|e| e.to_string())?;
            }
            _ = ping.tick() => {
                sink.send(Message::Text(r#"{"t":"ping"}"#.into())).await.map_err(|e| e.to_string())?;
            }
            _ = CHANGED.notified() => {}
        }
    }
}

fn removed(app: &AppHandle) {
    if !REMOVED.swap(true, Ordering::SeqCst) {
        warn!("Inyon removed Legato; pair again in Settings › Connections");
        status_changed(app);
    }
}

fn send(frame: String) {
    if let Some(tx) = OUTBOX.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        let _ = tx.send(frame);
    }
}

fn chat_frame(t: &str, pane: &PaneRef) -> String {
    serde_json::json!({ "t": t, "host": pane.host, "pane": pane.pane }).to_string()
}

fn handle(app: &AppHandle, frame: Frame) {
    match frame {
        Frame::Overview(panes) => {
            let (start, stop) = with_waiting(|w| w.overview(panes));
            for pane in &start {
                send(chat_frame("sub.chat", pane));
            }
            for pane in &stop {
                send(chat_frame("unsub.chat", pane));
            }
            show(app);
        }
        Frame::Screen { pane, prompt } => {
            with_waiting(|w| w.screen(pane, prompt));
            show(app);
        }
        Frame::Notify(n) if n.kind == "done" => {
            // A short word that an agent finished, unless something else is
            // on the bar.
            if with_waiting(|w| w.current().is_none()) {
                crate::overlay::show_agent_done(app, &n.agent, &n.title);
            }
        }
        Frame::Notify(_) | Frame::Info { .. } | Frame::Pong => {}
    }
}

/// Hands the agent waiting longest to the bar, or takes the card away.
fn show(app: &AppHandle) {
    let alert = with_waiting(|w| w.current());
    crate::overlay::set_agent_card(app, alert.as_ref());
    status_changed(app);
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        // A send waits until the hub knows whether the text landed.
        .timeout(Duration::from_secs(40))
        .build()
        .map_err(|e| e.to_string())
}

/// The hub's answer as a reason key under `inyon.reason`, for an error.
async fn refusal(response: reqwest::Response) -> String {
    let status = response.status().as_u16();
    let reason = response
        .json::<serde_json::Value>()
        .await
        .ok()
        .and_then(|v| v.get("reason").and_then(|r| r.as_str()).map(str::to_string));
    rules::reason_key(status, reason.as_deref()).to_string()
}

/// Pairs with a hub from the code Inyon shows. Errors are reason keys.
pub async fn pair(app: &AppHandle, hub: &str, code: &str) -> Result<InyonStatus, String> {
    let hub = rules::hub_url(hub).ok_or("badAddress")?;
    let code = rules::pairing_code(code).ok_or("badCode")?;
    let name = format!(
        "Legato on {}",
        std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Windows".into())
    );
    let client = client()?;
    let response = client
        .post(format!("{hub}/api/companions/claim"))
        .header(CONTRACT_HEADER, rules::CONTRACT)
        .json(&serde_json::json!({ "code": code, "name": name, "app": "Legato for Windows" }))
        .send()
        .await
        .map_err(|e| {
            debug!("Inyon pairing: {e}");
            "unreachable".to_string()
        })?;
    if !response.status().is_success() {
        let status = response.status().as_u16();
        return Err(match refusal(response).await.as_str() {
            "signedOut" => "badCode".to_string(),
            "failed" if status == 429 => "rateLimit".to_string(),
            "failed" if status == 404 => "notAHub".to_string(),
            other => other.to_string(),
        });
    }
    let claimed: rules::Claimed = response.json().await.map_err(|_| "notAHub")?;
    if claimed.contract_version != rules::CONTRACT {
        return Err("contract".into());
    }
    crate::secrets::write_secret(TOKEN, &claimed.token).map_err(|e| {
        warn!("Couldn't keep Inyon's token in Credential Manager: {e}");
        "vault".to_string()
    })?;
    // The hub's own name, for Settings; the address stands in without it.
    let hub_name = async {
        let me: serde_json::Value = client
            .get(format!("{hub}/api/companions/me"))
            .header(CONTRACT_HEADER, rules::CONTRACT)
            .bearer_auth(&claimed.token)
            .send()
            .await
            .ok()?
            .json()
            .await
            .ok()?;
        me.get("hub")?.get("name")?.as_str().map(str::to_string)
    }
    .await;
    let mut settings = get_settings(app);
    settings.inyon.hub = Some(hub.clone());
    settings.inyon.hub_id = Some(claimed.hub_id);
    settings.inyon.hub_name = hub_name;
    write_settings(app, settings);
    info!("Paired with Inyon at {hub}");
    REMOVED.store(false, Ordering::SeqCst);
    restart();
    Ok(status(app))
}

/// Forgets the hub: the token goes from Credential Manager, the connection
/// ends. Removing Legato in Inyon's Paired devices ends it on that side.
pub fn unpair(app: &AppHandle) {
    if let Err(e) = crate::secrets::delete_secret(TOKEN) {
        warn!("Couldn't remove Inyon's token: {e}");
    }
    let mut settings = get_settings(app);
    settings.inyon.hub = None;
    settings.inyon.hub_id = None;
    settings.inyon.hub_name = None;
    write_settings(app, settings);
    REMOVED.store(false, Ordering::SeqCst);
    restart();
    status_changed(app);
}

/// Agents on the bar, or not; the pairing stays.
pub fn set_alerts(app: &AppHandle, on: bool) {
    let mut settings = get_settings(app);
    settings.inyon.alerts = on;
    write_settings(app, settings);
    restart();
    status_changed(app);
}

/// Answers the question on the card with one of its options. Errors are
/// reason keys; "already answered" takes the card away all the same.
pub async fn answer(
    app: &AppHandle,
    host: &str,
    pane: &str,
    option: u32,
    key: Option<String>,
    text: Option<String>,
) -> Result<(), String> {
    let hub = get_settings(app).inyon.hub.ok_or("signedOut")?;
    let token = token().ok_or("signedOut")?;
    let mut body = serde_json::json!({ "option": option, "key": key });
    if let Some(text) = text {
        body["text"] = text.into();
    }
    let response = client()?
        .post(rules::pane_route(&hub, host, pane, "answer"))
        .header(CONTRACT_HEADER, rules::CONTRACT)
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(|_| "unreachable".to_string())?;
    let pane_ref = PaneRef {
        host: host.to_string(),
        pane: pane.to_string(),
    };
    let result = if response.status().is_success() {
        Ok(())
    } else {
        Err(refusal(response).await)
    };
    if matches!(&result, Err(reason) if reason == "signedOut") {
        removed(app);
    }
    // Answered, here or elsewhere, or its session or machine went away.
    let gone = match &result {
        Ok(()) => true,
        Err(reason) => matches!(reason.as_str(), "alreadyAnswered" | "paneGone" | "offline"),
    };
    if gone {
        with_waiting(|w| w.answered(&pane_ref, key.as_deref()));
        show(app);
    }
    result
}

/// Types a message into an agent's session. Ok with how far it got
/// ("delivered", "queued" or "sent"); errors are reason keys.
pub async fn send_text(app: &AppHandle, pane: &PaneRef, text: &str) -> Result<String, String> {
    let hub = get_settings(app).inyon.hub.ok_or("signedOut")?;
    let token = token().ok_or("signedOut")?;
    // A retry with the same id is never typed twice.
    let request_id = format!(
        "legato-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default()
    );
    let response = client()?
        .post(rules::pane_route(&hub, &pane.host, &pane.pane, "send"))
        .header(CONTRACT_HEADER, rules::CONTRACT)
        .bearer_auth(token)
        .json(&serde_json::json!({ "text": text, "requestId": request_id }))
        .send()
        .await
        .map_err(|_| "unreachable".to_string())?;
    if !response.status().is_success() {
        let reason = refusal(response).await;
        if reason == "signedOut" {
            removed(app);
        }
        return Err(reason);
    }
    let v: serde_json::Value = response.json().await.unwrap_or_default();
    Ok(v.get("state")
        .and_then(|s| s.as_str())
        .unwrap_or("sent")
        .to_string())
}

/// The mic on an agent's card: starts a dictation whose words answer its
/// question, or stops the one running.
pub fn dictate(app: &AppHandle, host: &str, pane: &str) {
    let Some(coordinator) =
        app.try_state::<crate::transcription_coordinator::TranscriptionCoordinator>()
    else {
        return;
    };
    let recording = app
        .try_state::<std::sync::Arc<crate::managers::audio::AudioRecordingManager>>()
        .is_some_and(|a| a.is_recording());
    {
        let mut voice = VOICE.lock().unwrap_or_else(|e| e.into_inner());
        // A target left from a dictation that never started is replaced.
        if voice.is_none() || !recording {
            let alert = with_waiting(|w| w.current()).filter(|a| a.host == host && a.pane == pane);
            *voice = Some(VoiceTarget {
                pane: PaneRef {
                    host: host.to_string(),
                    pane: pane.to_string(),
                },
                agent: alert
                    .as_ref()
                    .map(|a| a.agent.clone())
                    .unwrap_or_else(|| "agent".into()),
                key: alert.as_ref().and_then(|a| a.key.clone()),
                options: alert.map(|a| a.options).unwrap_or_default(),
            });
        }
    }
    coordinator.send_external_input(DICTATE_BINDING, "inyon");
}

/// A dictation for an agent was cancelled.
pub fn dictation_cancelled() {
    VOICE.lock().unwrap_or_else(|e| e.into_inner()).take();
}

/// The words of a dictation for an agent: matched to one of the question's
/// options and sent as the answer, or typed in as a message when it asked
/// nothing. The bar says what happened.
pub fn receive(app: &AppHandle, text: String) {
    let Some(target) = VOICE.lock().unwrap_or_else(|e| e.into_inner()).take() else {
        return;
    };
    let text = text.trim().to_string();
    if text.is_empty() {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let VoiceTarget {
            pane,
            agent,
            key,
            options,
        } = target;
        let result = if options.is_empty() {
            send_text(&app, &pane, &text).await.map(|_| ())
        } else {
            match rules::voice_answer(&text, &options) {
                Some(VoiceAnswer::Pick(n)) => {
                    answer(&app, &pane.host, &pane.pane, n, key, None).await
                }
                Some(VoiceAnswer::WithText(n, said)) => {
                    answer(&app, &pane.host, &pane.pane, n, key, Some(said)).await
                }
                None => Err("notAnOption".to_string()),
            }
        };
        let notice = match &result {
            Ok(()) => serde_json::json!({ "kind": "agent_sent", "agent": agent }),
            Err(reason) => serde_json::json!({ "kind": "agent_failed", "reason": reason }),
        };
        if let Err(reason) = &result {
            info!("Inyon: a spoken answer didn't go through ({reason})");
        }
        crate::overlay::show_overlay_notice(&app, &notice);
    });
}
