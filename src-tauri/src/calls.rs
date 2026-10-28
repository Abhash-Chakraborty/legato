//! Noticing calls. Windows records which apps use the microphone; when a
//! watched app holds it, the Legato bar offers notes, and notes started for
//! a call stop soon after the app lets go of the microphone. Legato reads
//! those records and, for Google Meet, Teams and Zoom in a browser, window
//! titles; never audio. A minute before a meeting on your calendar, the bar
//! offers to join it, with or without notes.

use crate::calendar_rules::{self, Meeting};
use crate::calls_rules::{self as rules, CallApp, CallEvent, Holder, Tracker};
use crate::managers::notes_store::NoteKind;
use crate::notes::{self, StartNotes};
use crate::settings::{get_settings, write_settings, CallSettings, LastCall};
use crate::transforms::BarNotice;
use log::{info, warn};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tauri_specta::Event;

/// How often the watcher looks again when Windows reports no change, for
/// the two-second start rule and for browser titles.
const SCAN_EVERY: Duration = Duration::from_secs(2);

/// A card that doesn't count down stays this long.
const OFFER_FOR: Duration = Duration::from_secs(60);

/// The call ended card stays this long when notes keep recording.
const ENDED_FOR: Duration = Duration::from_secs(15);

/// A call going on now, as the Notes page shows it.
#[derive(Serialize, Clone, Debug, Type)]
pub struct LiveCall {
    pub app: CallApp,
    pub name: String,
    /// Unix seconds.
    pub since: i64,
}

/// What Notes and Settings show about calls.
#[derive(Serialize, Clone, Debug, Type)]
pub struct CallState {
    pub detect: bool,
    pub watched: Vec<CallApp>,
    pub live: Option<LiveCall>,
}

#[derive(Serialize, Clone, Debug, Type, tauri_specta::Event)]
pub struct CallsChanged {
    pub state: CallState,
}

/// What a button on the call card, or Start notes on the Notes page, does.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum CallAction {
    Start,
    NotNow,
    Never,
    StopNow,
    Keep,
    UndoNever,
    /// On a meeting's card: opens its link.
    Join,
    /// On a meeting's card: opens its link and starts notes.
    JoinNotes,
}

/// A button pressed on the card Test detection showed; the page says what
/// would have happened.
#[derive(Serialize, Clone, Debug, Type, tauri_specta::Event)]
pub struct CallTest {
    pub action: CallAction,
    pub app: CallApp,
    pub snooze_minutes: u32,
}

/// The call card, as the Legato bar draws it.
#[derive(Serialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum CallCard {
    Offer {
        app: CallApp,
        name: &'static str,
        app_path: Option<String>,
        /// Notes start on their own after this long.
        countdown_ms: Option<u64>,
        test: bool,
    },
    Ended {
        app: CallApp,
        name: &'static str,
        app_path: Option<String>,
        /// Notes stop after this long.
        stop_in_ms: Option<u64>,
        elapsed_ms: i64,
    },
    /// A meeting on your calendar about to start: Join with notes, Join,
    /// Not now; Take notes when it has no link.
    Meeting {
        title: String,
        /// Unix seconds.
        starts_at: i64,
        link: bool,
        /// The earlier note of this meeting, when there is one.
        last: Option<notes::LastTime>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CardKind {
    Offer,
    Ended,
}

#[derive(Clone, Copy)]
struct Shown {
    app: CallApp,
    kind: CardKind,
    test: bool,
}

/// When notes may be offered again for a call after Not now.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ask {
    Yes,
    After(u64),
    NotThisCall,
}

struct Live {
    app: CallApp,
    since: i64,
    ask: Ask,
}

#[derive(Default)]
struct State {
    tracker: Tracker,
    live: Vec<Live>,
    card: Option<Shown>,
    /// The browser whose window was the call, and the call it showed.
    call_browser: Option<(String, CallApp)>,
    /// The calendar meeting the card offers to join.
    meeting: Option<Meeting>,
    /// Meetings offered already, by uid and start.
    reminded: Vec<(String, i64)>,
    /// Notes started from Join with notes, until the call they're for starts.
    joined: Option<i64>,
    /// The note started for a call.
    note: Option<(i64, CallApp)>,
    /// The app Never here last turned off, for Undo.
    never: Option<CallApp>,
}

