//! Noticing calls. Windows records which apps use the microphone; when a
//! watched app holds it, the Legato bar offers notes, and notes started for
//! a call stop soon after the app lets go of the microphone. Legato reads
//! those records and, for Google Meet, browser window titles; never audio.

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
    /// The browser whose window was the Meet call.
    meet_browser: Option<String>,
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
fn app_path(app: CallApp, meet_browser: Option<&str>) -> Option<String> {
    crate::loopback::audio_apps()
        .into_iter()
        .find(|a| owns(app, meet_browser, &a.exe_path))
        .map(|a| a.exe_path)
}

fn owns(app: CallApp, meet_browser: Option<&str>, exe_path: &str) -> bool {
    match rules::holder_for_exe(exe_path) {
        Holder::App(a) => a == app,
        Holder::Browser(exe) => app == CallApp::Meet && meet_browser == Some(exe.as_str()),
        _ => false,
    }
}

fn show_card(app: &AppHandle, card: Option<CallCard>) {
    let hide = get_settings(app).calls.hide_card_from_capture;
    crate::overlay::set_call_card(app, card.as_ref(), hide);
}

fn hide_card(app: &AppHandle) {
    next_generation();
    lock().card = None;
    show_card(app, None);
}

/// Offers notes for a call: Start notes, Not now, Never here, and a
/// countdown when notes start on their own.
fn offer(app: &AppHandle, call: CallApp, test: bool) {
    let settings = get_settings(app).calls;
    let countdown_ms = settings
        .auto_start
        .then(|| settings.countdown_secs.clamp(1, 60) as u64 * 1000);
    let meet_browser = {
        let mut st = lock();
        st.card = Some(Shown {
            app: call,
            kind: CardKind::Offer,
            test,
        });
        st.meet_browser.clone()
    };
    next_generation();
    show_card(
        app,
        Some(CallCard::Offer {
            app: call,
            name: call.name(),
            app_path: app_path(call, meet_browser.as_deref()),
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
    let meet_browser = lock().meet_browser.clone();
    let app_pid = crate::loopback::audio_apps()
        .into_iter()
        .find(|a| owns(call, meet_browser.as_deref(), &a.exe_path))
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
        let meet_browser = {
            let mut st = lock();
            st.card = Some(Shown {
                app: call,
                kind: CardKind::Ended,
                test: false,
            });
            st.meet_browser.clone()
        };
        next_generation();
        show_card(
            app,
            Some(CallCard::Ended {
                app: call,
                name: call.name(),
                app_path: app_path(call, meet_browser.as_deref()),
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

/// Who holds the microphone now, among the apps watched.
fn holding(settings: &CallSettings) -> Vec<CallApp> {
    let mut found = Vec::new();
    for (entry, packaged) in mic_users() {
        match rules::holder_for_entry(&entry, packaged) {
            Holder::App(call) if settings.watched.contains(&call) => found.push(call),
            Holder::Browser(exe)
                if settings.browser_titles && settings.watched.contains(&CallApp::Meet) =>
            {
                // The Meet tab only names the call while it's in front; once
                // the call is going, the browser keeping the microphone is
                // enough, so switching tabs doesn't end it.
                let going = {
                    let st = lock();
                    st.tracker.is_live(CallApp::Meet)
                        && st.meet_browser.as_deref() == Some(exe.as_str())
                };
                if going || browser_shows_meet(&exe) {
                    lock().meet_browser = Some(exe);
                    found.push(CallApp::Meet);
                }
            }
            _ => {}
        }
    }
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
                let settings = get_settings(&app).calls;
                let now = if settings.detect {
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
                watcher.wait(SCAN_EVERY);
            }
        });
    if let Err(e) = spawned {
        warn!("Call detection didn't start: {e}");
    }
}

#[cfg(windows)]
use win::{browser_shows_meet, mic_users, Watcher};

#[cfg(not(windows))]
fn mic_users() -> Vec<(String, bool)> {
    Vec::new()
}

#[cfg(not(windows))]
fn browser_shows_meet(_exe: &str) -> bool {
    false
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
    use super::rules;
    use std::time::Duration;
    use windows::core::{w, BOOL, PCWSTR, PWSTR};
    use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM};
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
        found: bool,
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
        if !rules::is_meet_title(&title) {
            return BOOL(1);
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if exe_of_process(pid).as_deref() == Some(search.exe.as_str()) {
            search.found = true;
            return BOOL(0);
        }
        BOOL(1)
    }

    /// Whether a window of this browser has a Google Meet call in front.
    pub fn browser_shows_meet(exe: &str) -> bool {
        let mut search = Search {
            exe: exe.to_string(),
            found: false,
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
