//! The move's decisions, kept apart from Windows and Tauri so tests can drive
//! them through a scripted [`System`] that never shows a message box.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use log::Level;

/// Windows error codes for a file another process has open: access denied,
/// sharing violation and lock violation. Renaming a folder fails with one of
/// them while a file inside it is open.
const IN_USE_ERRORS: [i32; 3] = [5, 32, 33];

/// Retries for files that are about to close, about three seconds in all.
/// After Handy quits, its WebView2 helpers can hold files for a few seconds.
const RETRIES: u32 = 6;
const RETRY_DELAY: Duration = Duration::from_millis(500);

/// What the move needs from the system. The parent module has the real one.
pub(super) trait System {
    /// Renames a folder, like [`fs::rename`].
    fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()>;
    /// Waits before the next try.
    fn sleep(&mut self, duration: Duration);
    /// Whether a Handy process is running.
    fn handy_running(&mut self) -> bool;
    /// Asks the user to quit Handy, or to start without its data.
    fn ask_to_quit_handy(&mut self) -> Choice;
    /// Whether Handy still looks installed.
    fn handy_installed(&mut self) -> bool;
    /// Suggests uninstalling Handy. Returns whether to open Installed apps.
    fn suggest_uninstalling_handy(&mut self) -> bool;
    /// Opens Installed apps in Windows Settings.
    fn open_installed_apps(&mut self) -> io::Result<()>;
}

/// The user's answer when a running Handy blocks the move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Choice {
    /// Handy was quit, so try again.
    Retry,
    /// Start without Handy's data.
    Cancel,
}

/// Which of Handy's folders is being moved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Folder {
    /// Settings, history and recordings: worth asking the user about.
    Roaming,
    /// Logs and the WebView2 profile: never worth holding up startup for.
    Local,
}

/// Where a pair of folders stands before a move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FolderState {
    /// Handy's folder is missing or empty.
    NothingToMove,
    /// Legato's folder exists, so Legato has run before.
    LegatoFolderExists,
    /// Handy's folder has data and Legato's doesn't exist yet.
    ReadyToMove,
}

/// Legato's folder wins: once it exists, Handy's is left alone for good.
pub(super) fn folder_state(handy_has_data: bool, legato_exists: bool) -> FolderState {
    match (handy_has_data, legato_exists) {
        (_, true) => FolderState::LegatoFolderExists,
        (false, false) => FolderState::NothingToMove,
        (true, false) => FolderState::ReadyToMove,
    }
}

/// Why a rename failed, as far as the move is concerned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Failure {
    /// The folders are on different volumes, which a rename can't cross.
    OtherVolume,
    /// Another process has a file in the folder open.
    InUse,
    /// Anything else, such as a full disk or a bad path.
    Other,
}

/// Sorts a failed rename by what can be done about it.
pub(super) fn classify(error: &io::Error) -> Failure {
    if error.kind() == io::ErrorKind::CrossesDevices {
        Failure::OtherVolume
    } else if error
        .raw_os_error()
        .is_some_and(|code| IN_USE_ERRORS.contains(&code))
    {
        Failure::InUse
    } else {
        Failure::Other
    }
}

/// Whether a failed rename was the first try or came after brief retries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Attempt {
    First,
    Retried,
}

/// What to do after a rename failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Step {
    /// Copy the folder instead.
    Copy,
    /// Try again for a few seconds: the files may be about to close.
    RetryBriefly,
    /// Ask the user to quit Handy, which has the files open.
    AskToQuitHandy,
    /// Leave Handy's folder where it is and start without it.
    GiveUp,
}

/// Picks what to do after a failed rename. Only the roaming folder is worth a
/// message box, and only a running Handy is worth one.
pub(super) fn next_step(
    folder: Folder,
    failure: Failure,
    handy_running: bool,
    attempt: Attempt,
) -> Step {
    match failure {
        Failure::OtherVolume => Step::Copy,
        Failure::InUse if handy_running && folder == Folder::Roaming => Step::AskToQuitHandy,
        // Retrying on its own only helps once Handy is gone.
        Failure::InUse if !handy_running && attempt == Attempt::First => Step::RetryBriefly,
        Failure::InUse | Failure::Other => Step::GiveUp,
    }
}

