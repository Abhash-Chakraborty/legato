//! First-run setup: the microphone check, the input languages that pick a
//! speech model, and the practice. While a practice scene is open, a
//! dictation lands in the scene instead of the app in front; on the other
//! setup steps the shortcut only says where to try it, so nothing is pasted
//! behind the setup window.

use crate::audio_toolkit::text::DictionaryCorrection;
use crate::cleanup_rules::{self, CleanupLevel};
use crate::context::{AppCategory, AppInfo};
use crate::managers::dictionary::applied_fixes;
use crate::managers::history_store::AppliedFix;
use crate::settings::get_settings;
use crate::style_rules::{self, Tone};
use log::{debug, warn};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tauri_specta::Event;

/// Where a dictation goes during setup.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum PracticeMode {
    /// Setup isn't open: dictation works as usual.
    #[default]
    Off,
    /// A setup step without a scene: the shortcut shows a hint.
    Hint,
    /// A chat scene, styled as personal messages.
    Chat,
    /// An email scene.
    Email,
    /// A document scene.
    Document,
}

impl PracticeMode {
    fn category(self) -> Option<AppCategory> {
        match self {
            PracticeMode::Chat => Some(AppCategory::Personal),
            PracticeMode::Email => Some(AppCategory::Email),
            PracticeMode::Document => Some(AppCategory::Documents),
            PracticeMode::Off | PracticeMode::Hint => None,
        }
    }
}

/// What a practice dictation is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum PracticePhase {
    Recording,
    Transcribing,
    /// The cancel key stopped it.
    Cancelled,
    Idle,
}

#[derive(Serialize, Clone, Debug, Type, tauri_specta::Event)]
pub struct PracticeStatus {
    pub phase: PracticePhase,
}

/// A practice dictation: what the model heard, what Legato would have
/// pasted, and what each step changed on the way.
#[derive(Serialize, Clone, Debug, Type, tauri_specta::Event)]
pub struct PracticeResult {
    pub scene: PracticeMode,
    pub heard: String,
    pub text: String,
    /// Filler words removed, each once.
    pub fillers: Vec<String>,
    /// The Dictionary's fixes, then the AI's.
    pub fixes: Vec<AppliedFix>,
    /// Cleanup with self-corrections left in, and with them taken out, so
    /// the scene can show what a correction dropped. Equal when nothing was.
    pub uncorrected: String,
    pub corrected: String,
    /// The Style tone used, or None with Style off.
    pub tone: Option<Tone>,
    /// How long transcribing took.
    pub ms: u32,
}

/// The shortcut was pressed on a setup step that has no scene.
#[derive(Serialize, Clone, Debug, Type, tauri_specta::Event)]
pub struct PracticeHint;

/// How loud the microphone is during the check, from 0 to 1.
#[derive(Serialize, Clone, Debug, Type, tauri_specta::Event)]
pub struct MicCheckLevel {
    pub level: f32,
}

static MODE: Lazy<Mutex<PracticeMode>> = Lazy::new(|| Mutex::new(PracticeMode::Off));