static STATE: Lazy<Mutex<State>> = Lazy::new(|| Mutex::new(State::default()));
/// Bumped whenever the card changes, so a timer for an older card does
/// nothing.
static GENERATION: AtomicU64 = AtomicU64::new(0);
static EPOCH: Lazy<Instant> = Lazy::new(Instant::now);

fn lock() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

fn now_ms() -> u64 {
    EPOCH.elapsed().as_millis() as u64
}

fn next_generation() -> u64 {
    GENERATION.fetch_add(1, Ordering::SeqCst) + 1
}

/// Runs `then` after `wait`, unless the card changed in the meantime.
fn after(app: &AppHandle, wait: Duration, then: impl FnOnce(&AppHandle) + Send + 'static) {
    let generation = GENERATION.load(Ordering::SeqCst);
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(wait);
        if GENERATION.load(Ordering::SeqCst) == generation {
            then(&app);
        }
    });
}

pub fn state(app: &AppHandle) -> CallState {
    let settings = get_settings(app).calls;
    let st = lock();
    CallState {
        detect: settings.detect,
        watched: settings.watched.clone(),
        live: st.live.last().map(|l| LiveCall {
            app: l.app,
            name: l.app.name().to_string(),
            since: l.since,
        }),
    }
}

fn changed(app: &AppHandle) {
    let _ = CallsChanged { state: state(app) }.emit(app);
}

fn save_calls(app: &AppHandle, edit: impl FnOnce(&mut CallSettings)) {
    let mut settings = get_settings(app);
    edit(&mut settings.calls);
    write_settings(app, settings);
}

/// Saves Settings › Context & calls; the last call stays as Legato saw it.
pub fn set_settings(app: &AppHandle, calls: CallSettings) {
    save_calls(app, |current| {
        let last = current.last.take();
        *current = CallSettings { last, ..calls };
    });
    changed(app);
}

/// The executable that owns the call, to find its sound for notes.
fn app_path(app: CallApp, browser: Option<&(String, CallApp)>) -> Option<String> {
    crate::loopback::audio_apps()
        .into_iter()
        .find(|a| rules::carries_call(&a.exe_path, app, browser))
        .map(|a| a.exe_path)
}

fn show_card(app: &AppHandle, card: Option<CallCard>) {
    crate::overlay::set_call_card(app, card.as_ref());
}

fn hide_card(app: &AppHandle) {
    next_generation();
    {
        let mut st = lock();
        st.card = None;
        st.meeting = None;
    }
    show_card(app, None);
}

/// Notes were switched off in Settings › Features: a card offering them
/// goes. One about notes already recording stays, so they can be stopped.
pub fn notes_turned_off(app: &AppHandle) {
    if !notes::is_recording() {
        hide_card(app);
    }
}

/// Offers to join a meeting on the calendar that's about to start, unless
/// a card shows or notes already run.
fn remind_due(app: &AppHandle) {
    let settings = get_settings(app);
    if !settings.calls.meeting_cards || !settings.notes_enabled {
        return;
    }
    let now = chrono::Utc::now().timestamp();
    let meetings = crate::calendar::meetings(app);
    let meeting = {
        let mut st = lock();
        if st.card.is_some() || st.meeting.is_some() || notes::is_busy() {
            return;
        }
        let Some(m) = calendar_rules::to_remind(&meetings, now, &st.reminded).cloned() else {
            return;
        };
        st.reminded.push((m.uid.clone(), m.start));
        let keep = st.reminded.len().saturating_sub(50);
        st.reminded.drain(..keep);
        st.meeting = Some(m.clone());
        m
    };
    let last = notes::last_time(app, &meeting.title, &meeting.people);
    info!(
        "Offering to join a meeting from the calendar{}",
        if last.is_some() {
            ", with last time's note"
        } else {
            ""
        }
    );
    next_generation();
    show_card(
        app,
        Some(CallCard::Meeting {
            title: meeting.title.clone(),
            starts_at: meeting.start,
            link: meeting.link.is_some(),
            last,
        }),
    );
    let left = (meeting.start + calendar_rules::REMIND_LATE - now).max(30) as u64;
    after(app, Duration::from_secs(left), hide_card);
}

