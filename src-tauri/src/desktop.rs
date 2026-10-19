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

/// Writes the jump list's tasks: Dictate, Scratchpad and New note.
pub fn set_jump_list(app: &AppHandle) {
    let id = app.config().identifier.clone();
    let strings = desktop_strings();
    let tasks = vec![
        (strings.jump_dictate, DICTATE_ARG),
        (strings.jump_scratchpad, SCRATCHPAD_ARG),
        (strings.jump_new_note, NEW_NOTE_ARG),
    ];
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
