//! Launch-at-login (autostart) handling through tauri-plugin-autostart.

use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

/// Apply the user's autostart preference.
///
/// Errors are logged rather than returned: the preference is re-applied on
/// every launch, so a transient failure self-heals and must not block
/// startup. This mirrors the pre-existing behavior of ignoring
/// enable()/disable() results.
pub fn apply_autostart(app: &AppHandle, enabled: bool) {
    let manager = app.autolaunch();
    // Disabling an entry that was never created fails with "file not found"
    // on Windows, which is nothing to warn about.
    let already = manager.is_enabled().unwrap_or(enabled);
    // An entry that's on is written again, so one from before it carried
    // --start-hidden gets the flag.
    if already == enabled && !enabled {
        return;
    }
    let result = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    if let Err(e) = result {
        log::warn!(
            "Failed to apply autostart setting (enabled={}): {}",
            enabled,
            e
        );
    }
}