/// A button on a meeting's card.
fn meeting_action(app: &AppHandle, meeting: &Meeting, action: CallAction) {
    let join = matches!(action, CallAction::Join | CallAction::JoinNotes);
    if join {
        if let Some(link) = &meeting.link {
            use tauri_plugin_opener::OpenerExt;
            if let Err(e) = app.opener().open_url(link, None::<String>) {
                warn!("Opening the meeting's link: {e}");
            }
        }
    }
    let notes = matches!(action, CallAction::JoinNotes | CallAction::Start);
    if !notes || notes::is_busy() {
        return;
    }
    // Everything the PC plays is recorded, since the call's app isn't known
    // yet; once the call starts, its end stops these notes like a call's.
    let result = notes::start(
        app,
        StartNotes {
            kind: NoteKind::Meeting,
            title: Some(meeting.title.clone()),
            microphone: None,
            app_pid: None,
        },
    );
    match result {
        Ok(note) => lock().joined = Some(note.id),
        Err(e) => {
            warn!("Notes for the meeting didn't start: {e}");
            crate::overlay::show_overlay_notice(
                app,
                &BarNotice::NotesFailed {
                    reason: e.to_string(),
                },
            );
        }
    }
}

/// Offers notes for a call: Start notes, Not now, Never here, and a
/// countdown when notes start on their own.
fn offer(app: &AppHandle, call: CallApp, test: bool) {
    let settings = get_settings(app).calls;
    let countdown_ms = settings
        .auto_start
        .then(|| settings.countdown_secs.clamp(1, 60) as u64 * 1000);
    let browser = {
        let mut st = lock();
        st.card = Some(Shown {
            app: call,
            kind: CardKind::Offer,
            test,
        });
        // A call card takes the place of a meeting's.
        st.meeting = None;
        st.call_browser.clone()
    };
    next_generation();
    show_card(
        app,
        Some(CallCard::Offer {
            app: call,
            name: call.name(),
            app_path: app_path(call, browser.as_ref()),
            countdown_ms,
            test,
        }),
    );
    match countdown_ms {
        Some(ms) => after(app, Duration::from_millis(ms), |app| {
            act(app, CallAction::Start)
        }),
        None => after(app, OFFER_FOR, |app| act(app, CallAction::NotNow)),
    }
}

/// Shows the card for the first watched app, as if it started a call.
/// Nothing is recorded.
pub fn test_card(app: &AppHandle) -> Result<(), String> {
    let settings = get_settings(app).calls;
    let call = settings
        .watched
        .first()
        .copied()
        .ok_or_else(|| "Turn on at least one app to watch".to_string())?;
    offer(app, call, true);
    Ok(())
}

fn when() -> String {
    chrono::Local::now().format("%b %-d, %-I:%M %p").to_string()
}

fn start_notes(app: &AppHandle, call: CallApp) {
    let browser = lock().call_browser.clone();
    let app_pid = crate::loopback::audio_apps()
        .into_iter()
        .find(|a| rules::carries_call(&a.exe_path, call, browser.as_ref()))
        .map(|a| a.pid);
    let result = notes::start(
        app,
        StartNotes {
            kind: NoteKind::Meeting,
            title: Some(
                crate::calendar::meeting_now(app)
                    .map(|m| m.title)
                    .unwrap_or_else(|| format!("{} call · {}", call.name(), when())),
            ),
            microphone: None,
            app_pid,
        },
    );
    match result {
        Ok(note) => {
            lock().note = Some((note.id, call));
            save_calls(app, |c| {
                if let Some(last) = c.last.as_mut().filter(|l| l.app == call) {
                    last.note_id = Some(note.id);
                }
            });
        }
        Err(e) => {
            warn!("Notes for the call didn't start: {e}");
            crate::overlay::show_overlay_notice(
                app,
                &BarNotice::NotesFailed {
                    reason: e.to_string(),
                },
            );
        }
    }
}

