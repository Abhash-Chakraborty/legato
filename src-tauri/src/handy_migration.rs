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

use std::fs;
use std::io;
use std::path::Path;

use log::Level;
use tauri::plugin::{Builder, TauriPlugin};
use tauri::{AppHandle, Manager, RunEvent, Runtime};

const HANDY_IDENTIFIER: &str = "com.pais.handy";

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
    [paths.app_data_dir(), paths.app_local_data_dir()]
        .into_iter()
        .flatten()
        .filter_map(|new| {
            let old = new.parent()?.join(HANDY_IDENTIFIER);
            move_folder(&old, &new)
        })
        .collect()
}

/// Moves `old` to `new` when `new` is missing and `old` has content, and says
/// what happened. Returns `None` when there was nothing to do.
fn move_folder(old: &Path, new: &Path) -> Option<(Level, String)> {
    let has_content = fs::read_dir(old).is_ok_and(|mut entries| entries.next().is_some());
    if new.exists() || !has_content {
        return None;
    }
    let (from, to) = (old.display(), new.display());
    Some(match fs::rename(old, new) {
        Ok(()) => (
            Level::Info,
            format!("Moved Handy's data from {from} to {to}"),
        ),
        Err(e) if e.kind() == io::ErrorKind::CrossesDevices => match copy_dir(old, new) {
            Ok(()) => (
                Level::Info,
                format!("Copied {from} to {to}; Handy's folder was kept"),
            ),
            Err(e) => {
                // Drop the partial copy so Legato starts clean instead of with
                // half of Handy's files.
                let _ = fs::remove_dir_all(new);
                let note = format!("Could not copy {from} to {to}: {e}; it was left in place");
                (Level::Warn, note)
            }
        },
        // Most likely Handy is still running and holds files open.
        Err(e) => {
            let note = format!("Could not move {from} to {to}: {e}; it was left in place");
            (Level::Warn, note)
        }
    })
}

/// Copies the directory tree at `from` to `to`, which must not exist yet.
fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_handy_folder_when_legato_has_none() {
        let root = tempfile::tempdir().unwrap();
        let (old, new) = (root.path().join("old"), root.path().join("new"));
        fs::create_dir_all(old.join("recordings")).unwrap();
        fs::write(old.join("settings_store.json"), "{}").unwrap();

        let (level, _) = move_folder(&old, &new).expect("a move to report");
        assert_eq!(level, Level::Info);
        assert!(!old.exists());
        assert!(new.join("settings_store.json").is_file());
        assert!(new.join("recordings").is_dir());
    }

    #[test]
    fn leaves_both_folders_alone_when_legato_has_one() {
        let root = tempfile::tempdir().unwrap();
        let (old, new) = (root.path().join("old"), root.path().join("new"));
        fs::create_dir_all(&old).unwrap();
        fs::write(old.join("settings_store.json"), "{}").unwrap();
        fs::create_dir_all(&new).unwrap();

        assert!(move_folder(&old, &new).is_none());
        assert!(old.join("settings_store.json").is_file());
    }

    #[test]
    fn skips_missing_or_empty_handy_folder() {
        let root = tempfile::tempdir().unwrap();
        let (old, new) = (root.path().join("old"), root.path().join("new"));
        assert!(move_folder(&old, &new).is_none());

        fs::create_dir_all(&old).unwrap();
        assert!(move_folder(&old, &new).is_none());
        assert!(!new.exists());
    }

    #[test]
    fn copy_dir_copies_nested_files_and_keeps_the_source() {
        let root = tempfile::tempdir().unwrap();
        let (from, to) = (root.path().join("from"), root.path().join("to"));
        fs::create_dir_all(from.join("logs")).unwrap();
        fs::write(from.join("logs").join("handy.log"), "line").unwrap();

        copy_dir(&from, &to).unwrap();
        assert_eq!(
            fs::read_to_string(to.join("logs").join("handy.log")).unwrap(),
            "line"
        );
        assert!(from.join("logs").join("handy.log").is_file());
    }
}
