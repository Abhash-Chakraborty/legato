//! Your calendars, from their secret iCal addresses (Google, Outlook, as
//! many accounts as you like) or a signed-in Google account (`google.rs`),
//! fetched every ten minutes so Legato knows your meetings: the bubble shows
//! the next one, notes take its name, and AI apps can ask for them. The
//! addresses and tokens are secrets and live in Credential Manager; the
//! meetings are cached in `calendar.json`, which the MCP server reads and
//! which covers a start without the network. A meeting added to a Google
//! account while offline waits there too, and goes out on the next fetch.

use crate::calendar_rules::{self as rules, Meeting};
use crate::settings::{get_settings, write_settings, CalendarFeed};
use chrono::{NaiveDateTime, TimeZone};
use log::{info, warn};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;
use tauri::AppHandle;
use tauri_specta::Event;
use tokio::sync::Notify;

/// The cache's file name in Legato's data folder.
pub const CACHE_FILE: &str = "calendar.json";
const EVERY: Duration = Duration::from_secs(10 * 60);
/// Meetings kept: from a day back to two weeks ahead.
const BACK: i64 = 86_400;
const AHEAD: i64 = 14 * 86_400;
/// A meeting this close counts as the one notes are for.
const SOON: i64 = 5 * 60;

/// A calendar as Settings › Connections lists it.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct CalendarStatus {
    pub id: String,
    pub name: String,
    /// Unix seconds of the last good fetch.
    pub synced_at: Option<i64>,
    pub error: Option<String>,
    /// Meetings in the next seven days.
    pub upcoming: u32,
    /// A Google account rather than an iCal address.
    pub google: bool,
    /// Meetings added offline, waiting to go to Google.
    pub waiting: u32,
}

/// A meeting to add to a Google account.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct NewMeeting {
    pub title: String,
    /// Unix seconds.
    pub start: i64,
    pub minutes: u32,
    pub invite: Vec<String>,
    pub details: Option<String>,
    /// Make a Google Meet link with it.
    pub meet: bool,
}

/// What happened to a meeting added to a Google account.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct AddedMeeting {
    /// Offline: it waits and goes out on the next fetch.
    pub waiting: bool,
    /// The event in Google Calendar, and its Meet link.
    pub link: Option<String>,
    pub meet: Option<String>,
}

/// A meeting added offline, as Google will be sent it.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Waiting {
    feed: String,
    event: serde_json::Value,
}

/// A removed calendar, for Undo.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct RemovedCalendar {
    pub name: String,
    pub url: String,
}

/// The calendars or their meetings changed.
#[derive(Clone, Debug, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct CalendarChanged;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct FeedState {
    synced_at: Option<i64>,
    error: Option<String>,
    meetings: Vec<Meeting>,
}

/// What's kept between runs, and what `legato --mcp` reads.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Cache {
    #[serde(default)]
    feeds: HashMap<String, FeedState>,
    #[serde(default)]
    waiting: Vec<Waiting>,
}

impl Cache {
    /// Every calendar's meetings, by start.
    pub fn meetings(&self) -> Vec<Meeting> {
        let mut all: Vec<Meeting> = self
            .feeds
            .values()
            .flat_map(|f| f.meetings.iter().cloned())
            .collect();
        all.sort_by(|a, b| a.start.cmp(&b.start).then(a.title.cmp(&b.title)));
        // One event on two calendars (a shared invite) shows once.
        all.dedup_by(|a, b| a.uid == b.uid && a.start == b.start);
        all
    }
}

static CACHE: Lazy<Mutex<Option<Cache>>> = Lazy::new(|| Mutex::new(None));
static WAKE: Lazy<Notify> = Lazy::new(Notify::new);

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn local(n: NaiveDateTime) -> Option<i64> {
    chrono::Local
        .from_local_datetime(&n)
        .earliest()
        .map(|t| t.timestamp())
}

fn cache_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    crate::portable::app_data_dir(app)
        .ok()
        .map(|d| d.join(CACHE_FILE))
}

/// The cache, read from disk the first time.
fn cache(app: &AppHandle) -> MutexGuard<'static, Option<Cache>> {
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        let loaded = cache_path(app)
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str::<Cache>(&s).ok())
            .unwrap_or_default();
        *guard = Some(loaded);
    }
    guard
}

fn save(app: &AppHandle, cache: &Cache) {
    let Some(path) = cache_path(app) else { return };
    match serde_json::to_string(cache) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                warn!("Couldn't save the calendar cache: {e}");
            }
        }
        Err(e) => warn!("Couldn't save the calendar cache: {e}"),
    }
}