/// A button on the call card, or Start notes on the Notes page.
pub fn act(app: &AppHandle, action: CallAction) {
    let settings = get_settings(app).calls;
    let meeting = lock().meeting.clone();
    if let Some(meeting) = meeting {
        if matches!(
            action,
            CallAction::Join | CallAction::JoinNotes | CallAction::Start | CallAction::NotNow
        ) {
            hide_card(app);
            meeting_action(app, &meeting, action);
            return;
        }
    }
    let shown = lock().card;
    let test = shown.is_some_and(|s| s.test);
    let call = if action == CallAction::UndoNever {
        lock().never.take()
    } else {
        shown
            .map(|s| s.app)
            .or_else(|| lock().live.last().map(|l| l.app))
    };
    if !matches!(action, CallAction::UndoNever) {
        hide_card(app);
    }
    let Some(call) = call else {
        return;
    };
    if test && matches!(action, CallAction::Start | CallAction::NotNow) {
        let _ = CallTest {
            action,
            app: call,
            snooze_minutes: settings.snooze_minutes,
        }
        .emit(app);
        return;
    }
    match action {
        CallAction::Start => {
            if !notes::is_busy() {
                start_notes(app, call);
            }
        }
        CallAction::NotNow => {
            let ask = rules::offer_again_at(now_ms(), settings.snooze_minutes)
                .map_or(Ask::NotThisCall, Ask::After);
            if let Some(live) = lock().live.iter_mut().find(|l| l.app == call) {
                live.ask = ask;
            }
        }
        CallAction::Never => {
            lock().never = Some(call);
            save_calls(app, |c| c.watched.retain(|a| *a != call));
            changed(app);
            crate::overlay::show_overlay_notice_for(
                app,
                &BarNotice::CallNever {
                    app: call.name().to_string(),
                },
                5000,
            );
        }
        CallAction::UndoNever => {
            save_calls(app, |c| {
                if !c.watched.contains(&call) {
                    c.watched.push(call);
                }
            });
            changed(app);
            crate::overlay::hide_recording_overlay(app);
        }
        CallAction::StopNow => {
            if let Err(e) = notes::stop(app) {
                warn!("Stopping notes for the call: {e}");
            }
        }
        CallAction::Keep => {
            // Keep recording: the end of this call no longer stops it.
            lock().note = None;
        }
        // Only on a meeting's card, handled above.
        CallAction::Join | CallAction::JoinNotes => {}
    }
}

fn started(app: &AppHandle, call: CallApp) {
    let since = chrono::Utc::now().timestamp();
    info!("Call started in {}", call.name());
    let resumed = {
        let mut st = lock();
        st.live.push(Live {
            app: call,
            since,
            ask: Ask::Yes,
        });
        // Notes from Join with notes belong to this call now, so its end
        // stops them.
        if let Some(id) = st.joined.take() {
            if st.note.is_none() && notes::session().is_some_and(|s| s.note_id == id) {
                st.note = Some((id, call));
            }
        }
        st.card
            .is_some_and(|c| c.app == call && c.kind == CardKind::Ended)
    };
    save_calls(app, |c| {
        c.last = Some(LastCall {
            app: call,
            at: since,
            minutes: None,
            note_id: None,
        })
    });
    if resumed {
        // Back on the microphone within the grace: the call goes on.
        hide_card(app);
    } else if !notes::is_busy() {
        offer(app, call, false);
    }
    changed(app);
}

