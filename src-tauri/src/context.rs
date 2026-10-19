//! Knowing the app in front.
//!
//! The foreground window is read when a dictation starts (the app the text is
//! meant for) and again right before paste. Reading it is a handful of Win32
//! calls and costs well under a millisecond, so it can sit on the start path;
//! the slower lookups (the app's display name, its category) happen later,
//! when the history row is written.

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::sync::{Mutex, RwLock};

/// The window in front, as read from Windows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForegroundWindow {
    /// Full path of the process's executable.
    pub exe_path: String,
    pub title: String,
}

/// Kinds of app, used to group them on Style and Insights.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum AppCategory {
    Personal,
    Work,
    Email,
    Documents,
    Ai,
    Code,
    Other,
}

impl AppCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            AppCategory::Personal => "personal",
            AppCategory::Work => "work",
            AppCategory::Email => "email",
            AppCategory::Documents => "documents",
            AppCategory::Ai => "ai",
            AppCategory::Code => "code",
            AppCategory::Other => "other",
        }
    }
}

/// What an app rule matches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum AppRuleKind {
    /// An app, by its name ("Claude Code") or executable ("slack.exe").
    App,
    /// A website in a browser, by text in its tab title ("Gmail").
    Site,
}

/// A move made on Style › Edit apps: this app or site belongs to that kind
/// of app. Rules win over the built-in lists.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct AppRule {
    pub kind: AppRuleKind,
    /// Lower case: an executable, an app name or text in a tab title.
    pub pattern: String,
    /// How the Style page names it.
    pub name: String,
    pub category: AppCategory,
}

static RULES: Lazy<RwLock<Vec<AppRule>>> = Lazy::new(Default::default);

/// Replaces the rules `resolve` applies; called at startup and whenever
/// Edit apps saves.
pub fn set_rules(rules: Vec<AppRule>) {
    match RULES.write() {
        Ok(mut slot) => *slot = rules,
        Err(e) => *e.into_inner() = rules,
    }
}

fn apply_rules(info: &mut AppInfo, rules: &[AppRule]) {
    if BROWSERS.contains(&info.exe.as_str()) {
        let title = info.title.to_lowercase();
        if let Some(rule) = rules.iter().find(|r| {
            r.kind == AppRuleKind::Site && !r.pattern.is_empty() && title.contains(&r.pattern)
        }) {
            info.site = Some(rule.name.clone());
            info.category = rule.category;
            return;
        }
        if info.site.is_some() {
            // A built-in site keeps its kind unless a rule names it.
            if let Some(rule) = rules.iter().find(|r| {
                r.kind == AppRuleKind::Site
                    && info
                        .site
                        .as_deref()
                        .is_some_and(|s| s.eq_ignore_ascii_case(&r.pattern))
            }) {
                info.category = rule.category;
            }
            return;
        }
    }
    let name = info.name.to_lowercase();
    if let Some(rule) = rules
        .iter()
        .filter(|r| r.kind == AppRuleKind::App)
        .find(|r| r.pattern == name)
        .or_else(|| {
            rules
                .iter()
                .filter(|r| r.kind == AppRuleKind::App)
                .find(|r| r.pattern == info.exe)
        })
    {
        info.category = rule.category;
    }
}

/// The kind of app a rule's target has without any rules.
pub fn builtin_category(rule: &AppRule) -> AppCategory {
    match rule.kind {
        AppRuleKind::Site => SITES
            .iter()
            .find(|(s, _)| s.eq_ignore_ascii_case(&rule.pattern))
            .map(|(_, c)| *c)
            .unwrap_or(AppCategory::Other),
        AppRuleKind::App if rule.pattern == "claude code" => AppCategory::Ai,
        AppRuleKind::App => APPS
            .iter()
            .find(|(e, _)| *e == rule.pattern)
            .map(|(_, c)| *c)
            .unwrap_or(AppCategory::Other),
    }
}

