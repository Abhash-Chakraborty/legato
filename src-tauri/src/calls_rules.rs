//! Rules for noticing calls: which app holds the microphone, from the
//! entries Windows keeps about microphone access, whether a browser window
//! is a Google Meet call, and when a call starts and ends. Free of Windows
//! calls, so it can be tested anywhere.

use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::{HashMap, HashSet};

/// An app Legato watches for calls.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum CallApp {
    Teams,
    Zoom,
    Meet,
    Slack,
    Discord,
    #[serde(rename = "whatsapp")]
    WhatsApp,
    Telegram,
}

impl CallApp {
    pub const ALL: [CallApp; 7] = [
        CallApp::Teams,
        CallApp::Zoom,
        CallApp::Meet,
        CallApp::Slack,
        CallApp::Discord,
        CallApp::WhatsApp,
        CallApp::Telegram,
    ];

    /// The name the call card says.
    pub fn name(self) -> &'static str {
        match self {
            CallApp::Teams => "Microsoft Teams",
            CallApp::Zoom => "Zoom",
            CallApp::Meet => "Google Meet",
            CallApp::Slack => "Slack",
            CallApp::Discord => "Discord",
            CallApp::WhatsApp => "WhatsApp",
            CallApp::Telegram => "Telegram",
        }
    }

    /// How long the app has to hold the microphone before it counts as a
    /// call. WhatsApp and Telegram also take it to record a voice message,
    /// which rarely runs this long.
    pub fn start_after_ms(self) -> u64 {
        match self {
            CallApp::WhatsApp | CallApp::Telegram => MESSAGING_START_AFTER_MS,
            _ => START_AFTER_MS,
        }
    }

    /// Discord holds the microphone for every voice channel, so it starts
    /// unwatched.
    pub fn watched_by_default(self) -> bool {
        self != CallApp::Discord
    }
}

/// The apps settings saved before WhatsApp and Telegram were added knew
/// about.
pub fn first_apps() -> Vec<CallApp> {
    CallApp::ALL[..5].to_vec()
}

/// Brings in apps Legato learned to watch since these settings were saved:
/// each one is watched when it is by default, and is known from then on, so
/// turning it off sticks. True when anything changed.
pub fn add_new_apps(watched: &mut Vec<CallApp>, known: &mut Vec<CallApp>) -> bool {
    let mut changed = false;
    for app in CallApp::ALL {
        if known.contains(&app) {
            continue;
        }
        known.push(app);
        if app.watched_by_default() && !watched.contains(&app) {
            watched.push(app);
        }
        changed = true;
    }
    changed
}

/// The apps watched when nothing was chosen yet.
pub fn default_watched() -> Vec<CallApp> {
    CallApp::ALL
        .into_iter()
        .filter(|a| a.watched_by_default())
        .collect()
}

/// Who holds the microphone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Holder {
    App(CallApp),
    /// A browser; a call when one of its windows shows Google Meet, Teams
    /// or Zoom.
    Browser(String),
    Legato,
    Other,
}

const BROWSERS: [&str; 13] = [
    "chrome.exe",
    "msedge.exe",
    "brave.exe",
    "firefox.exe",
    "opera.exe",
    "vivaldi.exe",
    "arc.exe",
    "zen.exe",
    "floorp.exe",
    "librewolf.exe",
    "waterfox.exe",
    "chromium.exe",
    "thorium.exe",
];

/// The file name of a desktop app's entry, lower-case: Windows writes the
/// path with "#" for "\", as in "C:#Program Files#Zoom#bin#Zoom.exe".
pub fn exe_of(path: &str) -> String {
    path.rsplit(['#', '\\', '/'])
        .next()
        .unwrap_or(path)
        .to_lowercase()
}

/// The app an executable belongs to.
pub fn holder_for_exe(exe: &str) -> Holder {
    let exe = exe_of(exe);
    match exe.as_str() {
        "ms-teams.exe" | "teams.exe" => Holder::App(CallApp::Teams),
        "zoom.exe" => Holder::App(CallApp::Zoom),
        "slack.exe" => Holder::App(CallApp::Slack),
        "discord.exe" => Holder::App(CallApp::Discord),
        "whatsapp.exe" | "whatsapp.root.exe" => Holder::App(CallApp::WhatsApp),
        "telegram.exe" => Holder::App(CallApp::Telegram),
        "legato.exe" => Holder::Legato,
        e if BROWSERS.contains(&e) => Holder::Browser(exe),
        _ => Holder::Other,
    }
}