/// What happened to one of Handy's folders.
#[derive(Debug)]
pub(super) enum Outcome {
    /// Renamed into place.
    Moved,
    /// Copied across volumes; Handy's folder was kept.
    Copied,
    /// The copy failed and the partial copy was removed.
    CopyFailed(io::Error),
    /// Handy had files open and the user chose to start without them.
    Cancelled(io::Error),
    /// The move failed without asking the user; Handy's folder stayed put.
    Failed(io::Error),
}

impl Outcome {
    /// Whether Handy's data is now in Legato's folder.
    fn brought_over(&self) -> bool {
        matches!(self, Outcome::Moved | Outcome::Copied)
    }

    /// The log line for this outcome.
    fn note(&self, old: &Path, new: &Path) -> (Level, String) {
        let (from, to) = (old.display(), new.display());
        match self {
            Outcome::Moved => (
                Level::Info,
                format!("Moved Handy's data from {from} to {to}"),
            ),
            Outcome::Copied => (
                Level::Info,
                format!("Copied {from} to {to}; Handy's folder was kept"),
            ),
            Outcome::CopyFailed(e) => (
                Level::Warn,
                format!("Could not copy {from} to {to}: {e}; it was left in place"),
            ),
            Outcome::Cancelled(e) => (
                Level::Warn,
                format!("Handy had files open in {from} ({e}); the user chose to start without its data"),
            ),
            Outcome::Failed(e) => (
                Level::Warn,
                format!("Could not move {from} to {to}: {e}; it was left in place"),
            ),
        }
    }
}

/// Moves Handy's roaming and local folders, each given as (Handy's folder,
/// Legato's folder), then suggests uninstalling Handy once its data is in
/// Legato. Returns what happened, for the log.
pub(super) fn migrate(
    system: &mut impl System,
    roaming: Option<(PathBuf, PathBuf)>,
    local: Option<(PathBuf, PathBuf)>,
) -> Vec<(Level, String)> {
    let mut notes = Vec::new();
    let mut brought_over = false;
    for (folder, pair) in [(Folder::Roaming, roaming), (Folder::Local, local)] {
        let Some((old, new)) = pair else { continue };
        if let Some(outcome) = move_folder(system, folder, &old, &new) {
            brought_over |= folder == Folder::Roaming && outcome.brought_over();
            notes.push(outcome.note(&old, &new));
        }
    }
    // Only now, so Handy's uninstaller can't delete a folder still being moved.
    if brought_over && system.handy_installed() {
        notes.push((
            Level::Info,
            "Handy still looks installed; suggested uninstalling it".to_string(),
        ));
        if system.suggest_uninstalling_handy() {
            if let Err(e) = system.open_installed_apps() {
                notes.push((Level::Warn, format!("Could not open Installed apps: {e}")));
            }
        }
    }
    notes
}

/// Moves Handy's folder `old` to Legato's folder `new` when there is data to
/// move. Returns what happened, or `None` when there was nothing to do.
pub(super) fn move_folder(
    system: &mut impl System,
    folder: Folder,
    old: &Path,
    new: &Path,
) -> Option<Outcome> {
    let ready = || folder_state(has_data(old), new.exists()) == FolderState::ReadyToMove;
    if !ready() {
        return None;
    }
    let mut attempt = Attempt::First;
    let mut result = system.rename(old, new);
    loop {
        let error = match result {
            Ok(()) => return Some(Outcome::Moved),
            Err(error) => error,
        };
        // The folders can change while a box is up, for example when Legato
        // was started twice and the other one moved them.
        if !ready() {
            return None;
        }
        let failure = classify(&error);
        let handy_running = failure == Failure::InUse && system.handy_running();
        match next_step(folder, failure, handy_running, attempt) {
            Step::Copy => return Some(copy_folder(old, new)),
            Step::RetryBriefly => {}
            Step::AskToQuitHandy => {
                if system.ask_to_quit_handy() == Choice::Cancel {
                    return Some(Outcome::Cancelled(error));
                }
            }
            Step::GiveUp => return Some(Outcome::Failed(error)),
        }
        attempt = Attempt::Retried;
        result = retry_rename(system, old, new);
    }
}