pub fn mode() -> PracticeMode {
    *MODE.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn set_mode(mode: PracticeMode) {
    *MODE.lock().unwrap_or_else(|e| e.into_inner()) = mode;
}

/// Whether the shortcut should only show the hint.
pub fn hint_only() -> bool {
    mode() == PracticeMode::Hint
}

/// The app a practice dictation is styled for, while a scene is open.
pub fn scene_target() -> Option<AppInfo> {
    let category = mode().category()?;
    Some(AppInfo {
        exe_path: String::new(),
        exe: String::new(),
        name: String::new(),
        title: String::new(),
        site: None,
        category,
    })
}

pub fn status(app: &AppHandle, phase: PracticePhase) {
    if scene_target().is_some() {
        let _ = PracticeStatus { phase }.emit(app);
    }
}

static HINTED: AtomicBool = AtomicBool::new(false);

/// On a setup step without a scene, a dictation start only shows the hint;
/// true when it did, and the matching stop is then skipped.
pub fn intercept_start(app: &AppHandle) -> bool {
    if !hint_only() {
        return false;
    }
    HINTED.store(true, Ordering::SeqCst);
    let _ = PracticeHint.emit(app);
    true
}

/// Whether the stop belongs to a start that only showed the hint.
pub fn take_hinted() -> bool {
    HINTED.swap(false, Ordering::SeqCst)
}

/// The cancel key stopped whatever practice dictation was running.
pub fn cancelled(app: &AppHandle) {
    HINTED.store(false, Ordering::SeqCst);
    status(app, PracticePhase::Cancelled);
}

/// Runs a practice dictation through the same rules as a real one, for the
/// scene's kind of app, and shows it in the scene instead of pasting it.
/// Nothing is saved to history and the Dictionary counts no hits.
pub async fn finish(
    app: &AppHandle,
    target: &AppInfo,
    heard: String,
    text: String,
    fillers: Vec<(String, i64)>,
    fixes: &[DictionaryCorrection],
    ms: u32,
) {
    if text.trim().is_empty() {
        return;
    }
    let processed = crate::actions::process_transcription_output(
        app,
        &text,
        crate::actions::DictationMode::Plain,
        Some(target),
        &crate::actions::no_stage,
    )
    .await;
    let settings = get_settings(app);
    let category = target.category.as_str();
    let tone = settings
        .style_enabled
        .then(|| settings.styles.get(category).tone);
    let (uncorrected, corrected) = if settings.cleanup_level == CleanupLevel::None {
        (text.clone(), text.clone())
    } else {
        let rules = style_rules::cleanup_rules_for(category, settings.cleanup_rules);
        let lists = style_rules::list_style(category, tone.unwrap_or(Tone::Raw), rules.lists);
        let without = cleanup_rules::CleanupRules {
            corrections: false,
            ..rules
        };
        (
            cleanup_rules::light(&text, &without, lists),
            cleanup_rules::light(&text, &rules, lists),
        )
    };
    let mut all_fixes = applied_fixes(fixes);
    all_fixes.extend(processed.ai_fixes.iter().map(|(from, to)| AppliedFix {
        from: from.clone(),
        to: to.clone(),
        source: crate::managers::history_store::FixSource::Ai,
    }));
    let _ = PracticeResult {
        scene: mode(),
        heard,
        text: processed.final_text,
        fillers: fillers.into_iter().map(|(word, _)| word).collect(),
        fixes: all_fixes,
        uncorrected,
        corrected,
        tone,
        ms,
    }
    .emit(app);
}

/// Says the practice dictation is over, however its task ends.
pub struct PracticeGuard(pub AppHandle);

impl Drop for PracticeGuard {
    fn drop(&mut self) {
        status(&self.0, PracticePhase::Idle);
    }
}

/// Windows input languages, as tags such as "en-US" and "de-DE", in the
/// order the language bar lists them.
#[cfg(windows)]
pub fn input_languages() -> Vec<String> {
    use windows::Win32::Globalization::{LCIDToLocaleName, LOCALE_ALLOW_NEUTRAL_NAMES};
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyboardLayoutList, HKL};

    // SAFETY: the first call asks for the count; the second fills a buffer
    // of that size.
    let layouts = unsafe {
        let count = GetKeyboardLayoutList(None);
        if count <= 0 {
            return Vec::new();
        }
        let mut list = vec![HKL::default(); count as usize];
        let filled = GetKeyboardLayoutList(Some(&mut list));
        list.truncate(filled.max(0) as usize);
        list
    };
    let mut tags: Vec<String> = Vec::new();
    for layout in layouts {
        // The low word of a layout handle is its language.
        let lcid = (layout.0 as usize & 0xFFFF) as u32;
        let mut buf = [0u16; 85];
        // SAFETY: the buffer is LOCALE_NAME_MAX_LENGTH long.
        let len = unsafe { LCIDToLocaleName(lcid, Some(&mut buf), LOCALE_ALLOW_NEUTRAL_NAMES) };
        if len > 1 {
            let tag = String::from_utf16_lossy(&buf[..len as usize - 1]);
            if !tags.contains(&tag) {
                tags.push(tag);
            }
        }
    }
    tags
}

