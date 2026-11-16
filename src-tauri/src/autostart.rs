//! Launch at login: a value in the current user's Run key that starts
//! Legato hidden in the tray.
//!
//! Only the current user's key. tauri-plugin-autostart wrote the machine's
//! Run key whenever Legato ran as administrator, so two Legatos started with
//! Windows and the second brought up the window despite Start hidden.
//!
//! On a Mac it's a launch agent in the user's Library
//! (`~/Library/LaunchAgents/<identifier>.plist`) that starts Legato the
//! same way, hidden in the menu bar.

#[cfg(target_os = "macos")]
use auto_launch::{AutoLaunch, MacOSLaunchMode};
#[cfg(windows)]
use auto_launch::{AutoLaunch, WindowsEnableMode};
use tauri::AppHandle;

/// The flag the Run entry starts Legato with.
pub const START_HIDDEN_ARG: &str = "--start-hidden";

#[cfg(target_os = "macos")]
fn entry(app: &AppHandle) -> Option<AutoLaunch> {
    let exe = std::env::current_exe().ok()?;
    let no_bundles: [&str; 0] = [];
    Some(AutoLaunch::new(
        &app.config().identifier,
        &exe.display().to_string(),
        MacOSLaunchMode::LaunchAgent,
        &[START_HIDDEN_ARG],
        &no_bundles,
        "",
    ))
}

#[cfg(windows)]
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
#[cfg(any(windows, target_os = "macos"))]
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

/// Nowhere else has a launch-at-login entry yet.
#[cfg(not(any(windows, target_os = "macos")))]
pub fn apply_autostart(_app: &AppHandle, _enabled: bool) {}