/// Whether the program at `exe` carries `call`: the browser the call was
/// found in when it's in a tab, else the call app itself. The desktop app
/// idling in the background doesn't stand in for a call in the browser, nor
/// the browser for a call in the app.
pub fn carries_call(exe: &str, call: CallApp, browser: Option<&(String, CallApp)>) -> bool {
    match (holder_for_exe(exe), browser.filter(|(_, c)| *c == call)) {
        (Holder::Browser(b), Some((tab_in, _))) => b == *tab_in,
        (Holder::App(a), None) => a == call,
        _ => false,
    }
}

/// Who an entry is: a packaged app by its family name, such as
/// "MSTeams_8wekyb3d8bbwe", or a desktop app by its path.
pub fn holder_for_entry(key: &str, packaged: bool) -> Holder {
    if !packaged {
        return holder_for_exe(key);
    }
    let family = key.to_lowercase();
    if family.starts_with("msteams_") || family.starts_with("microsoftteams_") {
        Holder::App(CallApp::Teams)
    } else if family.contains(".slack_") {
        Holder::App(CallApp::Slack)
    } else if family.contains("discord") {
        Holder::App(CallApp::Discord)
    } else if family.contains("zoom") {
        Holder::App(CallApp::Zoom)
    } else if family.contains("whatsappdesktop") {
        Holder::App(CallApp::WhatsApp)
    } else if family.contains("telegramdesktop") {
        Holder::App(CallApp::Telegram)
    } else {
        Holder::Other
    }
}

/// Adds the apps recording from a microphone right now, by executable, to
/// the entries Windows' privacy records show in use. Those records can lag
/// behind a call, so either is enough; an app both name is listed once.
pub fn merge_users(mut users: Vec<(String, bool)>, capturing: &[String]) -> Vec<(String, bool)> {
    for exe in capturing {
        let holder = holder_for_exe(exe);
        let known = users.iter().any(|(entry, packaged)| {
            let seen = holder_for_entry(entry, *packaged);
            match holder {
                Holder::App(_) => seen == holder,
                _ => !*packaged && exe_of(entry) == exe_of(exe),
            }
        });
        if !known {
            users.push((exe_of(exe), false));
        }
    }
    users
}

/// Windows keeps when each app last started and stopped using the
/// microphone; it is in use when it started after it last stopped.
pub fn in_use(started: u64, stopped: u64) -> bool {
    started != 0 && started > stopped
}

/// Whether a browser window's title is a Google Meet call, such as
/// "Meet - abc-defg-hij - Google Chrome" or the Meet app's "Google Meet".
pub fn is_meet_title(title: &str) -> bool {
    let t = title.trim().to_lowercase();
    t.starts_with("meet - ")
        || t.starts_with("meet – ")
        || t.starts_with("meet: ")
        || t.contains("google meet")
        || t.contains("meet.google.com")
}

/// The call a browser window's title shows: a Google Meet, Microsoft Teams
/// or Zoom tab in front.
pub fn web_call(title: &str) -> Option<CallApp> {
    let t = title.trim().to_lowercase();
    if is_meet_title(&t) {
        Some(CallApp::Meet)
    } else if t.contains("microsoft teams")
        || t.contains("teams.microsoft.com")
        || t.contains("teams.live.com")
    {
        Some(CallApp::Teams)
    } else if t.starts_with("zoom - ")
        || t.starts_with("zoom – ")
        || t.contains("zoom meeting")
        || t.contains("zoom workplace")
        || t.contains("app.zoom.us")
    {
        Some(CallApp::Zoom)
    } else {
        None
    }
}

/// A call has to hold the microphone this long before it counts, so a
/// quick device check doesn't bring up the card.
pub const START_AFTER_MS: u64 = 2000;

/// The same for apps that also record voice messages.
pub const MESSAGING_START_AFTER_MS: u64 = 20_000;

/// After the call app lets go of the microphone, notes stop this much
/// later, unless it takes the microphone again.
pub const END_GRACE_MS: u64 = 20_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallEvent {
    Started(CallApp),
    Ended(CallApp),
}

/// Turns "who holds the microphone now", sampled over time, into calls
/// starting and ending.
#[derive(Default)]
pub struct Tracker {
    first_seen: HashMap<CallApp, u64>,
    live: HashSet<CallApp>,
}