#[cfg(not(windows))]
pub fn input_languages() -> Vec<String> {
    Vec::new()
}

/// Root mean square of a chunk, mapped from -60..0 dBFS to 0..1.
fn loudness(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let rms = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
    let db = 20.0 * rms.max(1e-6).log10();
    ((db + 60.0) / 60.0).clamp(0.0, 1.0)
}

struct LevelSink {
    app: AppHandle,
    last: Instant,
    peak: f32,
}

impl handy_recorder::Sink for LevelSink {
    fn process_chunk(&mut self, chunk: handy_recorder::AudioChunk<'_>) {
        let frames = chunk.valid_frames * chunk.channels.max(1) as usize;
        let level = loudness(&chunk.samples[..frames.min(chunk.samples.len())]);
        self.peak = self.peak.max(level);
        // About 20 updates a second is plenty for a meter.
        if self.last.elapsed() >= Duration::from_millis(50) {
            let _ = MicCheckLevel { level: self.peak }.emit(&self.app);
            self.peak = 0.0;
            self.last = Instant::now();
        }
    }
}

static CHECK: Lazy<Mutex<Option<mpsc::Sender<()>>>> = Lazy::new(|| Mutex::new(None));

/// Opens the chosen microphone on its own, without recording anything, and
/// reports its level until `stop_mic_check`.
pub fn start_mic_check(app: &AppHandle) {
    stop_mic_check();
    let (tx, rx) = mpsc::channel::<()>();
    *CHECK.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);
    let app = app.clone();
    std::thread::spawn(move || {
        let wanted = crate::settings::get_settings(&app).selected_microphone;
        let device = wanted.and_then(|name| {
            handy_recorder::list_input_devices()
                .ok()?
                .into_iter()
                .find(|d| d.name == name)
                .map(|d| d.id)
        });
        let config = handy_recorder::RecorderConfig {
            device,
            ..handy_recorder::RecorderConfig::speech()
        };
        let recorder = match handy_recorder::Recorder::open(config) {
            Ok(r) => r,
            Err(e) => {
                warn!("Microphone check couldn't open the microphone: {e}");
                return;
            }
        };
        let sink = LevelSink {
            app: app.clone(),
            last: Instant::now(),
            peak: 0.0,
        };
        if let Err(e) = recorder.start(sink) {
            warn!("Microphone check couldn't start: {}", e.error);
            return;
        }
        debug!("Microphone check started");
        // Stops when asked, or when the sender is dropped by a new check.
        let _ = rx.recv();
        let _ = recorder.stop();
        let _ = recorder.close();
        debug!("Microphone check stopped");
    });
}

pub fn stop_mic_check() {
    if let Some(tx) = CHECK.lock().unwrap_or_else(|e| e.into_inner()).take() {
        let _ = tx.send(());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loudness_maps_decibels() {
        assert_eq!(loudness(&[]), 0.0);
        assert_eq!(loudness(&[0.0; 10]), 0.0);
        assert!((loudness(&[1.0; 10]) - 1.0).abs() < 1e-6);
        // -30 dBFS is half way.
        let half = 10f32.powf(-30.0 / 20.0);
        assert!((loudness(&[half; 10]) - 0.5).abs() < 0.01);
    }

    #[test]
    fn only_scenes_have_a_target() {
        set_mode(PracticeMode::Hint);
        assert!(hint_only());
        assert!(!take_hinted());
        assert!(scene_target().is_none());
        set_mode(PracticeMode::Email);
        assert_eq!(scene_target().map(|t| t.category), Some(AppCategory::Email));
        set_mode(PracticeMode::Off);
        assert!(!hint_only());
    }
}
