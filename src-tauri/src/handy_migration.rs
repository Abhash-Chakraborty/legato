//! First-run move of Handy's data into Legato's folders.
//!
//! Legato started as a fork of Handy, which kept its data under the
//! `com.pais.handy` identifier. On Windows that is two folders: the roaming one
//! (settings, history, recordings, local models, custom sounds) and the local
//! one (logs and the WebView2 profile). Downloaded models live in the shared
//! Hugging Face cache, which doesn't depend on the identifier, so they stay put.
//!
//! Each Handy folder is moved only when Legato's folder doesn't exist yet. A
//! rename is tried first; a copy is made only when the rename fails because the
//! two folders are on different volumes. Handy's folder is never deleted, so a
//! failed move leaves it exactly where it was.
//!
//! Legato creates its own folders right after this runs, so a move that fails
//! is never tried again. Handy writes to its roaming folder while it runs, and
//! a rename can go through even then, since Handy may have nothing open there.
//! So before that folder moves, if `handy.exe` is running, a message box asks
//! the user to quit Handy and retry, or to start without the data. The box
//! says that Cancel is final. Files still in use get a few seconds of retries;
//! after that, and on any other failure, the folder is left behind, the failure
//! is logged and startup carries on. The local folder never shows a box.
//! Without one, each folder holds up startup by a few seconds at most.
//!
//! Once the roaming folder has moved, if Handy still looks installed, a second
//! box suggests uninstalling it so the two apps don't both start with Windows
//! and claim the same shortcut. It shows at most once, as the move does.
//!
//! The decisions live in [`flow`], which tests drive without Windows.

mod flow;

use std::fs;
use std::io;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

use log::Level;
use tauri::plugin::{Builder, TauriPlugin};
use tauri::{AppHandle, Manager, RunEvent, Runtime};
use windows::core::{w, HSTRING};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, IDRETRY, IDYES, MB_ICONINFORMATION, MB_ICONWARNING, MB_RETRYCANCEL,
    MB_SETFOREGROUND, MB_TOPMOST, MB_YESNO, MESSAGEBOX_RESULT, MESSAGEBOX_STYLE,
};
use winreg::enums::{
    HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
};
use winreg::RegKey;

use flow::{Choice, System};

const HANDY_IDENTIFIER: &str = "com.pais.handy";

/// Handy's executable. Its transcription worker runs the same file.
const HANDY_EXE: &str = "handy.exe";

/// Handy's product name, which names its Run entry and its uninstall entry.
const HANDY_NAME: &str = "Handy";

const QUIT_HANDY_TEXT: &str = "Handy is still running, so Legato can't bring over \
your settings and history yet.\n\nQuit Handy from its tray icon, then choose Retry. \
If you choose Cancel, Legato starts without them and won't ask again. They stay in \
Handy's folder.";

const UNINSTALL_HANDY_TEXT: &str = "Your settings, history and recordings are now \
in Legato.\n\nUninstall Handy so the two apps don't both start with Windows and \
fight over the same shortcut. Open Installed apps now?";

/// What the move did, held until the logger is attached.
struct Notes(Vec<(Level, String)>);

/// The migration as a plugin, so it can run before anything touches Legato's
/// folders. Register it first: plugins set up in registration order, the log
/// plugin creates Legato's log folder in its setup, and the managers read
/// settings later, in app setup.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("handy-migration")
        .setup(|app, _api| {
            // A portable install keeps its data next to the exe instead.
            let notes = if crate::portable::is_portable() {
                Vec::new()
            } else {
                migrate(app)
            };
            app.manage(Notes(notes));
            Ok(())
        })
        .on_event(|app, event| {
            // The log plugin attaches its logger after this plugin's setup, so
            // report once the app is ready.
            if !matches!(event, RunEvent::Ready) {
                return;
            }
            if let Some(notes) = app.try_state::<Notes>() {
                for (level, note) in &notes.0 {
                    log::log!(*level, "{note}");
                }
            }
        })
        .build()
}

/// Moves each of Handy's folders that has content to Legato's matching folder.
fn migrate<R: Runtime>(app: &AppHandle<R>) -> Vec<(Level, String)> {
    let paths = app.path();
    // On Windows the config dir is the roaming data dir, and the cache and log
    // dirs sit inside the local data dir, so these two roots cover everything.
    let roaming = paths.app_data_dir().ok().and_then(next_to_handy);
    let local = paths.app_local_data_dir().ok().and_then(next_to_handy);
    flow::migrate(&mut Windows, roaming, local)
}

