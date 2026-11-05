//! Launch at login: a value in the current user's Run key that starts
//! Legato hidden in the tray.
//!
//! Only the current user's key. tauri-plugin-autostart wrote the machine's
//! Run key whenever Legato ran as administrator, so two Legatos started with
//! Windows and the second brought up the window despite Start hidden.

use auto_launch::{AutoLaunch, WindowsEnableMode};
use tauri::AppHandle;

/// The flag the Run entry starts Legato with.
pub const START_HIDDEN_ARG: &str = "--start-hidden";

fn entry(app: &AppHandle) -> Option<AutoLaunch> {
    let exe = std::env::current_exe().ok()?;
    Some(AutoLaunch::new(
        &app.package_info().name,
        &exe.display().to_string(),
        WindowsEnableMode::CurrentUser,
        &[START_HIDDEN_ARG],
    ))
}

/// Apply the user's autostart preference.
///
/// Errors are logged rather than returned: the preference is re-applied on
/// every launch, so a transient failure self-heals and must not block
/// startup. The entry is removed first, from the machine's key too when
/// Legato runs as administrator, then written again when it's on, so an
/// older entry gets this exe and the flag.
pub fn apply_autostart(app: &AppHandle, enabled: bool) {
    let Some(entry) = entry(app) else {
        log::warn!("Couldn't find Legato's exe for launch at login");
        return;
    };
    let result = entry
        .disable()
        .and_then(|()| if enabled { entry.enable() } else { Ok(()) });
    if let Err(e) = result {
        log::warn!(
            "Failed to apply autostart setting (enabled={}): {}",
            enabled,
            e
        );
    }
}