fn ended(app: &AppHandle, call: CallApp) {
    info!("Call ended in {}", call.name());
    let (since, shown, note) = {
        let mut st = lock();
        let since = st.live.iter().find(|l| l.app == call).map(|l| l.since);
        st.live.retain(|l| l.app != call);
        (since, st.card, st.note)
    };
    if let Some(since) = since {
        let minutes = ((chrono::Utc::now().timestamp() - since) as f64 / 60.0).round() as u32;
        save_calls(app, |c| {
            if let Some(last) = c.last.as_mut().filter(|l| l.app == call && l.at == since) {
                last.minutes = Some(minutes.max(1));
            }
        });
    }
    if shown.is_some_and(|s| s.app == call && s.kind == CardKind::Offer) {
        hide_card(app);
    }
    let recording_this = note.is_some_and(|(id, a)| {
        a == call && notes::session().is_some_and(|s| s.note_id == id && notes::is_recording())
    });
    if recording_this {
        let settings = get_settings(app).calls;
        let elapsed_ms = notes::session().map_or(0, |s| s.elapsed_ms);
        let stop_in_ms = settings.stop_on_end.then_some(rules::END_GRACE_MS);
        let browser = {
            let mut st = lock();
            st.card = Some(Shown {
                app: call,
                kind: CardKind::Ended,
                test: false,
            });
            st.call_browser.clone()
        };
        next_generation();
        show_card(
            app,
            Some(CallCard::Ended {
                app: call,
                name: call.name(),
                app_path: app_path(call, browser.as_ref()),
                stop_in_ms,
                elapsed_ms,
            }),
        );
        match stop_in_ms {
            Some(ms) => after(app, Duration::from_millis(ms), |app| {
                act(app, CallAction::StopNow)
            }),
            None => after(app, ENDED_FOR, hide_card),
        }
    }
    changed(app);
}

/// Notes stopped: a call ended card has nothing left to stop.
pub fn notes_stopped(app: &AppHandle) {
    lock().joined = None;
    let ended_card = lock().card.is_some_and(|c| c.kind == CardKind::Ended);
    if ended_card {
        hide_card(app);
    }
}

/// A note is ready: opens it when it was taken for a call and Settings asks
/// for that.
pub fn note_ready(app: &AppHandle, note_id: i64) {
    let ours = {
        let mut st = lock();
        let ours = st.note.is_some_and(|(id, _)| id == note_id);
        if ours {
            st.note = None;
        }
        ours
    };
    if ours && get_settings(app).calls.open_note {
        crate::open_page(app, "notes", Some(&note_id.to_string()));
    }
}

/// Offers notes again for calls whose Not now has run out.
fn offer_due(app: &AppHandle) {
    let now = now_ms();
    let due = {
        let mut st = lock();
        if st.card.is_some() || notes::is_busy() {
            return;
        }
        let due = st
            .live
            .iter_mut()
            .find(|l| matches!(l.ask, Ask::After(t) if now >= t));
        due.map(|l| {
            l.ask = Ask::Yes;
            l.app
        })
    };
    if let Some(call) = due {
        offer(app, call, false);
    }
}

/// What the log last said about the microphone, so it speaks only when
/// that changes.
static LAST_HOLDERS: Mutex<String> = Mutex::new(String::new());

/// Logs who holds the microphone when it changes, and what Legato made of
/// each, so a call that brought no card can be explained afterwards. Only
/// app names are logged, never window titles.
fn log_holders(users: &[(String, bool)], found: &[CallApp]) {
    let described: Vec<String> = users
        .iter()
        .map(|(entry, packaged)| {
            let name = if *packaged {
                entry.split('_').next().unwrap_or(entry).to_string()
            } else {
                rules::exe_of(entry)
            };
            let what = match rules::holder_for_entry(entry, *packaged) {
                Holder::App(call) if found.contains(&call) => "a call app, watched",
                Holder::App(_) => "a call app, not watched",
                Holder::Browser(exe) => {
                    let call = lock()
                        .call_browser
                        .as_ref()
                        .filter(|(b, _)| *b == exe)
                        .map(|(_, call)| *call);
                    if call.is_some_and(|c| found.contains(&c)) {
                        "a browser with a call"
                    } else {
                        "a browser, no call tab in front"
                    }
                }
                Holder::Legato => "Legato",
                Holder::Other => "not a call app",
            };
            format!("{name} ({what})")
        })
        .collect();
    let line = if described.is_empty() {
        "Microphone not in use".to_string()
    } else {
        format!("Microphone in use by {}", described.join(", "))
    };
    let mut last = LAST_HOLDERS.lock().unwrap_or_else(|e| e.into_inner());
    if *last != line {
        info!("{line}");
        *last = line;
    }
}