/// Pairs Handy's folder with Legato's folder `new`, its sibling.
fn next_to_handy(new: PathBuf) -> Option<(PathBuf, PathBuf)> {
    let old = new.parent()?.join(HANDY_IDENTIFIER);
    Some((old, new))
}

/// The real system: the file system, the process list, the registry and
/// native message boxes.
struct Windows;

impl System for Windows {
    fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()> {
        fs::rename(from, to)
    }

    fn sleep(&mut self, duration: Duration) {
        thread::sleep(duration);
    }

    fn handy_running(&mut self) -> bool {
        handy_running()
    }

    fn ask_to_quit_handy(&mut self) -> Choice {
        if message_box(QUIT_HANDY_TEXT, MB_RETRYCANCEL | MB_ICONWARNING) == IDRETRY {
            Choice::Retry
        } else {
            Choice::Cancel
        }
    }

    fn handy_installed(&mut self) -> bool {
        handy_installed()
    }

    fn suggest_uninstalling_handy(&mut self) -> bool {
        message_box(UNINSTALL_HANDY_TEXT, MB_YESNO | MB_ICONINFORMATION) == IDYES
    }

    fn open_installed_apps(&mut self) -> io::Result<()> {
        // Like the microphone privacy link: `start` hands the URI to the shell.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        Command::new("cmd")
            .args(["/C", "start", "", "ms-settings:appsfeatures"])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map(drop)
    }
}

/// Shows a message box titled Legato and returns the button chosen. A failed
/// box returns 0, which matches no button.
fn message_box(text: &str, style: MESSAGEBOX_STYLE) -> MESSAGEBOX_RESULT {
    // No window exists yet, so the box has no owner. Keep it in front so it
    // isn't lost behind other windows.
    let style = style | MB_SETFOREGROUND | MB_TOPMOST;
    unsafe { MessageBoxW(None, &HSTRING::from(text), w!("Legato"), style) }
}

/// Whether a process other than this one runs Handy's executable.
fn handy_running() -> bool {
    // SAFETY: the snapshot handle is owned here and closed before returning,
    // and the entry's size field is set as the API requires.
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return false;
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut found = false;
        let mut more = Process32FirstW(snapshot, &mut entry).is_ok();
        while more && !found {
            found = is_handy(&entry);
            more = Process32NextW(snapshot, &mut entry).is_ok();
        }
        let _ = CloseHandle(snapshot);
        found
    }
}

/// Whether a process list entry runs Handy's executable and isn't this process.
fn is_handy(entry: &PROCESSENTRY32W) -> bool {
    let name = &entry.szExeFile;
    let len = name.iter().position(|&c| c == 0).unwrap_or(name.len());
    entry.th32ProcessID != std::process::id()
        && String::from_utf16_lossy(&name[..len]).eq_ignore_ascii_case(HANDY_EXE)
}

/// Whether Handy still looks installed: it starts with Windows, or Windows
/// lists it as an installed app.
fn handy_installed() -> bool {
    // tauri-plugin-autostart names the Run entry after the product, and
    // Handy's uninstaller removes it.
    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    // Handy's NSIS installer writes Uninstall\Handy: to HKCU by default, or to
    // the 32-bit view of HKLM for all users. Its MSI writes an entry named
    // after its product code. All of them have Handy as the display name.
    const UNINSTALL: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall";

    let starts_with_windows = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(RUN)
        .is_ok_and(|run| run.get_raw_value(HANDY_NAME).is_ok());
    starts_with_windows
        || [
            (HKEY_CURRENT_USER, KEY_READ),
            (HKEY_LOCAL_MACHINE, KEY_READ | KEY_WOW64_64KEY),
            (HKEY_LOCAL_MACHINE, KEY_READ | KEY_WOW64_32KEY),
        ]
        .into_iter()
        .any(|(root, access)| {
            let Ok(apps) = RegKey::predef(root).open_subkey_with_flags(UNINSTALL, access) else {
                return false;
            };
            apps.enum_keys().flatten().any(|app| {
                apps.open_subkey_with_flags(&app, access)
                    .and_then(|app| app.get_value::<String, _>("DisplayName"))
                    .is_ok_and(|name| name == HANDY_NAME)
            })
        })
}