/// The rule that moves this app or site, for Edit apps.
pub fn rule_for(info: &AppInfo, category: AppCategory) -> AppRule {
    match &info.site {
        Some(site) => AppRule {
            kind: AppRuleKind::Site,
            pattern: site.to_lowercase(),
            name: site.clone(),
            category,
        },
        None if info.name == "Claude Code" => AppRule {
            kind: AppRuleKind::App,
            pattern: "claude code".into(),
            name: info.name.clone(),
            category,
        },
        None => AppRule {
            kind: AppRuleKind::App,
            pattern: info.exe.clone(),
            name: info.name.clone(),
            category,
        },
    }
}

/// A foreground window resolved into what the history row stores.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppInfo {
    pub exe_path: String,
    /// Executable file name in lower case, such as `chrome.exe`.
    pub exe: String,
    /// What people call the app: "Chrome", "Outlook", "Claude Code".
    pub name: String,
    pub title: String,
    /// The site in a browser tab, matched from the window title ("Gmail").
    pub site: Option<String>,
    pub category: AppCategory,
}

/// The window in front when the current dictation started. Set by the
/// transcribe action's start and taken by its stop.
#[derive(Default)]
pub struct DictationContext(Mutex<Option<ForegroundWindow>>);

impl DictationContext {
    pub fn set(&self, window: Option<ForegroundWindow>) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = window;
    }

    pub fn take(&self) -> Option<ForegroundWindow> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).take()
    }
}

