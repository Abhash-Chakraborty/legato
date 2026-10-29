//! Windows shell integration: the app ID the taskbar groups Legato's windows
//! under, the jump list, notifications that keep quiet during Do not disturb,
//! and Mica behind the main window on Windows 11.

use std::sync::atomic::{AtomicBool, Ordering};

use log::{debug, warn};
use tauri::AppHandle;

// The jump list and notification strings, from the "desktop" section of the
// English locale file (see build.rs).
include!(concat!(env!("OUT_DIR"), "/desktop_translations.rs"));

/// The jump list's tasks: the label's string and the arguments the task
/// starts Legato with. A running Legato gets them from single-instance.
pub const DICTATE_ARG: &str = "--toggle-transcription";
pub const SCRATCHPAD_ARG: &str = "--open-scratchpad";
pub const NEW_NOTE_ARG: &str = "--new-note";

static MICA: AtomicBool = AtomicBool::new(false);

/// Windows 11 starts at build 22000; Mica needs it.
pub fn supports_mica() -> bool {
    cfg!(windows) && windows_build().is_some_and(|build| build >= 22000)
}

fn windows_build() -> Option<u32> {
    // "10.0.22631" on Windows 10 and 11 alike.
    tauri_plugin_os::version()
        .to_string()
        .split('.')
        .nth(2)?
        .parse()
        .ok()
}

/// Whether the main window was built with Mica behind it, so the frontend
/// can clear the background of the sidebar and title bar.
pub fn mica() -> bool {
    MICA.load(Ordering::Relaxed)
}

pub fn set_mica(on: bool) {
    MICA.store(on, Ordering::Relaxed);
}

/// Groups every Legato window, and the jump list, under the installer's
/// Start menu shortcut, however Legato was started (autostart has no
/// shortcut to inherit the ID from). Call before the first window opens.
pub fn set_app_id(app: &AppHandle) {
    #[cfg(windows)]
    unsafe {
        use windows::core::HSTRING;
        use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
        let id = HSTRING::from(app.config().identifier.as_str());
        if let Err(e) = SetCurrentProcessExplicitAppUserModelID(&id) {
            warn!("Couldn't set the app ID: {e}");
        }
    }
    #[cfg(not(windows))]
    let _ = app;
}

/// Runs Legato a step above normal priority. It barely uses the CPU, but
/// while a game had it, the microphone's capture thread missed its turn
/// hundreds of times a dictation ("xruns": audio lost before it reached
/// Legato) and shortcuts could lag. Processes Legato starts (the model's
/// worker, WebView2) start at normal priority, so the heavy work doesn't
/// compete with the game.
pub fn raise_priority() {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::System::Threading::{
            GetCurrentProcess, SetPriorityClass, ABOVE_NORMAL_PRIORITY_CLASS,
        };
        if let Err(e) = SetPriorityClass(GetCurrentProcess(), ABOVE_NORMAL_PRIORITY_CLASS) {
            warn!("Couldn't raise Legato's priority: {e}");
        }
    }
}

/// Writes the jump list's tasks: Dictate, and Scratchpad and New note
/// while the Scratchpad is on. Written again when that changes.
pub fn set_jump_list(app: &AppHandle) {
    let id = app.config().identifier.clone();
    let strings = desktop_strings();
    let mut tasks = vec![(strings.jump_dictate, DICTATE_ARG)];
    if crate::settings::get_settings(app).scratchpad_enabled {
        tasks.push((strings.jump_scratchpad, SCRATCHPAD_ARG));
        tasks.push((strings.jump_new_note, NEW_NOTE_ARG));
    }
    // COM wants its own apartment; the main thread's belongs to the webview.
    std::thread::spawn(move || {
        #[cfg(windows)]
        match unsafe { win::jump_list(&id, &tasks) } {
            Ok(()) => debug!("Jump list written"),
            Err(e) => warn!("Couldn't write the jump list: {e}"),
        }
        #[cfg(not(windows))]
        let _ = (id, tasks);
    });
}

/// What a notification opens when clicked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Click {
    /// The Scratchpad, on this note.
    Scratchpad(i64),
    /// The main window, where the footer offers the update.
    Main,
}

/// Whether Windows would show a notification now: not during Do not disturb
/// (Focus assist on Windows 10), a presentation, or a full-screen game.
pub fn accepts_notifications() -> bool {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::UI::Shell::{SHQueryUserNotificationState, QUNS_ACCEPTS_NOTIFICATIONS};
        // Windows 11's Do not disturb reads as "quiet time"; if the state
        // can't be read, Windows still holds the banner back itself.
        SHQueryUserNotificationState().map_or(true, |state| state == QUNS_ACCEPTS_NOTIFICATIONS)
    }
    #[cfg(not(windows))]
    false
}