/// Who holds the microphone now, among the apps watched.
fn holding(settings: &CallSettings) -> Vec<CallApp> {
    let users = rules::merge_users(mic_users(), &capturing_exes());
    let found = holding_among(settings, &users);
    log_holders(&users, &found);
    found
}

fn holding_among(settings: &CallSettings, users: &[(String, bool)]) -> Vec<CallApp> {
    let mut found = Vec::new();
    let mut in_app = Vec::new();
    let mut in_tab = None;
    for (entry, packaged) in users.iter().cloned() {
        match rules::holder_for_entry(&entry, packaged) {
            Holder::App(call) if settings.watched.contains(&call) => {
                in_app.push(call);
                found.push(call);
            }
            Holder::Browser(exe) if settings.browser_titles => {
                // The tab only names the call while it's in front; once the
                // call is going, the browser keeping the microphone is
                // enough, so switching tabs doesn't end it.
                let going = {
                    let st = lock();
                    st.call_browser
                        .as_ref()
                        .filter(|(b, call)| *b == exe && st.tracker.is_live(*call))
                        .map(|(_, call)| *call)
                };
                let call =
                    going.or_else(|| browser_call(&exe).filter(|c| settings.watched.contains(c)));
                if let Some(call) = call {
                    lock().call_browser = Some((exe, call));
                    in_tab = Some(call);
                    found.push(call);
                }
            }
            _ => {}
        }
    }
    // The call has moved to its app (or a new one started there): its
    // sound comes from the app now, not the tab it was in before.
    let mut st = lock();
    if st
        .call_browser
        .as_ref()
        .is_some_and(|(_, call)| in_app.contains(call) && in_tab != Some(*call))
    {
        st.call_browser = None;
    }
    drop(st);
    found.sort_by_key(|a| CallApp::ALL.iter().position(|b| b == a));
    found.dedup();
    found
}

/// Starts watching for calls, for as long as Legato runs.
pub fn init(app: &AppHandle) {
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("call-watch".into())
        .spawn(move || {
            let mut watcher = Watcher::open();
            loop {
                let all = get_settings(&app);
                let settings = all.calls;
                let now = if settings.detect && all.notes_enabled {
                    holding(&settings)
                } else {
                    Vec::new()
                };
                let events = lock().tracker.update(now_ms(), &now);
                for event in events {
                    match event {
                        CallEvent::Started(call) => started(&app, call),
                        CallEvent::Ended(call) => ended(&app, call),
                    }
                }
                offer_due(&app);
                remind_due(&app);
                watcher.wait(SCAN_EVERY);
            }
        });
    if let Err(e) = spawned {
        warn!("Call detection didn't start: {e}");
    }
}

#[cfg(windows)]
use win::{browser_call, capturing_exes, mic_users, Watcher};

#[cfg(not(windows))]
fn mic_users() -> Vec<(String, bool)> {
    Vec::new()
}

#[cfg(not(windows))]
fn capturing_exes() -> Vec<String> {
    Vec::new()
}

#[cfg(not(windows))]
fn browser_call(_exe: &str) -> Option<CallApp> {
    None
}

#[cfg(not(windows))]
struct Watcher;

#[cfg(not(windows))]
impl Watcher {
    fn open() -> Self {
        Watcher
    }
    fn wait(&mut self, timeout: Duration) {
        std::thread::sleep(timeout);
    }
}

