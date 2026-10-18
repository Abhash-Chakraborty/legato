//! Tray menu strings
//!
//! Generated at compile time by build.rs from the "tray" section of the
//! frontend's English locale file (src/i18n/locales/en/translation.json), so
//! the tray and the UI share one source of truth.
//!
//! To add a new tray menu item:
//! 1. Add the key to en/translation.json under "tray"
//! 2. Update tray.rs to use the new field (e.g., strings.new_field)

// Include the generated TrayStrings struct and tray_strings()
include!(concat!(env!("OUT_DIR"), "/tray_translations.rs"));
