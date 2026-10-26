//! A Google account for Google Calendar: signing in through the browser,
//! reading the account's events and adding new ones with a Meet link. The
//! sign-in is Google's desktop flow: the browser opens Google's page, the
//! answer comes back to a port on this PC, and a PKCE challenge makes the
//! code worthless to anyone else. Only the refresh token is kept, in
//! Credential Manager; meetings read are cached in `calendar.json` (see
//! `calendar.rs`), so everything but adding a meeting works offline.

use crate::calendar_rules as rules;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use log::{info, warn};
use once_cell::sync::Lazy;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Notify;

/// How long the browser has to come back from Google's page.
const SIGN_IN_FOR: Duration = Duration::from_secs(5 * 60);
const TIMEOUT: Duration = Duration::from_secs(20);
/// Pages of events read at most, 250 a page.
const MAX_PAGES: usize = 4;

/// Legato's OAuth client, set when the app is built
/// (`LEGATO_GOOGLE_CLIENT_ID`, `LEGATO_GOOGLE_CLIENT_SECRET`). An installed
/// app can't keep a secret, and Google says as much for desktop clients:
/// the "secret" ships in the app, and PKCE is what protects a sign-in.
fn client() -> Option<(&'static str, &'static str)> {
    let id = option_env!("LEGATO_GOOGLE_CLIENT_ID")?.trim();
    if id.is_empty() {
        return None;
    }
    Some((id, option_env!("LEGATO_GOOGLE_CLIENT_SECRET").unwrap_or("")))
}

/// Whether this build can sign in to Google.
pub fn available() -> bool {
    client().is_some()
}

/// Why Google didn't answer.
#[derive(Debug)]
pub enum GoogleError {
    /// No network, or Google didn't answer in time.
    Offline,
    /// The account must be signed in again (the access was removed or ran
    /// out).
    SignIn,
    Other(String),
}

impl std::fmt::Display for GoogleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GoogleError::Offline => write!(f, "Offline; the meetings are from the last check"),
            GoogleError::SignIn => write!(f, "Sign in to Google again"),
            GoogleError::Other(e) => write!(f, "{e}"),
        }
    }
}

fn http() -> Result<reqwest::Client, GoogleError> {
    reqwest::Client::builder()
        .timeout(TIMEOUT)
        .build()
        .map_err(|e| GoogleError::Other(e.to_string()))
}

/// A request that failed to go out: offline when it never reached Google.
/// The URL is left out, though none carries a token.
fn sent(e: reqwest::Error) -> GoogleError {
    if e.is_connect() || e.is_timeout() || e.is_request() {
        GoogleError::Offline
    } else {
        GoogleError::Other(e.without_url().to_string())
    }
}

/// Google's answer as JSON; its error code when it said no.
async fn answer(response: reqwest::Response) -> Result<Value, GoogleError> {
    let status = response.status();
    let body: Value = response.json().await.unwrap_or(Value::Null);
    if status.is_success() {
        return Ok(body);
    }
    let code = body["error"]
        .as_str()
        .or(body["error"]["status"].as_str())
        .unwrap_or("")
        .to_string();
    if status == reqwest::StatusCode::UNAUTHORIZED || code == "invalid_grant" {
        return Err(GoogleError::SignIn);
    }
    let message = body["error_description"]
        .as_str()
        .or(body["error"]["message"].as_str())
        .map(str::to_string)
        .unwrap_or_else(|| format!("Google answered {status}"));
    Err(GoogleError::Other(message))
}

fn random(bytes: usize) -> Result<String, String> {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).map_err(|e| e.to_string())?;
    Ok(URL_SAFE_NO_PAD.encode(buf))
}

/// The account's address, from the sign-in's ID token. It came straight
/// from Google over TLS, so it's read without checking its signature.
fn email_from(id_token: &str) -> Option<String> {
    let payload = id_token.split('.').nth(1)?;
    let json: Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload).ok()?).ok()?;
    json["email"].as_str().map(str::to_string)
}