/// Tries the rename again every half second for about three seconds, until
/// it works or fails for a reason other than files in use.
fn retry_rename(system: &mut impl System, old: &Path, new: &Path) -> io::Result<()> {
    let mut tries = 0;
    loop {
        system.sleep(RETRY_DELAY);
        tries += 1;
        let result = system.rename(old, new);
        let in_use = result
            .as_ref()
            .is_err_and(|error| classify(error) == Failure::InUse);
        if !in_use || tries == RETRIES {
            return result;
        }
    }
}

/// Whether `dir` exists and has at least one entry.
fn has_data(dir: &Path) -> bool {
    fs::read_dir(dir).is_ok_and(|mut entries| entries.next().is_some())
}

/// Copies `old` to `new`, keeping Handy's folder. A failed copy is removed so
/// Legato starts clean instead of with half of Handy's files.
fn copy_folder(old: &Path, new: &Path) -> Outcome {
    match copy_dir(old, new) {
        Ok(()) => Outcome::Copied,
        Err(e) => {
            let _ = fs::remove_dir_all(new);
            Outcome::CopyFailed(e)
        }
    }
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
    use std::collections::VecDeque;

    /// A scripted system. Renames fail with the queued errors, then go
    /// through; each message box takes the next queued answer.
    #[derive(Default)]
    struct Fake {
        rename_errors: VecDeque<io::Error>,
        handy_running: bool,
        /// Whether choosing Retry also quits Handy.
        retry_quits_handy: bool,
        choices: VecDeque<Choice>,
        /// A rename another Legato makes while the box is up.
        moved_meanwhile: Option<(PathBuf, PathBuf)>,
        installed: bool,
        wants_installed_apps: bool,
        installed_apps_fail: bool,
        // What happened.
        renames: u32,
        slept: Duration,
        asked: u32,
        suggested: u32,
        opened: u32,
    }

    impl System for Fake {
        fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()> {
            self.renames += 1;
            match self.rename_errors.pop_front() {
                Some(error) => Err(error),
                None => fs::rename(from, to),
            }
        }

        fn sleep(&mut self, duration: Duration) {
            self.slept += duration;
        }

        fn handy_running(&mut self) -> bool {
            self.handy_running
        }

        fn ask_to_quit_handy(&mut self) -> Choice {
            self.asked += 1;
            if let Some((from, to)) = self.moved_meanwhile.take() {
                fs::rename(from, to).unwrap();
            }
            let choice = self.choices.pop_front().expect("an answer for each box");
            if choice == Choice::Retry && self.retry_quits_handy {
                self.handy_running = false;
            }
            choice
        }

        fn handy_installed(&mut self) -> bool {
            self.installed
        }

        fn suggest_uninstalling_handy(&mut self) -> bool {
            self.suggested += 1;
            self.wants_installed_apps
        }

        fn open_installed_apps(&mut self) -> io::Result<()> {
            self.opened += 1;
            if self.installed_apps_fail {
                Err(io::Error::other("no Settings app"))
            } else {
                Ok(())
            }
        }
    }

    fn in_use() -> io::Error {
        io::Error::from_raw_os_error(32)
    }

    fn errors(count: usize, error: fn() -> io::Error) -> VecDeque<io::Error> {
        (0..count).map(|_| error()).collect()
    }

    /// A temp dir holding Handy's folder with a settings file and a
    /// recordings folder, and the path Legato's folder would take.
    fn handy_folder() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let (old, new) = (root.path().join("old"), root.path().join("new"));
        fs::create_dir_all(old.join("recordings")).unwrap();
        fs::write(old.join("settings_store.json"), "{}").unwrap();
        (root, old, new)
    }

    fn assert_moved(old: &Path, new: &Path) {
        assert!(!old.exists());
        assert!(new.join("settings_store.json").is_file());
        assert!(new.join("recordings").is_dir());
    }

    fn assert_left_in_place(old: &Path, new: &Path) {
        assert!(old.join("settings_store.json").is_file());
        assert!(!new.exists());
    }

    #[test]
    fn moves_only_into_a_missing_legato_folder() {
        assert_eq!(folder_state(true, false), FolderState::ReadyToMove);
        assert_eq!(folder_state(false, false), FolderState::NothingToMove);
        assert_eq!(folder_state(true, true), FolderState::LegatoFolderExists);
        assert_eq!(folder_state(false, true), FolderState::LegatoFolderExists);
    }

    #[test]
    fn tells_other_volume_in_use_and_other_errors_apart() {
        let other_volume = io::Error::from(io::ErrorKind::CrossesDevices);
        assert_eq!(classify(&other_volume), Failure::OtherVolume);
        // Access denied, sharing violation, lock violation.
        for code in [5, 32, 33] {
            let in_use = io::Error::from_raw_os_error(code);
            assert_eq!(classify(&in_use), Failure::InUse);
        }
        // File not found, and a full disk.
        assert_eq!(classify(&io::Error::from_raw_os_error(2)), Failure::Other);
        let full = io::Error::from(io::ErrorKind::StorageFull);
        assert_eq!(classify(&full), Failure::Other);
    }

    #[test]
    fn asks_only_about_the_roaming_folder_while_handy_runs() {
        for attempt in [Attempt::First, Attempt::Retried] {
            let step = next_step(Folder::Roaming, Failure::InUse, true, attempt);
            assert_eq!(step, Step::AskToQuitHandy);
            let step = next_step(Folder::Local, Failure::InUse, true, attempt);
            assert_eq!(step, Step::GiveUp);
        }
    }

    #[test]
    fn retries_once_when_files_are_in_use_but_handy_is_not_running() {
        for folder in [Folder::Roaming, Folder::Local] {
            let first = next_step(folder, Failure::InUse, false, Attempt::First);
            assert_eq!(first, Step::RetryBriefly);
            let retried = next_step(folder, Failure::InUse, false, Attempt::Retried);
            assert_eq!(retried, Step::GiveUp);
        }
    }

    #[test]
    fn copies_across_volumes_and_gives_up_on_other_errors() {
        for folder in [Folder::Roaming, Folder::Local] {
            for handy_running in [false, true] {
                for attempt in [Attempt::First, Attempt::Retried] {
                    let step = next_step(folder, Failure::OtherVolume, handy_running, attempt);
                    assert_eq!(step, Step::Copy);
                    let step = next_step(folder, Failure::Other, handy_running, attempt);
                    assert_eq!(step, Step::GiveUp);
                }
            }
        }
    }

    #[test]
    fn moves_handy_folder_when_legato_has_none() {
        let (_root, old, new) = handy_folder();
        let mut system = Fake::default();

        let outcome = move_folder(&mut system, Folder::Roaming, &old, &new);
        assert!(matches!(outcome, Some(Outcome::Moved)));
        assert_moved(&old, &new);
        assert_eq!((system.asked, system.slept), (0, Duration::ZERO));
    }

    #[test]
    fn leaves_both_folders_alone_when_legato_has_one() {
        let (_root, old, new) = handy_folder();
        fs::create_dir_all(&new).unwrap();
        let mut system = Fake::default();

        assert!(move_folder(&mut system, Folder::Roaming, &old, &new).is_none());
        assert!(old.join("settings_store.json").is_file());
        assert_eq!(system.renames, 0);
    }

    #[test]
    fn skips_missing_or_empty_handy_folder() {
        let root = tempfile::tempdir().unwrap();
        let (old, new) = (root.path().join("old"), root.path().join("new"));
        let mut system = Fake::default();
        assert!(move_folder(&mut system, Folder::Roaming, &old, &new).is_none());

        fs::create_dir_all(&old).unwrap();
        assert!(move_folder(&mut system, Folder::Roaming, &old, &new).is_none());
        assert!(!new.exists());
        assert_eq!(system.renames, 0);
    }

    #[test]
    fn asks_to_quit_handy_then_moves_after_retry() {
        let (_root, old, new) = handy_folder();
        // Handy's helpers still hold a file just after it quits.
        let mut system = Fake {
            rename_errors: errors(2, in_use),
            handy_running: true,
            retry_quits_handy: true,
            choices: [Choice::Retry].into(),
            ..Fake::default()
        };

        let outcome = move_folder(&mut system, Folder::Roaming, &old, &new);
        assert!(matches!(outcome, Some(Outcome::Moved)));
        assert_moved(&old, &new);
        assert_eq!(system.asked, 1);
        assert_eq!(system.renames, 3);
        assert_eq!(system.slept, 2 * RETRY_DELAY);
    }

    #[test]
    fn cancel_starts_without_handys_data() {
        let (_root, old, new) = handy_folder();
        let mut system = Fake {
            rename_errors: errors(1, in_use),
            handy_running: true,
            choices: [Choice::Cancel].into(),
            ..Fake::default()
        };

        let outcome = move_folder(&mut system, Folder::Roaming, &old, &new);
        assert!(matches!(outcome, Some(Outcome::Cancelled(_))));
        assert_left_in_place(&old, &new);
        assert_eq!((system.asked, system.slept), (1, Duration::ZERO));
    }

    #[test]
    fn asks_again_after_retry_while_handy_keeps_running() {
        let (_root, old, new) = handy_folder();
        let mut system = Fake {
            rename_errors: errors(1 + RETRIES as usize, in_use),
            handy_running: true,
            choices: [Choice::Retry, Choice::Cancel].into(),
            ..Fake::default()
        };

        let outcome = move_folder(&mut system, Folder::Roaming, &old, &new);
        assert!(matches!(outcome, Some(Outcome::Cancelled(_))));
        assert_left_in_place(&old, &new);
        assert_eq!(system.asked, 2);
        assert_eq!(system.renames, 1 + RETRIES);
        assert_eq!(system.slept, RETRIES * RETRY_DELAY);
    }

    #[test]
    fn retries_briefly_without_a_box_when_handy_is_not_running() {
        let (_root, old, new) = handy_folder();
        let mut system = Fake {
            rename_errors: errors(2, in_use),
            ..Fake::default()
        };

        let outcome = move_folder(&mut system, Folder::Roaming, &old, &new);
        assert!(matches!(outcome, Some(Outcome::Moved)));
        assert_moved(&old, &new);
        assert_eq!(system.asked, 0);
        assert_eq!(system.slept, 2 * RETRY_DELAY);
    }

    #[test]
    fn gives_up_after_three_seconds_when_files_stay_in_use_without_handy() {
        let (_root, old, new) = handy_folder();
        let mut system = Fake {
            rename_errors: errors(1 + RETRIES as usize, in_use),
            ..Fake::default()
        };

        let outcome = move_folder(&mut system, Folder::Roaming, &old, &new);
        assert!(matches!(outcome, Some(Outcome::Failed(_))));
        assert_left_in_place(&old, &new);
        assert_eq!(system.asked, 0);
        assert_eq!(system.renames, 1 + RETRIES);
        assert_eq!(system.slept, Duration::from_secs(3));
    }

    #[test]
    fn gives_up_at_once_on_other_errors() {
        let (_root, old, new) = handy_folder();
        let mut system = Fake {
            rename_errors: errors(1, || io::ErrorKind::StorageFull.into()),
            handy_running: true,
            ..Fake::default()
        };

        let outcome = move_folder(&mut system, Folder::Roaming, &old, &new);
        assert!(matches!(outcome, Some(Outcome::Failed(_))));
        assert_left_in_place(&old, &new);
        assert_eq!((system.asked, system.renames), (0, 1));
        assert_eq!(system.slept, Duration::ZERO);
    }

    #[test]
    fn never_holds_up_startup_for_the_local_folder() {
        let (_root, old, new) = handy_folder();
        let mut system = Fake {
            rename_errors: errors(1, in_use),
            handy_running: true,
            ..Fake::default()
        };

        let outcome = move_folder(&mut system, Folder::Local, &old, &new);
        assert!(matches!(outcome, Some(Outcome::Failed(_))));
        assert_left_in_place(&old, &new);
        assert_eq!((system.asked, system.slept), (0, Duration::ZERO));
    }

    #[test]
    fn stops_when_another_legato_moved_the_folder_meanwhile() {
        let (_root, old, new) = handy_folder();
        let mut system = Fake {
            rename_errors: errors(1, in_use),
            handy_running: true,
            choices: [Choice::Retry].into(),
            moved_meanwhile: Some((old.clone(), new.clone())),
            ..Fake::default()
        };

        assert!(move_folder(&mut system, Folder::Roaming, &old, &new).is_none());
        assert_moved(&old, &new);
        assert_eq!(system.asked, 1);
    }

    #[test]
    fn copies_across_volumes_and_keeps_handys_folder() {
        let (_root, old, new) = handy_folder();
        let mut system = Fake {
            rename_errors: errors(1, || io::ErrorKind::CrossesDevices.into()),
            ..Fake::default()
        };

        let outcome = move_folder(&mut system, Folder::Roaming, &old, &new);
        assert!(matches!(outcome, Some(Outcome::Copied)));
        assert!(old.join("settings_store.json").is_file());
        assert!(new.join("settings_store.json").is_file());
        assert!(new.join("recordings").is_dir());
    }

    #[test]
    fn failed_copy_leaves_handys_folder_in_place() {
        let (root, old, _) = handy_folder();
        // Legato's folder can't be created: its parent is missing.
        let new = root.path().join("missing").join("new");
        let mut system = Fake {
            rename_errors: errors(1, || io::ErrorKind::CrossesDevices.into()),
            ..Fake::default()
        };

        let outcome = move_folder(&mut system, Folder::Roaming, &old, &new);
        assert!(matches!(outcome, Some(Outcome::CopyFailed(_))));
        assert_left_in_place(&old, &new);
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

    /// Handy's roaming and local folders side by side in one temp dir.
    fn both_folders() -> (tempfile::TempDir, (PathBuf, PathBuf), (PathBuf, PathBuf)) {
        let root = tempfile::tempdir().unwrap();
        let roaming = (
            root.path().join("roaming-old"),
            root.path().join("roaming-new"),
        );
        let local = (root.path().join("local-old"), root.path().join("local-new"));
        fs::create_dir_all(&roaming.0).unwrap();
        fs::write(roaming.0.join("history.db"), "").unwrap();
        fs::create_dir_all(local.0.join("logs")).unwrap();
        (root, roaming, local)
    }

    #[test]
    fn moves_both_folders_and_offers_installed_apps() {
        let (_root, roaming, local) = both_folders();
        let mut system = Fake {
            installed: true,
            wants_installed_apps: true,
            ..Fake::default()
        };

        let notes = migrate(&mut system, Some(roaming.clone()), Some(local.clone()));
        assert!(roaming.1.join("history.db").is_file());
        assert!(local.1.join("logs").is_dir());
        assert_eq!((system.suggested, system.opened), (1, 1));
        assert_eq!(notes.len(), 3);
        assert!(notes.iter().all(|(level, _)| *level == Level::Info));
    }

    #[test]
    fn no_closes_the_uninstall_suggestion() {
        let (_root, roaming, local) = both_folders();
        let mut system = Fake {
            installed: true,
            ..Fake::default()
        };

        migrate(&mut system, Some(roaming), Some(local));
        assert_eq!((system.suggested, system.opened), (1, 0));
    }

    #[test]
    fn suggests_uninstalling_only_after_the_roaming_folder_moved() {
        // Handy is no longer installed.
        let (_root, roaming, local) = both_folders();
        let mut system = Fake::default();
        migrate(&mut system, Some(roaming), Some(local));
        assert_eq!(system.suggested, 0);

        // The user started without Handy's data.
        let (_root, roaming, _) = both_folders();
        let mut system = Fake {
            rename_errors: errors(1, in_use),
            handy_running: true,
            choices: [Choice::Cancel].into(),
            installed: true,
            ..Fake::default()
        };
        migrate(&mut system, Some(roaming), None);
        assert_eq!(system.suggested, 0);

        // Only the local folder had anything to move.
        let (_root, (roaming_old, roaming_new), local) = both_folders();
        fs::remove_dir_all(&roaming_old).unwrap();
        let mut system = Fake {
            installed: true,
            ..Fake::default()
        };
        migrate(&mut system, Some((roaming_old, roaming_new)), Some(local));
        assert_eq!(system.suggested, 0);
    }

    #[test]
    fn notes_when_installed_apps_wont_open() {
        let (_root, roaming, _) = both_folders();
        let mut system = Fake {
            installed: true,
            wants_installed_apps: true,
            installed_apps_fail: true,
            ..Fake::default()
        };

        let notes = migrate(&mut system, Some(roaming), None);
        let (level, note) = notes.last().unwrap();
        assert_eq!(*level, Level::Warn);
        assert!(note.contains("Installed apps"));
    }
}
