//! Learn from my edits: after a paste, reads the field once, ten seconds
//! later, and proposes the words that were corrected. The field's text stays
//! in memory and is dropped after the comparison.

use crate::context::AppInfo;
use crate::learn_rules;
use crate::managers::audio::AudioRecordingManager;
use crate::managers::dictionary::DictionaryManager;
use crate::managers::dictionary_store::DictionarySource;
use crate::settings::{get_settings, LearnMode};
use crate::transforms::BarNotice;
use log::{debug, error, info};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};

/// How long after a paste the field is read.
#[cfg_attr(not(windows), allow(dead_code))]
const READ_AFTER: std::time::Duration = std::time::Duration::from_secs(10);
/// Longest field text read.
#[cfg_attr(not(windows), allow(dead_code))]
const MAX_CHARS: i32 = 20_000;

#[cfg_attr(not(windows), allow(dead_code))]
struct Job {
    app: AppHandle,
    pasted: String,
    target: Option<AppInfo>,
}

static READER: Mutex<Option<Sender<Job>>> = Mutex::new(None);

/// Starts watching the field that has focus, which just received `pasted`.
pub fn watch(app: &AppHandle, pasted: &str, target: Option<&AppInfo>) {
    if !get_settings(app).learn_from_edits || pasted.split_whitespace().count() == 0 {
        return;
    }
    let job = Job {
        app: app.clone(),
        pasted: pasted.to_string(),
        target: target.cloned(),
    };
    let mut reader = READER.lock().unwrap_or_else(|e| e.into_inner());
    if reader.is_none() {
        *reader = start();
    }
    if let Some(tx) = reader.as_ref() {
        if tx.send(job).is_err() {
            *reader = None;
        }
    }
}

#[cfg(windows)]
fn start() -> Option<Sender<Job>> {
    let (tx, rx) = std::sync::mpsc::channel::<Job>();
    std::thread::Builder::new()
        .name("edit-reader".into())
        .spawn(move || uia::run(rx))
        .map_err(|e| error!("Couldn't start the edit reader: {}", e))
        .ok()?;
    Some(tx)
}

#[cfg(not(windows))]
fn start() -> Option<Sender<Job>> {
    None
}

/// What happens to the words found in one field.
#[cfg_attr(not(windows), allow(dead_code))]
fn propose(job: &Job, field: &str) {
    let settings = get_settings(&job.app);
    if !settings.learn_from_edits {
        return;
    }
    let edits = learn_rules::find_edits(&job.pasted, field, settings.word_correction_threshold);
    if edits.is_empty() {
        return;
    }
    let Some(dictionary) = job.app.try_state::<Arc<DictionaryManager>>() else {
        return;
    };
    let app_name = job.target.as_ref().map(|t| t.name.clone());
    let fix_app = job
        .target
        .as_ref()
        .map(|t| (t.name.as_str(), Some(t.exe_path.as_str())));
    debug!("Found {} corrected word(s) after a paste", edits.len());
    let notice = match settings.learn_mode {
        LearnMode::Ask => {
            let words: Vec<(String, String, String)> = edits
                .into_iter()
                .map(|e| (e.word, e.heard, e.context))
                .collect();
            match dictionary.add_pending(&words, fix_app) {
                Ok(added) if !added.is_empty() => Some(BarNotice::Spotted {
                    word: added[0].word.clone(),
                    more: added.len() - 1,
                    app: app_name,
                }),
                Ok(_) => None,
                Err(e) => {
                    error!("Couldn't save words for review: {}", e);
                    None
                }
            }
        }
        LearnMode::Auto => {
            let mut learned = Vec::new();
            for edit in edits {
                if dictionary.is_known(&edit.word, &edit.heard).unwrap_or(true) {
                    continue;
                }
                match dictionary.learn(
                    &edit.word,
                    Some(&edit.heard),
                    DictionarySource::Edited,
                    false,
                    fix_app,
                ) {
                    Ok(l) => learned.push(l),
                    Err(e) => error!("Couldn't learn a corrected word: {}", e),
                }
            }
            let first = learned.first().map(|l| l.entry.text.clone());
            first.map(|word| {
                let more = learned.len() - 1;
                crate::transforms::offer_unlearn(learned);
                BarNotice::Learned {
                    word,
                    more,
                    app: app_name,
                }
            })
        }
    };
    let Some(notice) = notice else {
        return;
    };
    info!("Learned from an edit");
    // Never cover a dictation that started meanwhile.
    let recording = job
        .app
        .try_state::<Arc<AudioRecordingManager>>()
        .is_some_and(|rm| rm.is_recording());
    if !recording {
        crate::overlay::show_overlay_notice_for(&job.app, &notice, 5000);
    }
}

#[cfg(windows)]
mod uia {
    use super::{propose, Job, MAX_CHARS, READ_AFTER};
    use log::{debug, warn};
    use std::sync::mpsc::{Receiver, RecvTimeoutError};
    use std::time::Instant;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
    };
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTextPattern,
        IUIAutomationValuePattern, UIA_TextPatternId, UIA_ValuePatternId,
    };

    struct Watch {
        job: Job,
        field: IUIAutomationElement,
        due: Instant,
    }

    /// The reader thread: takes the focused field at each paste and reads
    /// it when its time comes. COM objects never leave this thread.
    pub fn run(rx: Receiver<Job>) {
        // SAFETY: COM is initialised once for this thread, which owns every
        // UI Automation object it creates.
        let automation: IUIAutomation = unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            match CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) {
                Ok(a) => a,
                Err(e) => {
                    warn!("UI Automation isn't available: {}", e);
                    return;
                }
            }
        };
        let mut watches: Vec<Watch> = Vec::new();
        loop {
            let wait = watches
                .iter()
                .map(|w| w.due.saturating_duration_since(Instant::now()))
                .min()
                .unwrap_or(std::time::Duration::from_secs(3600));
            match rx.recv_timeout(wait) {
                Ok(job) => {
                    if let Some(field) = focused(&automation) {
                        watches.push(Watch {
                            job,
                            field,
                            due: Instant::now() + READ_AFTER,
                        });
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            let now = Instant::now();
            let (ready, waiting): (Vec<Watch>, Vec<Watch>) =
                watches.into_iter().partition(|w| w.due <= now);
            watches = waiting;
            for w in ready {
                match read(&w.field) {
                    Some(text) => propose(&w.job, &text),
                    None => debug!("The field Legato pasted into can't be read"),
                }
            }
        }
    }

    /// The focused element, unless it's a password field or Legato's own.
    fn focused(automation: &IUIAutomation) -> Option<IUIAutomationElement> {
        // SAFETY: plain UI Automation calls on objects owned by this thread.
        unsafe {
            let field = automation.GetFocusedElement().ok()?;
            if field.CurrentIsPassword().ok()?.as_bool() {
                debug!("Not watching a password field");
                return None;
            }
            if field.CurrentProcessId().ok()? as u32 == std::process::id() {
                return None;
            }
            Some(field)
        }
    }

    /// The field's text: its value if it has one, else its document.
    fn read(field: &IUIAutomationElement) -> Option<String> {
        // SAFETY: as above; the element may be gone, which returns an error.
        unsafe {
            if let Ok(value) =
                field.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
            {
                if let Ok(text) = value.CurrentValue() {
                    let text = text.to_string();
                    if !text.is_empty() {
                        return Some(text.chars().take(MAX_CHARS as usize).collect());
                    }
                }
            }
            let pattern = field
                .GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId)
                .ok()?;
            let text = pattern.DocumentRange().ok()?.GetText(MAX_CHARS).ok()?;
            Some(text.to_string())
        }
    }
}