fn secret_name(id: &str) -> String {
    format!("calendar-{id}")
}

/// The largest calendar file read; a busy work calendar is a few MB.
const MAX_FEED: usize = 16 * 1024 * 1024;

/// Fetches one address and reads its meetings. The address is the
/// calendar's password, so no error carries it (reqwest's do, which is why
/// they're never shown as they are).
async fn fetch(url: &str, name: &str) -> Result<Vec<Meeting>, String> {
    if rules::feed_url(url).is_none() {
        return Err("Calendars need an https address; add it again".into());
    }
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(25))
        .user_agent(concat!("Legato/", env!("CARGO_PKG_VERSION")))
        // A redirect would otherwise tell the next host the address.
        .referer(false)
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.url().scheme() == "https" && attempt.previous().len() < 5 {
                attempt.follow()
            } else {
                attempt.stop()
            }
        }))
        .build()
        .map_err(|_| "Couldn't start the calendar fetch".to_string())?;
    let mut response = client.get(url).send().await.map_err(|e| {
        if e.is_timeout() {
            "The calendar took too long to answer".to_string()
        } else {
            "Couldn't reach the calendar; check the connection".to_string()
        }
    })?;
    let status = response.status();
    if status == reqwest::StatusCode::NOT_FOUND || status == reqwest::StatusCode::FORBIDDEN {
        return Err("The address doesn't work any more; copy it again".into());
    }
    if status.is_redirection() {
        return Err("The calendar sent Legato somewhere else; copy the address again".into());
    }
    if !status.is_success() {
        return Err(format!("The calendar answered {}", status.as_u16()));
    }
    let too_big = || "The calendar is too big to read".to_string();
    if response
        .content_length()
        .is_some_and(|n| n > MAX_FEED as u64)
    {
        return Err(too_big());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| {
        if e.is_timeout() {
            "The calendar took too long to answer".to_string()
        } else {
            "The calendar's answer was cut off".to_string()
        }
    })? {
        if body.len() + chunk.len() > MAX_FEED {
            return Err(too_big());
        }
        body.extend_from_slice(&chunk);
    }
    let text = String::from_utf8_lossy(&body);
    if !rules::is_calendar(&text) {
        return Err("That address isn't a calendar; copy the iCal address".into());
    }
    let events = rules::parse(&text);
    let t = now();
    Ok(rules::meetings(&events, name, t - BACK, t + AHEAD, &local))
}

/// A Google account's meetings, after sending the ones added offline.
async fn fetch_google(
    app: &AppHandle,
    feed: &CalendarFeed,
    refresh: &str,
) -> Result<Vec<Meeting>, String> {
    let token = crate::google::access_token(refresh)
        .await
        .map_err(|e| e.to_string())?;
    let waiting: Vec<Waiting> = cache(app)
        .as_ref()
        .map(|c| {
            c.waiting
                .iter()
                .filter(|w| w.feed == feed.id)
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    for w in waiting {
        match crate::google::create(&token, &w.event).await {
            Ok(_) => info!("A meeting added offline went to Google Calendar"),
            Err(crate::google::GoogleError::Offline) => break,
            // Google said no (a bad address, say): it won't say yes later.
            Err(e) => warn!("Google Calendar refused a meeting added offline: {e}"),
        }
        let mut guard = cache(app);
        if let Some(cache) = guard.as_mut() {
            cache.waiting.retain(|x| x.event != w.event);
        }
    }
    let t = now();
    let events = crate::google::events(&token, t - BACK, t + AHEAD)
        .await
        .map_err(|e| e.to_string())?;
    Ok(rules::google_meetings(&events, &feed.name, &local))
}

/// Fetches every calendar and tells the windows.
async fn refresh(app: &AppHandle) {
    let feeds = get_settings(app).calendars;
    for feed in &feeds {
        if feed.google {
            let result = match crate::secrets::read_secret(&secret_name(&feed.id)) {
                Ok(Some(token)) => fetch_google(app, feed, &token).await,
                Ok(None) => Err("Sign in to Google again".into()),
                Err(e) => Err(e),
            };
            if let Err(e) = &result {
                warn!("Calendar \"{}\": {e}", feed.name);
            }
            set_feed(app, &feed.id, result);
            continue;
        }
        let url = match crate::secrets::read_secret(&secret_name(&feed.id)) {
            Ok(Some(url)) => url,
            Ok(None) => {
                set_feed(
                    app,
                    &feed.id,
                    Err("The address is missing; add the calendar again".into()),
                );
                continue;
            }
            Err(e) => {
                set_feed(app, &feed.id, Err(e));
                continue;
            }
        };
        let result = fetch(&url, &feed.name).await;
        if let Err(e) = &result {
            warn!("Calendar \"{}\": {e}", feed.name);
        }
        set_feed(app, &feed.id, result);
    }
    {
        let mut guard = cache(app);
        let cache = guard.get_or_insert_with(Cache::default);
        // Calendars removed meanwhile.
        cache
            .feeds
            .retain(|id, _| feeds.iter().any(|f| &f.id == id));
        cache
            .waiting
            .retain(|w| feeds.iter().any(|f| f.id == w.feed));
        save(app, cache);
    }
    let _ = CalendarChanged.emit(app);
}

fn set_feed(app: &AppHandle, id: &str, result: Result<Vec<Meeting>, String>) {
    let mut guard = cache(app);
    let cache = guard.get_or_insert_with(Cache::default);
    let state = cache.feeds.entry(id.to_string()).or_default();
    match result {
        Ok(meetings) => {
            state.meetings = meetings;
            state.synced_at = Some(now());
            state.error = None;
        }
        // The last meetings stay until a fetch works again.
        Err(e) => state.error = Some(e),
    }
}

/// Starts the ten-minute fetch.
pub fn init(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            if !get_settings(&app).calendars.is_empty() {
                refresh(&app).await;
            }
            tokio::select! {
                _ = tokio::time::sleep(EVERY) => {}
                _ = WAKE.notified() => {}
            }
        }
    });
}