/// Access tokens by refresh token, while they last.
static TOKENS: Lazy<Mutex<HashMap<String, (String, Instant)>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn keep_token(refresh: &str, body: &Value) {
    if let Some(access) = body["access_token"].as_str() {
        let secs = body["expires_in"]
            .as_u64()
            .unwrap_or(3600)
            .saturating_sub(60);
        TOKENS.lock().unwrap_or_else(|e| e.into_inner()).insert(
            refresh.to_string(),
            (
                access.to_string(),
                Instant::now() + Duration::from_secs(secs),
            ),
        );
    }
}

/// A signed-in account.
pub struct Account {
    pub email: String,
    pub refresh: String,
}

static SIGNING_IN: AtomicBool = AtomicBool::new(false);
static CANCEL: Lazy<Notify> = Lazy::new(Notify::new);

/// Stops a sign-in waiting for the browser.
pub fn cancel_sign_in() {
    CANCEL.notify_waiters();
}

const DONE_PAGE: &str = "<!doctype html><meta charset=utf-8><title>Legato</title>\
<body style=\"font:16px system-ui;margin:15vh auto;max-width:28em;padding:0 16px\">\
<h1 style=\"font-size:20px\">Legato is connected to Google Calendar</h1>\
<p>You can close this tab and go back to Legato.</p>";
const FAILED_PAGE: &str = "<!doctype html><meta charset=utf-8><title>Legato</title>\
<body style=\"font:16px system-ui;margin:15vh auto;max-width:28em;padding:0 16px\">\
<h1 style=\"font-size:20px\">Legato didn't connect</h1>\
<p>Close this tab and try again from Legato's settings.</p>";

/// Waits for the browser to come back from Google's page with a code.
async fn wait_for_code(listener: tokio::net::TcpListener, state: &str) -> Result<String, String> {
    loop {
        let (mut stream, _) = listener.accept().await.map_err(|e| e.to_string())?;
        let mut buf = vec![0u8; 8192];
        let n = tokio::time::timeout(Duration::from_secs(10), stream.read(&mut buf))
            .await
            .map_err(|_| "The browser didn't answer".to_string())?
            .map_err(|e| e.to_string())?;
        let request = String::from_utf8_lossy(&buf[..n]);
        let line = request.lines().next().unwrap_or("");
        // The browser also asks for an icon; that's not the answer.
        if !line.starts_with("GET /?") {
            let _ = stream
                .write_all(
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await;
            continue;
        }
        let result = rules::google_redirect(line, state);
        let page = if result.is_ok() {
            DONE_PAGE
        } else {
            FAILED_PAGE
        };
        let reply = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}",
            page.len()
        );
        let _ = stream.write_all(reply.as_bytes()).await;
        let _ = stream.shutdown().await;
        return result.map_err(|e| match e.as_str() {
            "access_denied" => "You didn't allow Legato in Google's page".to_string(),
            _ => format!("Google's sign-in didn't finish ({e})"),
        });
    }
}

/// Signs in to a Google account in the browser. Blocks until the browser
/// comes back, Cancel, or five minutes.
pub async fn sign_in(app: &AppHandle) -> Result<Account, String> {
    let (client_id, client_secret) =
        client().ok_or("This build of Legato can't sign in to Google")?;
    if SIGNING_IN.swap(true, Ordering::SeqCst) {
        return Err("A Google sign-in is already open in your browser".into());
    }
    struct Done;
    impl Drop for Done {
        fn drop(&mut self) {
            SIGNING_IN.store(false, Ordering::SeqCst);
        }
    }
    let _done = Done;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("Couldn't wait for Google's answer: {e}"))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let redirect = format!("http://127.0.0.1:{port}");
    let verifier = random(48)?;
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state = random(16)?;
    let url = rules::google_auth_url(client_id, &redirect, &challenge, &state);
    {
        use tauri_plugin_opener::OpenerExt;
        app.opener()
            .open_url(url, None::<String>)
            .map_err(|e| format!("Couldn't open the browser: {e}"))?;
    }
    let code = tokio::select! {
        code = tokio::time::timeout(SIGN_IN_FOR, wait_for_code(listener, &state)) => {
            code.map_err(|_| "Google's sign-in took too long; try again".to_string())??
        }
        _ = CANCEL.notified() => return Err("cancelled".into()),
    };
    crate::show_main_window(app);

    let response = http()
        .map_err(|e| e.to_string())?
        .post(rules::GOOGLE_TOKEN)
        .form(&[
            ("code", code.as_str()),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("redirect_uri", redirect.as_str()),
            ("grant_type", "authorization_code"),
            ("code_verifier", verifier.as_str()),
        ])
        .send()
        .await
        .map_err(|e| sent(e).to_string())?;
    let body = answer(response).await.map_err(|e| e.to_string())?;
    let refresh = body["refresh_token"]
        .as_str()
        .ok_or("Google didn't let Legato stay signed in; try again")?
        .to_string();
    let email = body["id_token"]
        .as_str()
        .and_then(email_from)
        .unwrap_or_else(|| "Google Calendar".to_string());
    keep_token(&refresh, &body);
    info!("Signed in to Google Calendar");
    Ok(Account { email, refresh })
}