/// Shows a Windows notification, unless the person shouldn't be disturbed.
/// Only for things that need them: a paste that went nowhere, an update.
pub fn notify(app: &AppHandle, title: &str, body: &str, click: Click) {
    if !accepts_notifications() {
        debug!("Notification held back: Do not disturb");
        return;
    }
    let id = app.config().identifier.clone();
    let xml = toast_xml(title, body);
    let app = app.clone();
    std::thread::spawn(move || {
        #[cfg(windows)]
        if let Err(e) = unsafe { win::toast(&id, &xml, move || on_click(&app, click)) } {
            warn!("Couldn't show a notification: {e}");
        }
        #[cfg(not(windows))]
        let _ = (id, xml, app, click);
    });
}

#[cfg_attr(not(windows), allow(dead_code))]
fn on_click(app: &AppHandle, click: Click) {
    match click {
        Click::Scratchpad(note) => {
            if let Err(e) = crate::scratchpad::show(app, Some(note)) {
                warn!("Couldn't open the Scratchpad: {e}");
            }
        }
        Click::Main => crate::show_main_window(app),
    }
}

/// A paste failed and the Scratchpad didn't open to show it, so the text
/// would otherwise sit there unnoticed.
pub fn notify_failed_paste(app: &AppHandle, note: i64, target: Option<&str>) {
    let strings = desktop_strings();
    let title = match target {
        Some(name) => strings.paste_failed_in.replace("{{app}}", name),
        None => strings.paste_failed,
    };
    notify(
        app,
        &title,
        &strings.paste_failed_body,
        Click::Scratchpad(note),
    );
}

/// A new version is ready to install.
pub fn notify_update(app: &AppHandle, version: &str) {
    let strings = desktop_strings();
    notify(
        app,
        &strings.update_title.replace("{{version}}", version),
        &strings.update_body,
        Click::Main,
    );
}

fn toast_xml(title: &str, body: &str) -> String {
    format!(
        "<toast><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual></toast>",
        xml_escape(title),
        xml_escape(body)
    )
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(windows)]
mod win {
    use std::sync::Mutex;

    use windows::core::{Interface, HSTRING};
    use windows::Data::Xml::Dom::XmlDocument;
    use windows::Foundation::TypedEventHandler;
    use windows::Win32::Storage::EnhancedStorage::PKEY_Title;
    use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
        COINIT_MULTITHREADED,
    };
    use windows::Win32::System::Variant::VT_LPWSTR;
    use windows::Win32::UI::Shell::Common::{IObjectArray, IObjectCollection};
    use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
    use windows::Win32::UI::Shell::{
        DestinationList, EnumerableObjectCollection, ICustomDestinationList, IShellLinkW,
        SHStrDupW, ShellLink,
    };
    use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};

    /// The last notification, kept alive so a click still reaches Legato.
    static LAST: Mutex<Option<ToastNotification>> = Mutex::new(None);

    pub unsafe fn jump_list(app_id: &str, tasks: &[(String, &str)]) -> windows::core::Result<()> {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let exe = std::env::current_exe().map_err(|e| {
            windows::core::Error::new(windows::Win32::Foundation::E_FAIL, e.to_string())
        })?;
        let exe = HSTRING::from(exe.as_os_str());

        let list: ICustomDestinationList =
            CoCreateInstance(&DestinationList, None, CLSCTX_INPROC_SERVER)?;
        list.SetAppID(&HSTRING::from(app_id))?;
        let mut slots = 0u32;
        let _removed: IObjectArray = list.BeginList(&mut slots)?;

        let collection: IObjectCollection =
            CoCreateInstance(&EnumerableObjectCollection, None, CLSCTX_INPROC_SERVER)?;
        for (title, args) in tasks {
            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
            link.SetPath(&exe)?;
            link.SetArguments(&HSTRING::from(*args))?;
            link.SetIconLocation(&exe, 0)?;
            link.SetDescription(&HSTRING::from(title.as_str()))?;
            // The title must be a plain wide string; a BSTR shows no label.
            // Dropping the value frees the copy with CoTaskMemFree.
            let mut value = PROPVARIANT::default();
            let inner = &mut *value.Anonymous.Anonymous;
            inner.vt = VT_LPWSTR;
            inner.Anonymous.pwszVal = SHStrDupW(&HSTRING::from(title.as_str()))?;
            let store: IPropertyStore = link.cast()?;
            store.SetValue(&PKEY_Title, &value)?;
            store.Commit()?;
            collection.AddObject(&link)?;
        }
        list.AddUserTasks(&collection.cast::<IObjectArray>()?)?;
        list.CommitList()
    }

    pub unsafe fn toast(
        app_id: &str,
        xml: &str,
        on_click: impl Fn() + Send + 'static,
    ) -> windows::core::Result<()> {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let doc = XmlDocument::new()?;
        doc.LoadXml(&HSTRING::from(xml))?;
        let toast = ToastNotification::CreateToastNotification(&doc)?;
        toast.Activated(&TypedEventHandler::new(move |_, _| {
            on_click();
            Ok(())
        }))?;
        ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(app_id))?
            .Show(&toast)?;
        if let Ok(mut last) = LAST.lock() {
            *last = Some(toast);
        }
        Ok(())
    }
}