/// Fetches now (Refresh, or the PC waking up).
pub fn refresh_soon() {
    WAKE.notify_one();
}

/// Every calendar's meetings, from a day back to two weeks ahead.
pub fn meetings(app: &AppHandle) -> Vec<Meeting> {
    cache(app).as_ref().map(Cache::meetings).unwrap_or_default()
}

/// The meeting going on now or about to start, for a note's title.
pub fn meeting_now(app: &AppHandle) -> Option<Meeting> {
    let all = meetings(app);
    rules::current(&all, now(), SOON).cloned()
}

pub fn statuses(app: &AppHandle) -> Vec<CalendarStatus> {
    let feeds = get_settings(app).calendars;
    let guard = cache(app);
    let week = now() + 7 * 86_400;
    feeds
        .into_iter()
        .map(|f| {
            let state = guard.as_ref().and_then(|c| c.feeds.get(&f.id));
            CalendarStatus {
                upcoming: state.map_or(0, |s| {
                    s.meetings
                        .iter()
                        .filter(|m| m.end > now() && m.start < week)
                        .count() as u32
                }),
                synced_at: state.and_then(|s| s.synced_at),
                error: state.and_then(|s| s.error.clone()),
                waiting: guard.as_ref().map_or(0, |c| {
                    c.waiting.iter().filter(|w| w.feed == f.id).count() as u32
                }),
                google: f.google,
                id: f.id,
                name: f.name,
            }
        })
        .collect()
}

/// A name for a calendar from its address, when none was typed.
fn name_for(url: &str, taken: &[CalendarFeed]) -> String {
    let base = if url.contains("calendar.google.com") {
        "Google Calendar"
    } else if url.contains("outlook.") || url.contains("office365.com") {
        "Outlook"
    } else if url.contains("icloud.com") {
        "iCloud"
    } else {
        "Calendar"
    };
    let mut name = base.to_string();
    let mut n = 2;
    while taken.iter().any(|f| f.name == name) {
        name = format!("{base} {n}");
        n += 1;
    }
    name
}

/// Adds a calendar after one good fetch.
pub async fn add(app: &AppHandle, name: &str, url: &str) -> Result<CalendarStatus, String> {
    let url = rules::feed_url(url).ok_or("Paste the calendar's secret iCal address")?;
    let mut settings = get_settings(app);
    let name = match name.trim() {
        "" => name_for(&url, &settings.calendars),
        typed => typed.to_string(),
    };
    let meetings = fetch(&url, &name).await?;
    let id = format!("{:x}", chrono::Utc::now().timestamp_millis());
    crate::secrets::write_secret(&secret_name(&id), &url)
        .map_err(|e| format!("Couldn't keep the address in Credential Manager: {e}"))?;
    settings.calendars.push(CalendarFeed {
        id: id.clone(),
        name: name.clone(),
        google: false,
    });
    write_settings(app, settings);
    set_feed(app, &id, Ok(meetings));
    if let Some(cache) = cache(app).as_ref() {
        save(app, cache);
    }
    info!("Calendar \"{name}\" added");
    let _ = CalendarChanged.emit(app);
    statuses(app)
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| "The calendar wasn't saved".into())
}