/// An access token for the account, from the cache or a fresh one.
pub async fn access_token(refresh: &str) -> Result<String, GoogleError> {
    if let Some((token, until)) = TOKENS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(refresh)
        .cloned()
    {
        if Instant::now() < until {
            return Ok(token);
        }
    }
    let (client_id, client_secret) =
        client().ok_or_else(|| GoogleError::Other("This build can't use Google".into()))?;
    let response = http()?
        .post(rules::GOOGLE_TOKEN)
        .form(&[
            ("refresh_token", refresh),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await
        .map_err(sent)?;
    let body = answer(response).await?;
    keep_token(refresh, &body);
    body["access_token"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| GoogleError::Other("Google sent no access token".into()))
}

/// The primary calendar's events between `from` and `to` (Unix seconds),
/// repeats expanded, as one events list.
pub async fn events(token: &str, from: i64, to: i64) -> Result<Value, GoogleError> {
    let at = |t: i64| {
        chrono::DateTime::from_timestamp(t, 0)
            .map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
            .unwrap_or_default()
    };
    let client = http()?;
    let mut items: Vec<Value> = Vec::new();
    let mut page: Option<String> = None;
    for _ in 0..MAX_PAGES {
        let mut query = vec![
            ("timeMin", at(from)),
            ("timeMax", at(to)),
            ("singleEvents", "true".to_string()),
            ("orderBy", "startTime".to_string()),
            ("maxResults", "250".to_string()),
        ];
        if let Some(p) = &page {
            query.push(("pageToken", p.clone()));
        }
        let response = client
            .get(rules::GOOGLE_EVENTS)
            .bearer_auth(token)
            .query(&query)
            .send()
            .await
            .map_err(sent)?;
        let body = answer(response).await?;
        items.extend(body["items"].as_array().cloned().unwrap_or_default());
        page = body["nextPageToken"].as_str().map(str::to_string);
        if page.is_none() {
            break;
        }
    }
    Ok(serde_json::json!({ "items": items }))
}

/// Adds an event to the primary calendar and invites its people; returns
/// the event as Google keeps it (with its Meet link).
pub async fn create(token: &str, event: &Value) -> Result<Value, GoogleError> {
    let response = http()?
        .post(rules::GOOGLE_EVENTS)
        .bearer_auth(token)
        .query(&[("conferenceDataVersion", "1"), ("sendUpdates", "all")])
        .json(event)
        .send()
        .await
        .map_err(sent)?;
    answer(response).await
}

/// Takes back Legato's access when the account is removed.
pub async fn revoke(refresh: &str) {
    TOKENS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(refresh);
    let Ok(client) = http() else { return };
    if let Err(e) = client
        .post(rules::GOOGLE_REVOKE)
        .form(&[("token", refresh)])
        .send()
        .await
    {
        warn!("Couldn't tell Google to forget Legato: {}", e.without_url());
    }
}

/// A fresh request id, so a Meet link asked for twice is made once.
pub fn request_id() -> String {
    random(12).unwrap_or_else(|_| format!("{:x}", chrono::Utc::now().timestamp_millis()))
}