/// Legato's floating windows, which Settings › Bar can keep out of
/// screenshots, recordings and shared screens: the bar or island (with the
/// live transcript it shows), the bubble and the Notes side panel. The main
/// window and the Scratchpad are yours to share.
const PRIVATE_WINDOWS: [&str; 3] = [
    "recording_overlay",
    crate::bubble::WINDOW,
    crate::notes_panel::WINDOW,
];

/// Keeps a floating window out of capture, or lets it back in, as the
/// setting says. Windows keeps this for the window while it hides and shows
/// again, so it's set once when the window is made and again when the
/// setting changes.
pub fn apply_capture_privacy(app: &AppHandle, window: &tauri::WebviewWindow) {
    let hide = crate::settings::get_settings(app).hide_from_capture;
    let window = window.clone();
    let _ = app.run_on_main_thread(move || set_capture_hidden(&window, hide));
}

/// The setting changed: the floating windows made so far follow it now.
pub fn capture_privacy_changed(app: &AppHandle) {
    use tauri::Manager;
    for label in PRIVATE_WINDOWS {
        if let Some(window) = app.get_webview_window(label) {
            apply_capture_privacy(app, &window);
        }
    }
}

/// Screen capture (sharing in a call, Snipping Tool, recorders, remote
/// desktops) leaves the window out and shows what's behind it. Needs
/// Windows 10 2004; on older builds the window stays visible.
fn set_capture_hidden(window: &tauri::WebviewWindow, hide: bool) {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{
            SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
        };
        let Ok(hwnd) = window.hwnd() else {
            return;
        };
        let affinity = if hide {
            WDA_EXCLUDEFROMCAPTURE
        } else {
            WDA_NONE
        };
        // SAFETY: the window is ours and alive; this is its owning thread.
        match unsafe { SetWindowDisplayAffinity(hwnd, affinity) } {
            Ok(()) => debug!(
                "'{}' is {} screen capture",
                window.label(),
                if hide { "hidden from" } else { "shown in" }
            ),
            Err(e) => warn!(
                "Couldn't change screen capture for '{}': {e}",
                window.label()
            ),
        }
    }
    #[cfg(not(windows))]
    let _ = (window, hide);
}

/// Moves and sizes a window in physical pixels, on the main thread. tao
/// converts a logical position at the scale of the screen the window is on
/// before the move, which misplaces it on a second screen with other
/// scaling; and arriving there makes tao resize it for the new scale, so
/// it's placed a second time once there.
pub fn place_physical(
    app: &AppHandle,
    window: &tauri::WebviewWindow,
    (x, y, width, height): (i32, i32, i32, i32),
) {
    let window = window.clone();
    let _ = app.run_on_main_thread(move || {
        #[cfg(windows)]
        {
            use windows::Win32::UI::WindowsAndMessaging::{
                SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER,
            };
            let Ok(hwnd) = window.hwnd() else {
                return;
            };
            for _ in 0..2 {
                // SAFETY: the window is ours and alive; this is its owning thread.
                if let Err(e) = unsafe {
                    SetWindowPos(
                        hwnd,
                        None,
                        x,
                        y,
                        width,
                        height,
                        SWP_NOACTIVATE | SWP_NOZORDER,
                    )
                } {
                    warn!("Couldn't place '{}': {e}", window.label());
                    return;
                }
            }
        }
        #[cfg(not(windows))]
        {
            let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
            let _ = window.set_size(tauri::PhysicalSize::new(
                width.max(0) as u32,
                height.max(0) as u32,
            ));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_text_is_escaped() {
        let xml = toast_xml("Couldn't paste into <Notes & Co>", "Saved \"here\"");
        assert!(xml.contains("Couldn&apos;t paste into &lt;Notes &amp; Co&gt;"));
        assert!(xml.contains("Saved &quot;here&quot;"));
    }
}