impl Tracker {
    pub fn update(&mut self, now_ms: u64, holding: &[CallApp]) -> Vec<CallEvent> {
        let mut events = Vec::new();
        for app in CallApp::ALL {
            if holding.contains(&app) {
                let since = *self.first_seen.entry(app).or_insert(now_ms);
                if !self.live.contains(&app) && now_ms.saturating_sub(since) >= app.start_after_ms()
                {
                    self.live.insert(app);
                    events.push(CallEvent::Started(app));
                }
            } else {
                self.first_seen.remove(&app);
                if self.live.remove(&app) {
                    events.push(CallEvent::Ended(app));
                }
            }
        }
        events
    }

    /// Whether a call in this app is going on now.
    pub fn is_live(&self, app: CallApp) -> bool {
        self.live.contains(&app)
    }

    /// The calls going on now.
    #[cfg(test)]
    pub fn live(&self) -> Vec<CallApp> {
        CallApp::ALL
            .into_iter()
            .filter(|a| self.live.contains(a))
            .collect()
    }
}

/// When to offer notes again after Not now: `minutes` later, or never
/// during this call when it is 0.
pub fn offer_again_at(now_ms: u64, minutes: u32) -> Option<u64> {
    (minutes > 0).then(|| now_ms + minutes as u64 * 60_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_map_to_apps() {
        assert_eq!(
            holder_for_entry("C:#Program Files#Zoom#bin#Zoom.exe", false),
            Holder::App(CallApp::Zoom)
        );
        assert_eq!(
            holder_for_entry("MSTeams_8wekyb3d8bbwe", true),
            Holder::App(CallApp::Teams)
        );
        assert_eq!(
            holder_for_entry("91750D7E.Slack_8she8kybcnzg4", true),
            Holder::App(CallApp::Slack)
        );
        assert_eq!(
            holder_for_entry(
                "C:#Program Files (x86)#Microsoft#Edge#Application#msedge.exe",
                false
            ),
            Holder::Browser("msedge.exe".into())
        );
        assert_eq!(
            holder_for_entry("C:#Program Files#Legato#legato.exe", false),
            Holder::Legato
        );
        // Sound Recorder records, but it isn't a call.
        assert_eq!(
            holder_for_entry("Microsoft.WindowsSoundRecorder_8wekyb3d8bbwe", true),
            Holder::Other
        );
    }

    #[test]
    fn the_call_is_recorded_where_it_is() {
        let teams = "C:\\Program Files\\WindowsApps\\MSTeams\\ms-teams.exe";
        let edge = "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe";
        let in_edge = ("msedge.exe".to_string(), CallApp::Teams);
        // A Teams call in an Edge tab: Edge's sound, not the idle app's.
        assert!(carries_call(edge, CallApp::Teams, Some(&in_edge)));
        assert!(!carries_call(teams, CallApp::Teams, Some(&in_edge)));
        // In the app: the app, not a browser playing something else.
        assert!(carries_call(teams, CallApp::Teams, None));
        assert!(!carries_call(edge, CallApp::Teams, None));
        // A tab with another call doesn't count for this one.
        assert!(carries_call(
            teams,
            CallApp::Teams,
            Some(&("msedge.exe".into(), CallApp::Zoom))
        ));
        assert!(!carries_call("chrome.exe", CallApp::Teams, Some(&in_edge)));
    }

    #[test]
    fn meet_titles() {
        assert!(is_meet_title("Meet - abc-defg-hij - Google Chrome"));
        assert!(is_meet_title(
            "Meet – abc-defg-hij and 2 more pages - Microsoft Edge"
        ));
        assert!(is_meet_title("Google Meet"));
        assert!(!is_meet_title(
            "Meeting notes - Google Docs - Google Chrome"
        ));
        assert!(!is_meet_title("YouTube - Google Chrome"));
    }

    #[test]
    fn web_calls_by_tab_title_in_any_browser() {
        assert_eq!(
            web_call("Meet - abc-defg-hij — Zen Browser"),
            Some(CallApp::Meet)
        );
        assert_eq!(
            web_call("Meeting with Sam | Microsoft Teams - Google Chrome"),
            Some(CallApp::Teams)
        );
        assert_eq!(web_call("Zoom - Microsoft Edge"), Some(CallApp::Zoom));
        assert_eq!(
            web_call("Zoom Meeting — Mozilla Firefox"),
            Some(CallApp::Zoom)
        );
        assert_eq!(web_call("Zooming out on maps - Google Chrome"), None);
        assert_eq!(web_call("YouTube — Zen Browser"), None);
        assert_eq!(
            holder_for_exe("C:#Program Files#Zen Browser#zen.exe"),
            Holder::Browser("zen.exe".into())
        );
    }

    #[test]
    fn merge_users_adds_capturing_apps_once() {
        let registry = vec![
            ("MSTeams_8wekyb3d8bbwe".to_string(), true),
            (
                "C:#Program Files#Google#Chrome#Application#chrome.exe".to_string(),
                false,
            ),
        ];
        let capturing = vec![
            "ms-teams.exe".to_string(),
            "chrome.exe".to_string(),
            "WhatsApp.Root.exe".to_string(),
            "whatsapp.root.exe".to_string(),
        ];
        let merged = merge_users(registry, &capturing);
        assert_eq!(merged.len(), 3);
        assert_eq!(merged[2], ("whatsapp.root.exe".to_string(), false));
        assert_eq!(
            merge_users(Vec::new(), &["Zoom.exe".to_string()]),
            vec![("zoom.exe".to_string(), false)]
        );
    }

    #[test]
    fn in_use_compares_times() {
        assert!(in_use(200, 0));
        assert!(in_use(200, 100));
        assert!(!in_use(100, 200));
        assert!(!in_use(0, 0));
    }

    #[test]
    fn a_call_starts_after_two_seconds_and_ends_when_released() {
        let mut t = Tracker::default();
        assert!(t.update(0, &[CallApp::Teams]).is_empty());
        assert!(t.update(1500, &[CallApp::Teams]).is_empty());
        assert_eq!(
            t.update(2000, &[CallApp::Teams]),
            vec![CallEvent::Started(CallApp::Teams)]
        );
        assert!(t.update(4000, &[CallApp::Teams]).is_empty());
        assert_eq!(t.live(), vec![CallApp::Teams]);
        assert_eq!(t.update(6000, &[]), vec![CallEvent::Ended(CallApp::Teams)]);
        assert!(t.live().is_empty());
    }

    #[test]
    fn a_quick_grab_is_not_a_call() {
        let mut t = Tracker::default();
        t.update(0, &[CallApp::Zoom]);
        assert!(t.update(1000, &[]).is_empty());
        // Holding it again starts the wait over.
        assert!(t.update(1500, &[CallApp::Zoom]).is_empty());
        assert!(t.update(3000, &[CallApp::Zoom]).is_empty());
        assert_eq!(
            t.update(3500, &[CallApp::Zoom]),
            vec![CallEvent::Started(CallApp::Zoom)]
        );
    }

    #[test]
    fn snooze() {
        assert_eq!(offer_again_at(1000, 2), Some(121_000));
        assert_eq!(offer_again_at(1000, 0), None);
    }

    #[test]
    fn discord_starts_unwatched() {
        assert_eq!(
            default_watched(),
            vec![
                CallApp::Teams,
                CallApp::Zoom,
                CallApp::Meet,
                CallApp::Slack,
                CallApp::WhatsApp,
                CallApp::Telegram
            ]
        );
    }

    #[test]
    fn messaging_apps_map_and_wait_longer() {
        assert_eq!(
            holder_for_entry("5319275A.WhatsAppDesktop_cv1g1gvanyjgm", true),
            Holder::App(CallApp::WhatsApp)
        );
        assert_eq!(
            holder_for_entry("TelegramMessengerLLP.TelegramDesktop_t4vj0pshhgkwm", true),
            Holder::App(CallApp::Telegram)
        );
        assert_eq!(
            holder_for_entry(
                "C:#Users#me#AppData#Roaming#Telegram Desktop#Telegram.exe",
                false
            ),
            Holder::App(CallApp::Telegram)
        );
        // A voice message isn't a call.
        let mut t = Tracker::default();
        t.update(0, &[CallApp::WhatsApp]);
        assert!(t.update(15_000, &[CallApp::WhatsApp]).is_empty());
        assert!(t.update(16_000, &[]).is_empty());
        t.update(20_000, &[CallApp::WhatsApp]);
        assert_eq!(
            t.update(40_000, &[CallApp::WhatsApp]),
            vec![CallEvent::Started(CallApp::WhatsApp)]
        );
    }

    #[test]
    fn new_apps_join_old_settings_once() {
        let mut watched = vec![CallApp::Teams, CallApp::Meet];
        let mut known = first_apps();
        assert!(add_new_apps(&mut watched, &mut known));
        assert_eq!(
            watched,
            vec![
                CallApp::Teams,
                CallApp::Meet,
                CallApp::WhatsApp,
                CallApp::Telegram
            ]
        );
        // Turned off afterwards, it stays off.
        watched.retain(|a| *a != CallApp::Telegram);
        assert!(!add_new_apps(&mut watched, &mut known));
        assert!(!watched.contains(&CallApp::Telegram));
    }
}