/// Reads the window in front. None when there is no foreground window or its
/// process can't be opened.
pub fn foreground_window() -> Option<ForegroundWindow> {
    #[cfg(windows)]
    {
        win::foreground_window()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// The app a process belongs to, for naming the app whose sound a meeting
/// records.
pub fn app_for_process(pid: u32) -> Option<AppInfo> {
    #[cfg(windows)]
    {
        // SAFETY: opens the process for a limited query and closes it.
        let exe_path = unsafe { win::process_path(pid) }?;
        Some(resolve(&ForegroundWindow {
            exe_path,
            title: String::new(),
        }))
    }
    #[cfg(not(windows))]
    {
        let _ = pid;
        None
    }
}

/// The apps' windows someone could switch to, front to back, with handles
/// `focus_window` can bring back. Legato's own windows are left out.
pub fn open_windows() -> Vec<(isize, ForegroundWindow)> {
    #[cfg(windows)]
    {
        win::open_windows()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

/// The window in front, as a handle `focus_window` can bring back.
pub fn foreground_handle() -> Option<isize> {
    #[cfg(windows)]
    {
        win::foreground_handle()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Brings a window back to the front. False when it's gone or Windows
/// refused.
pub fn focus_window(handle: isize) -> bool {
    #[cfg(windows)]
    {
        win::focus_window(handle)
    }
    #[cfg(not(windows))]
    {
        let _ = handle;
        false
    }
}

/// Whether the window in front runs at a higher integrity level than
/// Legato, as an app started "as administrator" does. Windows drops input
/// sent to it, so a paste would silently fail.
pub fn foreground_is_elevated() -> bool {
    #[cfg(windows)]
    {
        win::foreground_is_elevated()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Resolves a window into its app's name, site and category.
pub fn resolve(window: &ForegroundWindow) -> AppInfo {
    let exe = window
        .exe_path
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(&window.exe_path)
        .to_ascii_lowercase();
    let described = cached_description(&window.exe_path);
    let mut info = classify(&window.exe_path, &exe, described.as_deref(), &window.title);
    let rules = RULES
        .read()
        .map(|r| r.clone())
        .unwrap_or_else(|e| e.into_inner().clone());
    apply_rules(&mut info, &rules);
    info
}

/// The app's icon as a PNG, for the history list.
pub fn app_icon_png(exe_path: &str) -> Option<Vec<u8>> {
    #[cfg(windows)]
    {
        win::app_icon_png(exe_path)
    }
    #[cfg(not(windows))]
    {
        let _ = exe_path;
        None
    }
}

fn cached_description(exe_path: &str) -> Option<String> {
    static CACHE: Mutex<Option<HashMap<String, Option<String>>>> = Mutex::new(None);
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let cache = guard.get_or_insert_with(HashMap::new);
    if let Some(hit) = cache.get(exe_path) {
        return hit.clone();
    }
    #[cfg(windows)]
    let described = win::file_description(exe_path);
    #[cfg(not(windows))]
    let described = None;
    cache.insert(exe_path.to_string(), described.clone());
    described
}

const BROWSERS: &[&str] = &[
    "chrome.exe",
    "msedge.exe",
    "firefox.exe",
    "brave.exe",
    "opera.exe",
    "vivaldi.exe",
    "arc.exe",
    "zen.exe",
];

/// Sites recognised in a browser tab's title, by the name the site puts there.
const SITES: &[(&str, AppCategory)] = &[
    ("Gmail", AppCategory::Email),
    ("Outlook", AppCategory::Email),
    ("WhatsApp", AppCategory::Personal),
    ("Telegram", AppCategory::Personal),
    ("Discord", AppCategory::Personal),
    ("Instagram", AppCategory::Personal),
    ("Slack", AppCategory::Work),
    ("Microsoft Teams", AppCategory::Work),
    ("LinkedIn", AppCategory::Work),
    ("Claude", AppCategory::Ai),
    ("ChatGPT", AppCategory::Ai),
    ("Gemini", AppCategory::Ai),
    ("Perplexity", AppCategory::Ai),
    ("GitHub", AppCategory::Code),
    ("Notion", AppCategory::Documents),
    ("Google Docs", AppCategory::Documents),
];

/// Desktop apps by executable.
const APPS: &[(&str, AppCategory)] = &[
    ("whatsapp.exe", AppCategory::Personal),
    ("whatsapp.root.exe", AppCategory::Personal),
    ("telegram.exe", AppCategory::Personal),
    ("discord.exe", AppCategory::Personal),
    ("signal.exe", AppCategory::Personal),
    ("slack.exe", AppCategory::Work),
    ("ms-teams.exe", AppCategory::Work),
    ("teams.exe", AppCategory::Work),
    ("outlook.exe", AppCategory::Email),
    ("olk.exe", AppCategory::Email),
    ("thunderbird.exe", AppCategory::Email),
    ("hxoutlook.exe", AppCategory::Email),
    ("winword.exe", AppCategory::Documents),
    ("onenote.exe", AppCategory::Documents),
    ("notion.exe", AppCategory::Documents),
    ("obsidian.exe", AppCategory::Documents),
    ("claude.exe", AppCategory::Ai),
    ("chatgpt.exe", AppCategory::Ai),
    ("code.exe", AppCategory::Code),
    ("cursor.exe", AppCategory::Code),
    ("windsurf.exe", AppCategory::Code),
    ("zed.exe", AppCategory::Code),
    ("devenv.exe", AppCategory::Code),
    ("idea64.exe", AppCategory::Code),
    ("windowsterminal.exe", AppCategory::Code),
    ("wezterm-gui.exe", AppCategory::Code),
    ("alacritty.exe", AppCategory::Code),
    ("powershell.exe", AppCategory::Code),
    ("pwsh.exe", AppCategory::Code),
    ("cmd.exe", AppCategory::Code),
];

const TERMINALS: &[&str] = &[
    "windowsterminal.exe",
    "wezterm-gui.exe",
    "alacritty.exe",
    "powershell.exe",
    "pwsh.exe",
    "cmd.exe",
    "code.exe",
    "cursor.exe",
];

/// Splits a window title into its parts ("Inbox - me@x.com - Gmail - Google
/// Chrome"), dropping unread counters such as "(3) ".
fn title_parts(title: &str) -> Vec<String> {
    let mut parts = vec![title.to_string()];
    for sep in [" - ", " – ", " — ", " | ", " · "] {
        parts = parts
            .iter()
            .flat_map(|p| p.split(sep).map(str::to_string).collect::<Vec<_>>())
            .collect();
    }
    parts
        .into_iter()
        .map(|p| {
            let p = p.trim();
            match p.strip_prefix('(').and_then(|r| r.split_once(") ")) {
                Some((count, rest)) if count.chars().all(|c| c.is_ascii_digit()) => {
                    rest.trim().to_string()
                }
                _ => p.to_string(),
            }
        })
        .filter(|p| !p.is_empty())
        .collect()
}

/// "Google Chrome" reads as "Chrome", "Microsoft Outlook" as "Outlook".
fn short_name(described: &str) -> String {
    for prefix in ["Google ", "Microsoft "] {
        if let Some(rest) = described.strip_prefix(prefix) {
            if !rest.trim().is_empty() {
                return rest.trim().to_string();
            }
        }
    }
    described.trim().to_string()
}

fn classify(exe_path: &str, exe: &str, described: Option<&str>, title: &str) -> AppInfo {
    let stem = exe.strip_suffix(".exe").unwrap_or(exe);
    let mut name = described
        .map(short_name)
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| {
            let mut chars = stem.chars();
            chars
                .next()
                .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        });
    let mut site = None;
    let mut category = APPS
        .iter()
        .find(|(e, _)| *e == exe)
        .map(|(_, c)| *c)
        .unwrap_or(AppCategory::Other);

    if BROWSERS.contains(&exe) {
        let parts = title_parts(title);
        if let Some((found, cat)) = SITES
            .iter()
            .find(|(s, _)| parts.iter().any(|p| p.eq_ignore_ascii_case(s)))
        {
            site = Some(found.to_string());
            category = *cat;
        }
    }

    // Claude Code runs in a terminal and puts its name in the title.
    if TERMINALS.contains(&exe) && title.to_lowercase().contains("claude code") {
        name = "Claude Code".to_string();
        category = AppCategory::Ai;
    }

    AppInfo {
        exe_path: exe_path.to_string(),
        exe: exe.to_string(),
        name,
        title: title.to_string(),
        site,
        category,
    }
}

#[cfg(windows)]
mod win {
    use super::ForegroundWindow;
    use std::ffi::c_void;
    use windows::core::{w, BOOL, HSTRING, PWSTR};
    use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM};
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP,
    };
    use windows::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW, FILE_FLAGS_AND_ATTRIBUTES,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
    use windows::Win32::UI::WindowsAndMessaging::{
        DestroyIcon, EnumChildWindows, GetForegroundWindow, GetIconInfo, GetWindowTextLengthW,
        GetWindowTextW, GetWindowThreadProcessId, HICON, ICONINFO,
    };

    pub fn foreground_handle() -> Option<isize> {
        let hwnd = unsafe { GetForegroundWindow() };
        (!hwnd.0.is_null()).then_some(hwnd.0 as isize)
    }

    pub fn focus_window(handle: isize) -> bool {
        use windows::Win32::UI::WindowsAndMessaging::{
            IsIconic, IsWindow, SetForegroundWindow, ShowWindow, SW_RESTORE,
        };
        let hwnd = HWND(handle as *mut c_void);
        unsafe {
            if !IsWindow(Some(hwnd)).as_bool() {
                return false;
            }
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            SetForegroundWindow(hwnd).as_bool()
        }
    }

    /// The integrity level's RID (0x2000 medium, 0x3000 high), or the error
    /// opening the token.
    unsafe fn integrity_of(
        process: windows::Win32::Foundation::HANDLE,
    ) -> windows::core::Result<Option<u32>> {
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::Security::{
            GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, TokenIntegrityLevel,
            TOKEN_MANDATORY_LABEL, TOKEN_QUERY,
        };
        use windows::Win32::System::Threading::OpenProcessToken;
        let mut token = HANDLE::default();
        OpenProcessToken(process, TOKEN_QUERY, &mut token)?;
        let mut size = 0u32;
        let _ = GetTokenInformation(token, TokenIntegrityLevel, None, 0, &mut size);
        if size == 0 {
            let _ = CloseHandle(token);
            return Ok(None);
        }
        let mut buf = vec![0u8; size as usize];
        let read = GetTokenInformation(
            token,
            TokenIntegrityLevel,
            Some(buf.as_mut_ptr() as *mut c_void),
            size,
            &mut size,
        );
        let _ = CloseHandle(token);
        if read.is_err() {
            return Ok(None);
        }
        let label = &*(buf.as_ptr() as *const TOKEN_MANDATORY_LABEL);
        let sid = label.Label.Sid;
        let count = *GetSidSubAuthorityCount(sid);
        if count == 0 {
            return Ok(None);
        }
        Ok(Some(*GetSidSubAuthority(sid, u32::from(count) - 1)))
    }

    pub fn foreground_is_elevated() -> bool {
        use windows::Win32::Foundation::E_ACCESSDENIED;
        use windows::Win32::System::Threading::GetCurrentProcess;
        const HIGH: u32 = 0x3000;
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() {
                return false;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == 0 {
                return false;
            }
            let own = integrity_of(GetCurrentProcess())
                .ok()
                .flatten()
                .unwrap_or(0x2000);
            if own >= HIGH {
                return false;
            }
            let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
                return false;
            };
            let level = integrity_of(process);
            let _ = CloseHandle(process);
            match level {
                Ok(Some(level)) => level > own,
                Ok(None) => false,
                // An elevated process's token can't be read from below.
                Err(e) => e.code() == E_ACCESSDENIED,
            }
        }
    }

    pub fn foreground_window() -> Option<ForegroundWindow> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() {
                return None;
            }
            window_of(hwnd)
        }
    }

    /// A window's title and its app's executable.
    unsafe fn window_of(hwnd: HWND) -> Option<ForegroundWindow> {
        let title = window_title(hwnd);
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let mut exe_path = process_path(pid)?;
        // Store apps draw inside ApplicationFrameHost; the app itself owns
        // one of the frame's child windows.
        if exe_path
            .to_ascii_lowercase()
            .ends_with("\\applicationframehost.exe")
        {
            if let Some(path) = store_app_child(hwnd, pid).and_then(|child| process_path(child)) {
                exe_path = path;
            }
        }
        Some(ForegroundWindow { exe_path, title })
    }

    unsafe extern "system" fn collect_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
        use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
        use windows::Win32::UI::WindowsAndMessaging::{
            GetWindow, GetWindowLongW, IsWindowVisible, GWL_EXSTYLE, GW_OWNER, WS_EX_TOOLWINDOW,
        };
        let found = &mut *(lparam.0 as *mut Vec<HWND>);
        let owned = GetWindow(hwnd, GW_OWNER).is_ok_and(|owner| !owner.0.is_null());
        let tool = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0 != 0;
        // Windows on another desktop, and suspended Store apps, are cloaked.
        let mut cloaked = 0u32;
        let _ = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut u32 as *mut c_void,
            std::mem::size_of::<u32>() as u32,
        );
        if IsWindowVisible(hwnd).as_bool()
            && !owned
            && !tool
            && cloaked == 0
            && GetWindowTextLengthW(hwnd) > 0
        {
            found.push(hwnd);
        }
        BOOL(1)
    }

    pub fn open_windows() -> Vec<(isize, ForegroundWindow)> {
        use windows::Win32::UI::WindowsAndMessaging::EnumWindows;
        let mut found: Vec<HWND> = Vec::new();
        let own = std::process::id();
        unsafe {
            let _ = EnumWindows(
                Some(collect_window),
                LPARAM(&mut found as *mut Vec<HWND> as isize),
            );
            found
                .into_iter()
                .filter(|hwnd| {
                    let mut pid = 0u32;
                    GetWindowThreadProcessId(*hwnd, Some(&mut pid));
                    pid != own
                })
                .filter_map(|hwnd| window_of(hwnd).map(|w| (hwnd.0 as isize, w)))
                .collect()
        }
    }

    unsafe fn window_title(hwnd: HWND) -> String {
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; len as usize + 1];
        let copied = GetWindowTextW(hwnd, &mut buf);
        String::from_utf16_lossy(&buf[..copied.max(0) as usize])
    }

    pub(super) unsafe fn process_path(pid: u32) -> Option<String> {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = vec![0u16; 1024];
        let mut size = buf.len() as u32;
        let result = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(process);
        result.ok()?;
        Some(String::from_utf16_lossy(&buf[..size as usize]))
    }

    struct ChildSearch {
        frame_pid: u32,
        found: Option<u32>,
    }

    unsafe extern "system" fn find_child(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let search = &mut *(lparam.0 as *mut ChildSearch);
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid != 0 && pid != search.frame_pid {
            search.found = Some(pid);
            return BOOL(0);
        }
        BOOL(1)
    }

    unsafe fn store_app_child(frame: HWND, frame_pid: u32) -> Option<u32> {
        let mut search = ChildSearch {
            frame_pid,
            found: None,
        };
        let _ = EnumChildWindows(
            Some(frame),
            Some(find_child),
            LPARAM(&mut search as *mut ChildSearch as isize),
        );
        search.found
    }

    /// The FileDescription from the executable's version resource, which is
    /// the name the app gives itself ("Google Chrome", "Slack").
    pub fn file_description(exe_path: &str) -> Option<String> {
        unsafe {
            let path = HSTRING::from(exe_path);
            let size = GetFileVersionInfoSizeW(&path, None);
            if size == 0 {
                return None;
            }
            let mut data = vec![0u8; size as usize];
            GetFileVersionInfoW(&path, None, size, data.as_mut_ptr() as *mut c_void).ok()?;

            let mut ptr: *mut c_void = std::ptr::null_mut();
            let mut len = 0u32;
            if !VerQueryValueW(
                data.as_ptr() as *const c_void,
                w!("\\VarFileInfo\\Translation"),
                &mut ptr,
                &mut len,
            )
            .as_bool()
                || len < 4
            {
                return None;
            }
            let lang = *(ptr as *const u16);
            let codepage = *(ptr as *const u16).add(1);
            let key = HSTRING::from(format!(
                "\\StringFileInfo\\{lang:04x}{codepage:04x}\\FileDescription"
            ));
            if !VerQueryValueW(data.as_ptr() as *const c_void, &key, &mut ptr, &mut len).as_bool()
                || len == 0
            {
                return None;
            }
            let text = std::slice::from_raw_parts(ptr as *const u16, len as usize);
            let text = String::from_utf16_lossy(text);
            let text = text.trim_end_matches('\0').trim();
            (!text.is_empty()).then(|| text.to_string())
        }
    }

    pub fn app_icon_png(exe_path: &str) -> Option<Vec<u8>> {
        unsafe {
            let mut info = SHFILEINFOW::default();
            let found = SHGetFileInfoW(
                &HSTRING::from(exe_path),
                FILE_FLAGS_AND_ATTRIBUTES(0),
                Some(&mut info),
                std::mem::size_of::<SHFILEINFOW>() as u32,
                SHGFI_ICON | SHGFI_LARGEICON,
            );
            if found == 0 || info.hIcon.is_invalid() {
                return None;
            }
            let png = icon_to_png(info.hIcon);
            let _ = DestroyIcon(info.hIcon);
            png
        }
    }

    unsafe fn bitmap_bgra(bitmap: HBITMAP, width: i32, height: i32) -> Option<Vec<u8>> {
        let dc = CreateCompatibleDC(None);
        let mut header = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        let lines = GetDIBits(
            dc,
            bitmap,
            0,
            height as u32,
            Some(pixels.as_mut_ptr() as *mut c_void),
            &mut header,
            DIB_RGB_COLORS,
        );
        let _ = DeleteDC(dc);
        (lines > 0).then_some(pixels)
    }

    unsafe fn icon_to_png(icon: HICON) -> Option<Vec<u8>> {
        let mut info = ICONINFO::default();
        GetIconInfo(icon, &mut info).ok()?;
        let result = (|| {
            let mut bitmap = BITMAP::default();
            if GetObjectW(
                info.hbmColor.into(),
                std::mem::size_of::<BITMAP>() as i32,
                Some(&mut bitmap as *mut BITMAP as *mut c_void),
            ) == 0
            {
                return None;
            }
            let (width, height) = (bitmap.bmWidth, bitmap.bmHeight);
            let mut pixels = bitmap_bgra(info.hbmColor, width, height)?;
            // Old icons carry no alpha; their transparency is in the mask,
            // where white means transparent.
            if pixels.as_chunks::<4>().0.iter().all(|p| p[3] == 0) {
                let mask = bitmap_bgra(info.hbmMask, width, height)?;
                for (p, m) in pixels
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .zip(mask.as_chunks::<4>().0)
                {
                    p[3] = 255 - m[0];
                }
            }
            for p in pixels.as_chunks_mut::<4>().0 {
                p.swap(0, 2);
            }
            let image = image::RgbaImage::from_raw(width as u32, height as u32, pixels)?;
            let mut png = Vec::new();
            image
                .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
                .ok()?;
            Some(png)
        })();
        let _ = DeleteObject(info.hbmColor.into());
        let _ = DeleteObject(info.hbmMask.into());
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(exe: &str, described: Option<&str>, title: &str) -> AppInfo {
        classify(&format!("C:\\Apps\\{exe}"), exe, described, title)
    }

    #[test]
    fn rules_move_apps_and_sites() {
        let rules = vec![
            AppRule {
                kind: AppRuleKind::App,
                pattern: "slack.exe".into(),
                name: "Slack".into(),
                category: AppCategory::Personal,
            },
            AppRule {
                kind: AppRuleKind::Site,
                pattern: "linear".into(),
                name: "Linear".into(),
                category: AppCategory::Work,
            },
            AppRule {
                kind: AppRuleKind::Site,
                pattern: "gmail".into(),
                name: "Gmail".into(),
                category: AppCategory::Other,
            },
        ];
        let mut slack = info("slack.exe", Some("Slack"), "general - Slack");
        apply_rules(&mut slack, &rules);
        assert_eq!(slack.category, AppCategory::Personal);
        let mut linear = info(
            "chrome.exe",
            Some("Google Chrome"),
            "LIN-12 - Linear - Google Chrome",
        );
        apply_rules(&mut linear, &rules);
        assert_eq!(
            (linear.site.as_deref(), linear.category),
            (Some("Linear"), AppCategory::Work)
        );
        let mut gmail = info(
            "chrome.exe",
            Some("Google Chrome"),
            "Inbox - Gmail - Google Chrome",
        );
        apply_rules(&mut gmail, &rules);
        assert_eq!(gmail.category, AppCategory::Other);
        assert_eq!(builtin_category(&rules[0]), AppCategory::Work);
        assert_eq!(rule_for(&linear, AppCategory::Ai).pattern, "linear");
    }

    #[test]
    fn browser_tab_is_matched_to_its_site() {
        let gmail = info(
            "chrome.exe",
            Some("Google Chrome"),
            "Inbox (3) - abhash@example.com - Gmail - Google Chrome",
        );
        assert_eq!(gmail.name, "Chrome");
        assert_eq!(gmail.site.as_deref(), Some("Gmail"));
        assert_eq!(gmail.category, AppCategory::Email);

        let claude = info(
            "msedge.exe",
            Some("Microsoft Edge"),
            "(2) Overlay bug - Claude - Microsoft Edge",
        );
        assert_eq!(claude.name, "Edge");
        assert_eq!(claude.category, AppCategory::Ai);
    }

    #[test]
    fn unknown_tab_stays_other() {
        let page = info(
            "firefox.exe",
            Some("Firefox"),
            "Rust docs — Mozilla Firefox",
        );
        assert_eq!(page.site, None);
        assert_eq!(page.category, AppCategory::Other);
    }

    #[test]
    fn desktop_apps_use_their_exe() {
        let outlook = info("olk.exe", Some("Microsoft Outlook"), "Inbox - Outlook");
        assert_eq!(outlook.name, "Outlook");
        assert_eq!(outlook.category, AppCategory::Email);
        let unknown = info("figma.exe", None, "Design");
        assert_eq!(unknown.name, "Figma");
        assert_eq!(unknown.category, AppCategory::Other);
    }

    #[test]
    fn claude_code_is_found_by_its_terminal_title() {
        let cc = info(
            "windowsterminal.exe",
            Some("Windows Terminal"),
            "✳ Claude Code",
        );
        assert_eq!(cc.name, "Claude Code");
        assert_eq!(cc.category, AppCategory::Ai);
        let shell = info("windowsterminal.exe", Some("Windows Terminal"), "pwsh");
        assert_eq!(shell.category, AppCategory::Code);
    }

    #[test]
    fn title_parts_drop_unread_counters() {
        assert_eq!(
            title_parts("(12) WhatsApp - Google Chrome"),
            vec!["WhatsApp", "Google Chrome"]
        );
    }
}