#[cfg(windows)]
mod win {
    use super::rules::{self, CallApp};
    use std::time::Duration;
    use windows::core::{w, Interface, BOOL, PCWSTR, PWSTR};
    use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM};
    use windows::Win32::Media::Audio::{
        eCapture, AudioSessionStateActive, IAudioSessionControl2, IAudioSessionManager2,
        IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED,
    };
    use windows::Win32::System::Registry::{
        RegCloseKey, RegEnumKeyExW, RegNotifyChangeKeyValue, RegOpenKeyExW, RegQueryValueExW, HKEY,
        HKEY_CURRENT_USER, KEY_NOTIFY, KEY_READ, REG_NOTIFY_CHANGE_LAST_SET,
        REG_NOTIFY_CHANGE_NAME,
    };
    use windows::Win32::System::Threading::{
        CreateEventW, OpenProcess, QueryFullProcessImageNameW, WaitForSingleObject,
        PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
    };

    const MICROPHONE: PCWSTR = w!(
        "Software\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\microphone"
    );

    fn open(parent: HKEY, path: PCWSTR) -> Option<HKEY> {
        let mut key = HKEY::default();
        // SAFETY: opens a key for reading; closed by the caller.
        let opened = unsafe { RegOpenKeyExW(parent, path, None, KEY_READ | KEY_NOTIFY, &mut key) };
        (opened.0 == 0).then_some(key)
    }

    fn subkeys(key: HKEY) -> Vec<String> {
        let mut names = Vec::new();
        for index in 0.. {
            let mut buf = vec![0u16; 512];
            let mut len = buf.len() as u32;
            // SAFETY: the buffer and its length match.
            let result = unsafe {
                RegEnumKeyExW(
                    key,
                    index,
                    Some(PWSTR(buf.as_mut_ptr())),
                    &mut len,
                    None,
                    None,
                    None,
                    None,
                )
            };
            if result.0 != 0 {
                break;
            }
            names.push(String::from_utf16_lossy(&buf[..len as usize]));
        }
        names
    }

    fn qword(key: HKEY, name: PCWSTR) -> u64 {
        let mut value = 0u64;
        let mut size = std::mem::size_of::<u64>() as u32;
        // SAFETY: reads at most eight bytes into `value`.
        let result = unsafe {
            RegQueryValueExW(
                key,
                name,
                None,
                None,
                Some(&mut value as *mut u64 as *mut u8),
                Some(&mut size),
            )
        };
        if result.0 == 0 {
            value
        } else {
            0
        }
    }

    /// Whether the entry `name` under `parent` holds the microphone now.
    fn entry_in_use(parent: HKEY, name: &str) -> bool {
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let Some(key) = open(parent, PCWSTR(wide.as_ptr())) else {
            return false;
        };
        let started = qword(key, w!("LastUsedTimeStart"));
        let stopped = qword(key, w!("LastUsedTimeStop"));
        // SAFETY: closes the key opened above.
        unsafe {
            let _ = RegCloseKey(key);
        }
        rules::in_use(started, stopped)
    }

    /// The apps holding the microphone: packaged apps by family name, and
    /// desktop apps by path.
    pub fn mic_users() -> Vec<(String, bool)> {
        let Some(root) = open(HKEY_CURRENT_USER, MICROPHONE) else {
            return Vec::new();
        };
        let mut users = Vec::new();
        for name in subkeys(root) {
            if name.eq_ignore_ascii_case("NonPackaged") {
                continue;
            }
            if entry_in_use(root, &name) {
                users.push((name, true));
            }
        }
        if let Some(desktop) = open(root, w!("NonPackaged")) {
            for name in subkeys(desktop) {
                if entry_in_use(desktop, &name) {
                    users.push((name, false));
                }
            }
            // SAFETY: closes the key opened above.
            unsafe {
                let _ = RegCloseKey(desktop);
            }
        }
        // SAFETY: closes the key opened above.
        unsafe {
            let _ = RegCloseKey(root);
        }
        users
    }

    /// The executables with an active recording session on any microphone.
    /// Unlike the privacy records, which Windows may only write when a
    /// stream stops, these show a call while it is going.
    pub fn capturing_exes() -> Vec<String> {
        // SAFETY: COM calls on this thread, which initialises COM first.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let Ok(enumerator) =
                CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL)
            else {
                return Vec::new();
            };
            let Ok(devices) = enumerator.EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE) else {
                return Vec::new();
            };
            let mut exes = Vec::new();
            for d in 0..devices.GetCount().unwrap_or(0) {
                let Ok(device) = devices.Item(d) else {
                    continue;
                };
                let Ok(manager) = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else {
                    continue;
                };
                let Ok(sessions) = manager.GetSessionEnumerator() else {
                    continue;
                };
                for i in 0..sessions.GetCount().unwrap_or(0) {
                    let Ok(control) = sessions.GetSession(i) else {
                        continue;
                    };
                    if !control
                        .GetState()
                        .is_ok_and(|s| s == AudioSessionStateActive)
                    {
                        continue;
                    }
                    let Ok(control) = control.cast::<IAudioSessionControl2>() else {
                        continue;
                    };
                    let Some(exe) = control.GetProcessId().ok().and_then(exe_of_process) else {
                        continue;
                    };
                    if !exes.contains(&exe) {
                        exes.push(exe);
                    }
                }
            }
            exes
        }
    }

    fn exe_of_process(pid: u32) -> Option<String> {
        // SAFETY: opens the process for a limited query and closes it.
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buf = vec![0u16; 1024];
            let mut size = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_WIN32,
                PWSTR(buf.as_mut_ptr()),
                &mut size,
            );
            let _ = CloseHandle(process);
            ok.ok()?;
            Some(rules::exe_of(&String::from_utf16_lossy(
                &buf[..size as usize],
            )))
        }
    }

    struct Search {
        exe: String,
        found: Option<CallApp>,
    }

    unsafe extern "system" fn visit(hwnd: HWND, data: LPARAM) -> BOOL {
        let search = &mut *(data.0 as *mut Search);
        if !IsWindowVisible(hwnd).as_bool() {
            return BOOL(1);
        }
        let mut buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, &mut buf);
        if len <= 0 {
            return BOOL(1);
        }
        let title = String::from_utf16_lossy(&buf[..len as usize]);
        let Some(call) = rules::web_call(&title) else {
            return BOOL(1);
        };
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if exe_of_process(pid).as_deref() == Some(search.exe.as_str()) {
            search.found = Some(call);
            return BOOL(0);
        }
        BOOL(1)
    }

    /// The call a window of this browser has in front: Google Meet, Teams
    /// or Zoom.
    pub fn browser_call(exe: &str) -> Option<CallApp> {
        let mut search = Search {
            exe: exe.to_string(),
            found: None,
        };
        // SAFETY: `search` outlives the enumeration, which is synchronous.
        unsafe {
            let _ = EnumWindows(Some(visit), LPARAM(&mut search as *mut Search as isize));
        }
        search.found
    }

    /// Waits for Windows to change the microphone records, or for the
    /// timeout.
    pub struct Watcher {
        key: Option<HKEY>,
        event: Option<HANDLE>,
    }

    impl Watcher {
        pub fn open() -> Self {
            let key = open(HKEY_CURRENT_USER, MICROPHONE);
            // SAFETY: an unnamed auto-reset event, closed on drop.
            let event = unsafe { CreateEventW(None, false, false, None) }.ok();
            Watcher { key, event }
        }

        pub fn wait(&mut self, timeout: Duration) {
            let (Some(key), Some(event)) = (self.key, self.event) else {
                std::thread::sleep(timeout);
                return;
            };
            // SAFETY: asks Windows to signal `event` on the next change
            // under the key; both live as long as the watcher.
            let armed = unsafe {
                RegNotifyChangeKeyValue(
                    key,
                    true,
                    REG_NOTIFY_CHANGE_LAST_SET | REG_NOTIFY_CHANGE_NAME,
                    Some(event),
                    true,
                )
            };
            if armed.0 != 0 {
                std::thread::sleep(timeout);
                return;
            }
            // SAFETY: waits on the event created in `open`.
            unsafe {
                WaitForSingleObject(event, timeout.as_millis() as u32);
            }
        }
    }

    impl Drop for Watcher {
        fn drop(&mut self) {
            // SAFETY: closes what `open` created.
            unsafe {
                if let Some(key) = self.key {
                    let _ = RegCloseKey(key);
                }
                if let Some(event) = self.event {
                    let _ = CloseHandle(event);
                }
            }
        }
    }
}