pub fn remove(app: &AppHandle, id: &str) -> Result<RemovedCalendar, String> {
    let mut settings = get_settings(app);
    let pos = settings
        .calendars
        .iter()
        .position(|f| f.id == id)
        .ok_or("That calendar is already gone")?;
    let feed = settings.calendars.remove(pos);
    let url = crate::secrets::read_secret(&secret_name(id))
        .ok()
        .flatten()
        .unwrap_or_default();
    let _ = crate::secrets::delete_secret(&secret_name(id));
    // A Google account isn't put back by Undo: Google forgets Legato.
    let url = if feed.google {
        let token = url;
        tauri::async_runtime::spawn(async move { crate::google::revoke(&token).await });
        String::new()
    } else {
        url
    };
    write_settings(app, settings);
    {
        let mut guard = cache(app);
        if let Some(cache) = guard.as_mut() {
            cache.feeds.remove(id);
            save(app, cache);
        }
    }
    let _ = CalendarChanged.emit(app);
    Ok(RemovedCalendar {
        name: feed.name,
        url,
    })
}

/// Signs in to a Google account in the browser and reads its meetings; an
/// account already here gets the new sign-in.
pub async fn connect_google(app: &AppHandle) -> Result<CalendarStatus, String> {
    let account = crate::google::sign_in(app).await?;
    let mut settings = get_settings(app);
    let existing = settings
        .calendars
        .iter()
        .find(|f| f.google && f.name.eq_ignore_ascii_case(&account.email))
        .map(|f| f.id.clone());
    let id = existing.unwrap_or_else(|| format!("{:x}", chrono::Utc::now().timestamp_millis()));
    crate::secrets::write_secret(&secret_name(&id), &account.refresh)
        .map_err(|e| format!("Couldn't keep the sign-in in Credential Manager: {e}"))?;
    if !settings.calendars.iter().any(|f| f.id == id) {
        settings.calendars.push(CalendarFeed {
            id: id.clone(),
            name: account.email.clone(),
            google: true,
        });
        write_settings(app, settings.clone());
    }
    let feed = settings
        .calendars
        .iter()
        .find(|f| f.id == id)
        .cloned()
        .ok_or("The account wasn't saved")?;
    let result = fetch_google(app, &feed, &account.refresh).await;
    set_feed(app, &id, result);
    if let Some(cache) = cache(app).as_ref() {
        save(app, cache);
    }
    info!("Google Calendar connected");
    let _ = CalendarChanged.emit(app);
    statuses(app)
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| "The account wasn't saved".into())
}

/// Adds a meeting to a Google account, with a Meet link when asked, and
/// invites its people. Offline, it waits and goes out on the next fetch.
pub async fn add_meeting(
    app: &AppHandle,
    feed_id: &str,
    m: NewMeeting,
) -> Result<AddedMeeting, String> {
    let feed = get_settings(app)
        .calendars
        .into_iter()
        .find(|f| f.id == feed_id && f.google)
        .ok_or("That Google account isn't connected any more")?;
    let refresh =
        crate::secrets::read_secret(&secret_name(&feed.id))?.ok_or("Sign in to Google again")?;
    let request = m.meet.then(crate::google::request_id);
    let event = rules::google_event(
        m.title.trim(),
        m.start,
        m.start + i64::from(m.minutes) * 60,
        &m.invite,
        m.details.as_deref(),
        request.as_deref(),
    );
    let sent = match crate::google::access_token(&refresh).await {
        Ok(token) => crate::google::create(&token, &event).await,
        Err(e) => Err(e),
    };
    match sent {
        Ok(created) => {
            refresh_soon();
            Ok(AddedMeeting {
                waiting: false,
                link: created["htmlLink"].as_str().map(str::to_string),
                meet: created["hangoutLink"].as_str().map(str::to_string),
            })
        }
        Err(crate::google::GoogleError::Offline) => {
            let mut guard = cache(app);
            let cache = guard.get_or_insert_with(Cache::default);
            cache.waiting.push(Waiting {
                feed: feed.id,
                event,
            });
            save(app, cache);
            drop(guard);
            let _ = CalendarChanged.emit(app);
            Ok(AddedMeeting {
                waiting: true,
                link: None,
                meet: None,
            })
        }
        Err(e) => Err(e.to_string()),
    }
}
