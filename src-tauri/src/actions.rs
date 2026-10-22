use crate::ai::{self, AiProblem, AiStatusKind};
use crate::audio_feedback::{play_feedback_sound, play_feedback_sound_blocking, SoundType};
use crate::audio_toolkit::text::DictionaryCorrection;
use crate::audio_toolkit::VadPolicy;
use crate::context::{self, AppInfo, DictationContext};
use crate::managers::audio::{recording_error_type, AudioRecordingManager};
use crate::managers::dictionary::{applied_fixes, DictionaryManager};
use crate::managers::history::HistoryManager;
use crate::managers::history_store::{
    AppFields, AppliedFix, DictationStatus, FixSource, NewEntry, PasteRecord,
};
use crate::managers::model::ModelManager;
use crate::managers::transcription::policy::{live_decode, LiveDecode, ModelTraits};
use crate::managers::transcription::TranscriptionManager;
use crate::managers::transcription::{FinishedText, StreamWorkKind};
use crate::settings::{get_settings, AppSettings, CleanupLevel, OverlayStyle, PasteMethod};
use crate::shortcut;
use crate::tray::{set_tray_state, TrayIconState};
use crate::utils::{
    self, show_processing_overlay, show_recording_overlay, show_transcribing_overlay,
};
use crate::TranscriptionCoordinator;
use crate::{cleanup_rules, snippet_rules, style_rules};
use log::{debug, error, warn};
use once_cell::sync::Lazy;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::Manager;
use tauri::{AppHandle, Emitter};
use tauri_plugin_clipboard_manager::ClipboardExt;

const CANCELLATION_POLL_INTERVAL: Duration = Duration::from_millis(25);

#[derive(Clone, serde::Serialize)]
struct RecordingErrorEvent {
    error_type: String,
    detail: Option<String>,
}

/// Frees the shortcut for the next dictation: once the speech is
/// transcribed (`release`), so the AI step and the paste go on in the
/// background, or at the latest when the task ends, early exits included.
struct FinishGuard(AppHandle, Arc<TranscriptionManager>, AtomicBool);
impl FinishGuard {
    fn release(&self) {
        // Only once: a second drain would end the next dictation's
        // recording in the coordinator.
        if self.2.swap(true, Ordering::SeqCst) {
            return;
        }
        self.1.maybe_unload_immediately("transcription session");
        if let Some(c) = self.0.try_state::<TranscriptionCoordinator>() {
            c.notify_processing_finished();
        }
    }
}
impl Drop for FinishGuard {
    fn drop(&mut self) {
        self.release();
    }
}

/// Counts dictations as they start. One still finishing in the background
/// when the next starts leaves the bar, the bubble and the tray to it.
static DICTATION_SESSION: AtomicU64 = AtomicU64::new(0);

fn is_current(session: u64) -> bool {
    DICTATION_SESSION.load(Ordering::SeqCst) == session
}

/// Ends a dictation's bar, bubble and tray, unless a newer one has them.
fn end_session(app: &AppHandle, session: u64) {
    if is_current(session) {
        utils::end_dictation(app);
        set_tray_state(app, TrayIconState::Idle);
    }
}

/// Dictations paste in the order they were spoken, even when a later one's
/// AI step answers first. Each takes a turn when it stops.
static NEXT_TURN: AtomicU64 = AtomicU64::new(0);
static TURNS_DONE: Mutex<BTreeSet<u64>> = Mutex::new(BTreeSet::new());
static TURN_NOW: once_cell::sync::Lazy<tokio::sync::watch::Sender<u64>> =
    once_cell::sync::Lazy::new(|| tokio::sync::watch::Sender::new(0));

/// A dictation's place in the paste order; dropping it, once the text is
/// out or the dictation ended without any, lets the next one go.
struct PasteTurn(u64);
impl PasteTurn {
    fn take() -> Self {
        Self(NEXT_TURN.fetch_add(1, Ordering::SeqCst))
    }

    /// Whether every earlier dictation has finished.
    fn ready(&self) -> bool {
        *TURN_NOW.borrow() >= self.0
    }

    /// Waits for every earlier dictation to finish.
    async fn wait(&self) {
        let mut now = TURN_NOW.subscribe();
        let _ = now.wait_for(|n| *n >= self.0).await;
    }
}
impl Drop for PasteTurn {
    fn drop(&mut self) {
        let mut done = TURNS_DONE.lock().unwrap_or_else(|e| e.into_inner());
        done.insert(self.0);
        let mut now = *TURN_NOW.borrow();
        while done.remove(&now) {
            now += 1;
        }
        TURN_NOW.send_replace(now);
    }
}

/// What a dictation is doing after you stop talking, named on the bar so a
/// slow step doesn't look stuck.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stage {
    /// The AI is checking for misheard Dictionary words.
    Fixing,
    /// The AI is cleaning up the text (Cleanup at Medium).
    Polishing,
    /// A transform is rewriting the text.
    Transforming,
    /// An earlier dictation is still being finished and pastes first.
    Waiting,
    Pasting,
}

impl Stage {
    fn key(self) -> &'static str {
        match self {
            Stage::Fixing => "fixing",
            Stage::Polishing => "polishing",
            Stage::Transforming => "transforming",
            Stage::Waiting => "waiting",
            Stage::Pasting => "pasting",
        }
    }
}

/// For callers whose dictation has no bar to tell.
pub(crate) fn no_stage(_: Stage) {}

// Shortcut Action Trait
pub trait ShortcutAction: Send + Sync {
    fn start(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str);
    fn stop(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str);
}

/// What a dictation shortcut does with the text.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum DictationMode {
    /// Dictate: pasted, after the transform set to run after every
    /// dictation, if any.
    Plain,
    /// Dictate then transform.
    Transform,
    /// Ask: the words are an instruction for the selection.
    Ask,
    /// Into the Scratchpad instead of the app in front.
    Scratchpad,
}

// Transcribe Action
struct TranscribeAction {
    mode: DictationMode,
}

/// Returns `true` when a transcription has no meaningful content to
/// post-process (empty or whitespace-only). Used to skip the post-processing
/// LLM call when nothing was actually transcribed, which would otherwise make
/// the model reply with an error message such as "you need to provide the
/// transcription".
fn is_blank_transcription(transcription: &str) -> bool {
    transcription.trim().is_empty()
}

async fn complete_unless_cancelled<F, C>(operation: F, is_cancelled: C) -> Option<F::Output>
where
    F: Future,
    C: Fn() -> bool,
{
    tokio::pin!(operation);

    loop {
        if is_cancelled() {
            return None;
        }

        if let Ok(result) =
            tokio::time::timeout(CANCELLATION_POLL_INTERVAL, operation.as_mut()).await
        {
            return Some(result);
        }
    }
}

fn should_use_streaming_overlay(style: OverlayStyle, is_streaming: bool) -> bool {
    style == OverlayStyle::Live && is_streaming
}

/// History's app for a dictation into the Scratchpad.
fn scratchpad_fields() -> AppFields {
    AppFields {
        name: "Scratchpad".to_string(),
        category: crate::context::AppCategory::Other.as_str().to_string(),
        ..Default::default()
    }
}

fn app_fields(info: AppInfo) -> AppFields {
    AppFields {
        exe: info.exe,
        path: info.exe_path,
        name: info.name,
        title: info.title,
        site: info.site,
        category: info.category.as_str().to_string(),
    }
}

pub(crate) struct ProcessedTranscription {
    /// The transcript after the correction pass, snippets, cleanup and Style.
    pub transcription: String,
    /// Words the correction pass fixed, as (heard text, Dictionary word).
    pub ai_fixes: Vec<(String, String)>,
    pub final_text: String,
    pub post_processed_text: Option<String>,
    pub post_process_prompt: Option<String>,
    /// Name of the prompt that rewrote the text.
    pub transform: Option<String>,
    /// The transform that ran, for the bar and Recent runs.
    pub transform_run: Option<crate::transforms::DictationTransform>,
    /// An AI step that was skipped because the provider didn't answer.
    pub ai_problem: Option<AiProblem>,
    /// Snippets the dictation pasted.
    pub snippet_hits: Vec<i64>,
}

impl ProcessedTranscription {
    /// The fixes to save with the dictation: the Dictionary's, then the AI's.
    pub fn all_fixes(&self, rule_fixes: &[DictionaryCorrection]) -> Vec<AppliedFix> {
        let mut fixes = applied_fixes(rule_fixes);
        fixes.extend(self.ai_fixes.iter().map(|(from, to)| AppliedFix {
            from: from.clone(),
            to: to.clone(),
            source: FixSource::Ai,
        }));
        fixes
    }
}

/// The transform a dictation runs, if any: Dictate then transform runs the
/// one set to run after every dictation, or else the Dictate then transform
/// choice; a plain dictation runs the former unless its skips apply.
fn dictation_transform(
    settings: &AppSettings,
    mode: DictationMode,
    category: Option<&str>,
    text: &str,
) -> Option<crate::settings::Transform> {
    if !settings.post_process_enabled || is_blank_transcription(text) {
        return None;
    }
    let auto = settings
        .auto_transform_id
        .as_deref()
        .and_then(|id| settings.transform(id))
        .filter(|t| t.enabled);
    match mode {
        DictationMode::Transform => auto
            .or_else(|| settings.transform(&settings.dictate_transform_id))
            .cloned(),
        DictationMode::Plain => {
            let rules = crate::transform_rules::AutoRules {
                skip_code: settings.auto_transform_skip_code,
                skip_ai: settings.auto_transform_skip_ai,
                min_words: settings.auto_transform_min_words,
            };
            auto.filter(|_| {
                crate::transform_rules::auto_applies(
                    &rules,
                    category,
                    crate::transform_rules::word_count(text),
                )
            })
            .cloned()
        }
        DictationMode::Ask | DictationMode::Scratchpad => None,
    }
}

/// Whether a dictation goes to the AI before it's pasted, so the bar can
/// say it's polishing rather than leave the transcribing spinner up.
fn polishes(
    settings: &AppSettings,
    text: &str,
    mode: DictationMode,
    category: Option<&str>,
) -> bool {
    let ai = ai::AiConfig::from_settings(settings).is_some();
    let words = text.split_whitespace().count() as u32;
    (ai && settings.cleanup_level == CleanupLevel::Medium)
        || (ai && settings.ai_correction_enabled && words >= settings.ai_min_words)
        || dictation_transform(settings, mode, category, text).is_some()
}

pub(crate) async fn process_transcription_output(
    app: &AppHandle,
    transcription: &str,
    mode: DictationMode,
    target: Option<&AppInfo>,
    stage: &(dyn Fn(Stage) + Sync),
) -> ProcessedTranscription {
    let settings = get_settings(app);
    let mut text = transcription.to_string();
    let mut ai_fixes = Vec::new();
    let mut ai_problem = None;
    let category = target.map(|a| a.category.as_str()).unwrap_or("other");

    // With Cleanup at Medium the correction pass rides along in its request,
    // so a dictation waits for one round trip, not two.
    let medium = settings.cleanup_level == CleanupLevel::Medium
        && ai::AiConfig::from_settings(&settings).is_some();
    if !medium {
        if settings.ai_correction_enabled
            && text.split_whitespace().count() as u32 >= settings.ai_min_words
            && ai::AiConfig::from_settings(&settings).is_some()
        {
            stage(Stage::Fixing);
        }
        match ai::correction_pass(app, &settings, &text).await {
            Ok(Some(corrected)) => {
                text = corrected.text;
                ai_fixes = corrected.changes;
            }
            Ok(None) => {}
            Err(problem) => ai_problem = Some(problem),
        }
    }

    // Snippets become placeholders, so cleanup, the AI and Style never touch
    // the saved text.
    let snippets = crate::managers::snippets::snippet_set();
    let marked = snippet_rules::mark(&text, &snippets.rules);
    let mut styled = marked.text.clone();
    if !marked.whole {
        let tone = if settings.style_enabled {
            settings.styles.get(category).tone
        } else {
            style_rules::Tone::Raw
        };
        if settings.cleanup_level != CleanupLevel::None {
            let lists = style_rules::list_style(category, tone, settings.cleanup_rules.lists);
            let rules = style_rules::cleanup_rules_for(category, settings.cleanup_rules);
            styled = cleanup_rules::light(&styled, &rules, lists);
        }
        if medium {
            stage(Stage::Polishing);
            match ai::medium_pass(app, &settings, &styled, marked.hits.len()).await {
                Ok(Some(cleaned)) => styled = cleaned,
                Ok(None) => {}
                Err(problem) => ai_problem = Some(problem),
            }
        }
        styled = style_rules::apply(&styled, category, tone);
    }
    let text = if marked.hits.is_empty() {
        styled
    } else {
        let wanted = |test: fn(&str) -> bool| {
            marked
                .hits
                .iter()
                .any(|id| snippets.text_of(*id).is_some_and(test))
        };
        let vars = snippet_rules::Variables {
            now: chrono::Local::now(),
            clipboard: wanted(snippet_rules::uses_clipboard)
                .then(|| app.clipboard().read_text().ok())
                .flatten(),
            selection: wanted(snippet_rules::uses_selection)
                .then(|| crate::selection::copy_selection(app).ok().flatten())
                .flatten(),
        };
        snippet_rules::unmark(&styled, &marked.hits, |id| {
            snippet_rules::fill(snippets.text_of(id).unwrap_or_default(), &vars)
        })
    };
    let snippet_hits = marked.hits;

    let mut final_text = text.clone();
    let mut post_processed_text: Option<String> = None;
    let mut post_process_prompt: Option<String> = None;
    let mut transform: Option<String> = None;
    let mut transform_run = None;

    if let Some(chosen) =
        dictation_transform(&settings, mode, target.map(|_| category), &final_text)
    {
        stage(Stage::Transforming);
        let started = Instant::now();
        let timeout = crate::ai::dictation_timeout(&settings);
        let app_name = target.map(|a| a.name.as_str());
        match crate::transforms::apply(app, &settings, &chosen, &final_text, app_name, timeout)
            .await
        {
            Ok(Some(processed_text)) => {
                transform_run = Some(crate::transforms::DictationTransform {
                    id: chosen.id.clone(),
                    name: chosen.name.clone(),
                    before: final_text.clone(),
                    after: processed_text.clone(),
                    ms: started.elapsed().as_millis() as i64,
                    show_changes: chosen.show_changes,
                });
                post_processed_text = Some(processed_text.clone());
                final_text = processed_text;
                post_process_prompt = Some(chosen.prompt.clone());
                transform = Some(chosen.name.clone());
            }
            Ok(None) => debug!("Transform '{}' answered with nothing", chosen.id),
            // Nothing is set up: the text goes out as dictated, as before
            // transforms existed, with no warning on every dictation.
            Err(problem) if problem.kind == AiStatusKind::None => {}
            Err(problem) => ai_problem = Some(problem),
        }
    }

    ProcessedTranscription {
        transcription: text,
        ai_fixes,
        final_text,
        post_processed_text,
        post_process_prompt,
        transform,
        transform_run,
        ai_problem,
        snippet_hits,
    }
}

impl ShortcutAction for TranscribeAction {
    fn start(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        let start_time = Instant::now();
        debug!("TranscribeAction::start called for binding: {}", binding_id);

        // During setup the shortcut records only in a practice scene.
        if crate::setup::intercept_start(app) {
            return;
        }
        DICTATION_SESSION.fetch_add(1, Ordering::SeqCst);

        // Load model in the background
        let tm = app.state::<Arc<TranscriptionManager>>();
        let rm = app.state::<Arc<AudioRecordingManager>>();

        // The app the text is meant for, which may have its own speech model
        // in Style. Read before the overlay shows, though the overlay never
        // takes focus.
        let front = context::foreground_window();
        let speech_model = {
            let settings = get_settings(app);
            let models = app.state::<Arc<ModelManager>>();
            let category = front.as_ref().map(|w| context::resolve(w).category);
            settings.speech_model_for(category.map(|c| c.as_str()), |id| {
                models.get_model_info(id).is_some_and(|m| m.is_downloaded)
            })
        };

        // Load ASR model and VAD model in parallel
        let kickoff_started = Instant::now();
        tm.initiate_model_load_of(&speech_model);
        crate::ai::warm_up(app);
        let rm_clone = Arc::clone(&rm);
        std::thread::spawn(move || {
            if let Err(e) = rm_clone.preload_vad() {
                debug!("VAD pre-load failed: {}", e);
            }
        });
        let kickoff_elapsed = kickoff_started.elapsed();

        // Don't open the mic if nothing can transcribe the recording; the load
        // kicked off above fails and reports why.
        if !tm.is_model_loaded() {
            if let Err(e) = app
                .state::<Arc<ModelManager>>()
                .get_model_path(&speech_model)
            {
                warn!("Not starting recording: no model can transcribe it ({})", e);
                return;
            }
        }

        let remember_app = get_settings(app).remember_app;
        app.state::<DictationContext>()
            .set(if remember_app { front } else { None });

        let binding_id = binding_id.to_string();
        let tray_started = Instant::now();
        set_tray_state(app, TrayIconState::Recording);
        let tray_elapsed = tray_started.elapsed();

        // Get the microphone mode to determine audio feedback timing
        let plan_started = Instant::now();
        let settings = get_settings(app);
        let is_always_on = settings.always_on_microphone;

        let selected_model_info = app
            .state::<Arc<ModelManager>>()
            .get_model_info(&speech_model);

        // Use the app-facing model capability as the single pre-recording source
        // for live streaming decisions. Unknown support is represented as false
        // until the model registry is updated by discovery or runtime load.
        let model_supports_streaming = selected_model_info
            .as_ref()
            .map(|m| m.supports_streaming)
            .unwrap_or(false);
        let vad_policy = if !settings.vad_enabled {
            VadPolicy::Disabled
        } else if model_supports_streaming {
            VadPolicy::Streaming
        } else {
            VadPolicy::Offline
        };
        // Before the model loads, whether it keeps its accuracy on long audio
        // is unknown, so assume not: the worker decides again with the loaded
        // model, and a wrong guess costs only an idle worker.
        let expected_model = ModelTraits {
            streaming: model_supports_streaming,
            long_form: false,
        };
        if live_decode(expected_model, settings.long_form_chunking) != LiveDecode::None {
            tm.start_stream();
        }
        let plan_elapsed = plan_started.elapsed();

        // Sizing the overlay follows the same advertised capability. A model that
        // doesn't stream (or whose capability is not known yet) gets the compact
        // pill instead of an oversized transparent live window.
        let overlay_started = Instant::now();
        match settings.overlay_style {
            OverlayStyle::Live if model_supports_streaming => utils::show_streaming_overlay(app),
            OverlayStyle::Live | OverlayStyle::Minimal => show_recording_overlay(app),
            // The bar stays off, but the bubble still shows the dictation.
            OverlayStyle::None => show_recording_overlay(app),
        }
        // Everything above runs before capture can begin, so each span here is
        // added keypress->capture latency.
        debug!(
            "start-path pre-recording steps: model_kickoff={:?} tray={:?} settings+stream_plan={:?} overlay={:?}",
            kickoff_elapsed,
            tray_elapsed,
            plan_elapsed,
            overlay_started.elapsed()
        );
        debug!("Microphone mode - always_on: {}", is_always_on);

        let mut recording_error: Option<anyhow::Error> = None;
        let recording_start_time = Instant::now();
        match rm.try_start_recording(&binding_id, vad_policy) {
            Ok(readiness) => {
                debug!(
                    "Recording request accepted in {:?}; waiting for first microphone samples",
                    recording_start_time.elapsed()
                );
                crate::setup::status(app, crate::setup::PracticePhase::Recording);
                let generation = readiness.generation();
                let app_clone = app.clone();
                let rm_clone = Arc::clone(&rm);
                std::thread::spawn(move || {
                    if !readiness.wait() {
                        debug!("Microphone readiness wait ended without receiving samples");
                        return;
                    }

                    // Development-only preview hook for evaluating the brief
                    // arming animation on hardware that normally starts too fast
                    // to make it visible.
                    #[cfg(debug_assertions)]
                    if let Ok(delay_ms) = std::env::var("LEGATO_DEBUG_MIC_READY_DELAY_MS")
                        .unwrap_or_default()
                        .parse::<u64>()
                    {
                        let delay_ms = delay_ms.min(10_000);
                        if delay_ms > 0 {
                            debug!("Delaying microphone-ready cue by {delay_ms}ms for UI preview");
                            std::thread::sleep(Duration::from_millis(delay_ms));
                        }
                    }

                    if !rm_clone.is_recording_readiness_current(generation) {
                        debug!("Microphone became ready for an inactive recording");
                        return;
                    }

                    debug!("Microphone is receiving samples; recording is ready");
                    utils::emit_recording_ready(&app_clone);

                    // The start chime is a readiness cue, so it must follow the
                    // first real input callback rather than Stream::play() or a
                    // fixed delay. The helper returns immediately when feedback
                    // is disabled; mute still follows the same readiness point.
                    if rm_clone.is_recording_readiness_current(generation) {
                        play_feedback_sound_blocking(&app_clone, SoundType::Start);
                    }
                    if rm_clone.is_recording_readiness_current(generation) {
                        rm_clone.apply_mute();
                    }
                });
            }
            Err(e) => {
                debug!("Failed to start recording: {}", e);
                recording_error = Some(e);
            }
        }

        if recording_error.is_none() {
            // Dynamically register the cancel shortcut in a separate task to avoid deadlock
            shortcut::register_cancel_shortcut(app);
        } else {
            // Starting failed (for example due to blocked microphone permissions).
            // Revert UI state so we don't stay stuck in the recording overlay.
            tm.cancel_stream();
            utils::end_dictation(app);
            set_tray_state(app, TrayIconState::Idle);
            if let Some(err) = recording_error {
                let _ = app.emit(
                    "recording-error",
                    RecordingErrorEvent {
                        error_type: recording_error_type(&err).to_string(),
                        detail: Some(format!("{err:#}")),
                    },
                );
            }
        }

        debug!(
            "TranscribeAction::start completed in {:?}",
            start_time.elapsed()
        );
    }

    fn stop(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        // Prevent a slow microphone from emitting a ready event or start chime
        // after the user has already requested stop.
        app.state::<Arc<AudioRecordingManager>>()
            .invalidate_recording_readiness();

        // Unregister the cancel shortcut when transcription stops
        shortcut::unregister_cancel_shortcut(app);

        let stop_time = Instant::now();
        debug!("TranscribeAction::stop called for binding: {}", binding_id);

        let ah = app.clone();
        let rm = Arc::clone(&app.state::<Arc<AudioRecordingManager>>());
        let tm = Arc::clone(&app.state::<Arc<TranscriptionManager>>());
        let hm = Arc::clone(&app.state::<Arc<HistoryManager>>());

        set_tray_state(app, TrayIconState::Transcribing);
        // Stop should give immediate visual feedback. Live streaming can keep
        // the larger panel, but it still switches from listening to a working
        // spinner while the stream finalizes. Non-streaming paths use the
        // compact transcribing pill (None no-ops in show_*).
        let style = get_settings(app).overlay_style;
        // Capture this before finalizing the stream so every later working state
        // targets the same overlay that was shown for this transcription.
        // With the bubble on, the bar's live view is hidden, so its working
        // states would reach nobody and the bubble would keep its recording
        // meter until the paste; the plain states go to the bubble, and to
        // the bar with the step's name when it takes a while.
        let use_streaming_overlay = should_use_streaming_overlay(style, tm.is_streaming())
            && !crate::bubble::takes_dictation(app);
        if use_streaming_overlay {
            tm.emit_stream_working(StreamWorkKind::Transcribing);
        } else {
            show_transcribing_overlay(app);
        }

        // Unmute before playing audio feedback so the stop sound is audible
        rm.remove_mute();

        // Play audio feedback for recording stop
        play_feedback_sound(app, SoundType::Stop);

        let binding_id = binding_id.to_string(); // Clone binding_id for the async task
        let mode = self.mode;
        let post_process = mode == DictationMode::Transform;
        let started_in = app.state::<DictationContext>().take();
        let remember_app = get_settings(app).remember_app;
        let cancel_generation = rm.cancel_generation();
        // A practice scene in setup gets the text instead of the app in front.
        let practice = crate::setup::scene_target();
        crate::setup::status(app, crate::setup::PracticePhase::Transcribing);
        let session = DICTATION_SESSION.load(Ordering::SeqCst);
        let turn = PasteTurn::take();

        tauri::async_runtime::spawn(async move {
            let guard = FinishGuard(ah.clone(), Arc::clone(&tm), AtomicBool::new(false));
            // A cancel stops this dictation only while it's the current one,
            // never one finishing in the background behind the next.
            let cancelled = || rm.was_cancelled_since(cancel_generation) && is_current(session);
            let _practice = practice
                .is_some()
                .then(|| crate::setup::PracticeGuard(ah.clone()));
            debug!(
                "Starting async transcription task for binding: {}",
                binding_id
            );

            let stop_recording_time = Instant::now();
            if let Some(samples) = rm.stop_recording(&binding_id, cancel_generation) {
                debug!(
                    "Recording stopped and samples retrieved in {:?}, sample count: {}",
                    stop_recording_time.elapsed(),
                    samples.len()
                );

                if cancelled() {
                    debug!("Transcription operation cancelled after recording stop");
                    tm.cancel_stream();
                    end_session(&ah, session);
                    return;
                }

                if samples.is_empty() {
                    debug!("Recording produced no audio samples; skipping persistence");
                    // Tear down any streaming worker so its channel doesn't leak
                    // and block the next start_stream.
                    tm.cancel_stream();
                    end_session(&ah, session);
                } else {
                    // Save WAV concurrently with transcription
                    let sample_count = samples.len();
                    // 16 kHz mono; with voice detection on, silence is
                    // already gone, so this is speaking time.
                    let speech_ms = (sample_count / 16) as i64;
                    let file_name = format!("legato-{}.wav", chrono::Utc::now().timestamp());
                    let wav_path = hm.recordings_dir().join(&file_name);
                    let wav_path_for_verify = wav_path.clone();
                    let samples_for_wav = samples.clone();
                    let wav_handle = tauri::async_runtime::spawn_blocking(move || {
                        crate::audio_toolkit::save_wav_file(&wav_path, &samples_for_wav)
                    });

                    // Transcribe concurrently with WAV save: the live decode's
                    // result when it has one (all audio was already fed to it),
                    // otherwise a batch run of the samples. A cancel is surfaced
                    // as an error, so it never starts a batch run.
                    let transcription_time = Instant::now();
                    let transcription_result = tm.transcribe_recording(samples);
                    // For Insights: the wait once you stopped, and where it ran.
                    let transcribe_ms = transcription_time.elapsed().as_millis() as i64;
                    let on_gpu = tm.on_gpu();
                    // The model that did the work, which Style may have picked
                    // for this kind of app.
                    let model_id = tm
                        .get_current_model()
                        .unwrap_or_else(|| get_settings(&ah).selected_model);
                    let model_name = ah
                        .state::<Arc<ModelManager>>()
                        .get_model_info(&model_id)
                        .map(|m| m.name);

                    // Await WAV save and verify
                    let wav_saved = match wav_handle.await {
                        Ok(Ok(())) => {
                            match crate::audio_toolkit::verify_wav_file(
                                &wav_path_for_verify,
                                sample_count,
                            ) {
                                Ok(()) => true,
                                Err(e) => {
                                    error!("WAV verification failed: {}", e);
                                    false
                                }
                            }
                        }
                        Ok(Err(e)) => {
                            error!("Failed to save WAV file: {}", e);
                            false
                        }
                        Err(e) => {
                            error!("WAV save task panicked: {}", e);
                            false
                        }
                    };

                    if cancelled() {
                        debug!("Transcription operation cancelled before output handling");
                        end_session(&ah, session);
                        return;
                    }

                    match transcription_result {
                        Ok(FinishedText {
                            text: transcription,
                            fixes,
                            heard,
                            fillers,
                        }) => {
                            debug!(
                                "Transcription completed in {:?}: '{}'",
                                transcription_time.elapsed(),
                                utils::redact_text(&transcription)
                            );

                            if let Some(target) = practice.as_ref() {
                                let _ = std::fs::remove_file(hm.recordings_dir().join(&file_name));
                                let ms = transcription_time.elapsed().as_millis() as u32;
                                crate::setup::finish(
                                    &ah,
                                    target,
                                    heard,
                                    transcription,
                                    fillers,
                                    &fixes,
                                    ms,
                                )
                                .await;
                                end_session(&ah, session);
                                return;
                            }

                            // Ask: the words are the instruction, not text to paste.
                            if mode == DictationMode::Ask {
                                let _ = std::fs::remove_file(hm.recordings_dir().join(&file_name));
                                end_session(&ah, session);
                                crate::transforms::ask(&ah, transcription);
                                return;
                            }

                            // The Scratchpad isn't an app with a style.
                            let app_info = started_in
                                .as_ref()
                                .filter(|_| mode != DictationMode::Scratchpad)
                                .map(context::resolve);
                            let polishing = post_process
                                || polishes(
                                    &get_settings(&ah),
                                    &transcription,
                                    mode,
                                    app_info.as_ref().map(|a| a.category.as_str()),
                                );
                            if polishing {
                                if use_streaming_overlay {
                                    tm.emit_stream_working(StreamWorkKind::Polishing);
                                } else {
                                    show_processing_overlay(&ah);
                                }
                            }
                            // The speech is text now: the shortcut can start
                            // the next dictation while this one is polished
                            // and pasted.
                            guard.release();
                            // Steps of a dictation still finishing after the
                            // next one started stay off that one's bar.
                            let stage = |s: Stage| {
                                if is_current(session) {
                                    crate::overlay::set_stage(&ah, Some(s.key()));
                                }
                            };
                            let Some(processed) = complete_unless_cancelled(
                                process_transcription_output(
                                    &ah,
                                    &transcription,
                                    mode,
                                    app_info.as_ref(),
                                    &stage,
                                ),
                                &cancelled,
                            )
                            .await
                            else {
                                debug!("Transcription operation cancelled during output handling");
                                end_session(&ah, session);
                                return;
                            };

                            if cancelled() {
                                debug!("Transcription operation cancelled before paste");
                                end_session(&ah, session);
                                return;
                            }

                            // Earlier dictations still polishing paste first.
                            if !turn.ready() {
                                stage(Stage::Waiting);
                            }
                            turn.wait().await;
                            stage(Stage::Pasting);
                            let quiet = !is_current(session);
                            // The next dictation's shortcut may be held: a
                            // Ctrl+V now would reach the app as Win+Ctrl+V
                            // and let go of Ctrl under the hold.
                            if quiet {
                                let deadline = Instant::now() + Duration::from_secs(120);
                                while crate::selection::modifiers_down()
                                    && Instant::now() < deadline
                                {
                                    tokio::time::sleep(Duration::from_millis(20)).await;
                                }
                            }

                            let fixes = processed.all_fixes(&fixes);
                            if !fixes.is_empty() {
                                let words: Vec<&str> =
                                    fixes.iter().map(|f| f.to.as_str()).collect();
                                ah.state::<Arc<DictionaryManager>>().record_fixes(
                                    &words,
                                    app_info
                                        .as_ref()
                                        .map(|a| (a.name.as_str(), Some(a.exe_path.as_str()))),
                                );
                            }

                            ah.state::<Arc<crate::managers::snippets::SnippetManager>>()
                                .record_use(
                                    &processed.snippet_hits,
                                    app_info
                                        .as_ref()
                                        .map(|a| (a.name.as_str(), Some(a.exe_path.as_str()))),
                                );

                            // Save to history if WAV was saved
                            let saved_id = if wav_saved {
                                match hm.save_entry(NewEntry {
                                    file_name,
                                    transcription_text: processed.transcription.clone(),
                                    post_process_requested: post_process,
                                    post_processed_text: processed.post_processed_text.clone(),
                                    post_process_prompt: processed.post_process_prompt.clone(),
                                    transform: processed.transform.clone(),
                                    app: if mode == DictationMode::Scratchpad {
                                        Some(scratchpad_fields())
                                    } else {
                                        app_info.clone().map(app_fields)
                                    },
                                    speech_ms: Some(speech_ms),
                                    model_id: Some(model_id.clone()),
                                    model_name: model_name.clone(),
                                    fixes,
                                    heard_text: Some(heard),
                                    fillers,
                                    snippets: processed.snippet_hits.len() as i64,
                                    transcribe_ms: Some(transcribe_ms),
                                    on_gpu,
                                    ..Default::default()
                                }) {
                                    Ok(entry) => Some(entry.id),
                                    Err(err) => {
                                        error!("Failed to save history entry: {}", err);
                                        None
                                    }
                                }
                            } else {
                                None
                            };

                            if mode == DictationMode::Scratchpad {
                                if let Some(id) = saved_id {
                                    let record = PasteRecord {
                                        status: DictationStatus::Pasted,
                                        error: None,
                                        paste_ms: Some(stop_time.elapsed().as_millis() as i64),
                                        paste_app: None,
                                    };
                                    if let Err(e) = hm.record_paste(id, record) {
                                        error!("Failed to record the paste: {}", e);
                                    }
                                }
                                end_session(&ah, session);
                                crate::scratchpad::receive(&ah, processed.final_text);
                            } else if processed.final_text.is_empty() {
                                end_session(&ah, session);
                            } else {
                                let ah_clone = ah.clone();
                                let paste_time = Instant::now();
                                let final_text = processed.final_text;
                                let words = crate::transform_rules::word_count(&final_text);
                                let ai_problem = processed.ai_problem;
                                let transform_run = processed.transform_run;
                                let rm_for_paste = Arc::clone(&rm);
                                let hm_for_paste = Arc::clone(&hm);
                                let paste_overrides =
                                    crate::clipboard::PasteOverrides::for_category(
                                        &get_settings(&ah),
                                        app_info.as_ref().map(|a| a.category.as_str()),
                                    );
                                let not_pasted = paste_overrides
                                    .paste_method
                                    .unwrap_or(get_settings(&ah).paste_method)
                                    == PasteMethod::None;
                                ah.run_on_main_thread(move || {
                                    if rm_for_paste.was_cancelled_since(cancel_generation) && is_current(session) {
                                        debug!("Transcription operation cancelled before paste");
                                        end_session(&ah_clone, session);
                                        return;
                                    }

                                    // The app in front now, in case focus moved
                                    // while the dictation was transcribed.
                                    let paste_into = if remember_app {
                                        context::foreground_window()
                                    } else {
                                        None
                                    };
                                    // Windows drops input sent to an app
                                    // running as administrator; don't try.
                                    let admin = !not_pasted && context::foreground_is_elevated();
                                    let fallback_text = final_text.clone();
                                    let paste_result = if admin {
                                        let name = app_info
                                            .as_ref()
                                            .map(|a| a.name.clone())
                                            .unwrap_or_else(|| "The app".to_string());
                                        let _ = ah_clone.clipboard().write_text(final_text);
                                        Err(format!(
                                            "{name} runs as administrator, so Windows blocks typing into it from Legato. The text is on the clipboard."
                                        ))
                                    } else {
                                        crate::clipboard::paste_dictation(
                                            final_text,
                                            ah_clone.clone(),
                                            paste_overrides,
                                        )
                                    };
                                    let pasted = paste_result.is_ok() && !not_pasted;
                                    if pasted {
                                        // Learn from my edits reads the field
                                        // again in ten seconds.
                                        crate::edit_reader::watch(
                                            &ah_clone,
                                            &fallback_text,
                                            app_info.as_ref(),
                                        );
                                    }
                                    if let Err(reason) = &paste_result {
                                        let note_id = crate::scratchpad::save_failed_paste(
                                            &ah_clone,
                                            &fallback_text,
                                            app_info.as_ref(),
                                            reason,
                                        );
                                        if !quiet {
                                        crate::overlay::show_overlay_notice(
                                            &ah_clone,
                                            &crate::scratchpad::failed_paste_notice(
                                                app_info.as_ref(),
                                                admin,
                                                note_id,
                                            ),
                                        );
                                        }
                                    }
                                    match &paste_result {
                                        Ok(()) => debug!(
                                            "Text pasted successfully in {:?}",
                                            paste_time.elapsed()
                                        ),
                                        Err(e) => {
                                            error!("Failed to paste transcription: {}", e);
                                            let _ = ah_clone.emit("paste-error", ());
                                        }
                                    }
                                    if let Some(id) = saved_id {
                                        let paste_app = match (&started_in, paste_into) {
                                            (Some(start), Some(now))
                                                if start.exe_path != now.exe_path =>
                                            {
                                                let info = context::resolve(&now);
                                                Some((info.name, info.title))
                                            }
                                            _ => None,
                                        };
                                        let record = match paste_result {
                                            Ok(()) if not_pasted => PasteRecord {
                                                status: DictationStatus::NotPasted,
                                                error: None,
                                                paste_ms: None,
                                                paste_app: None,
                                            },
                                            Ok(()) => PasteRecord {
                                                status: DictationStatus::Pasted,
                                                error: None,
                                                paste_ms: Some(
                                                    stop_time.elapsed().as_millis() as i64
                                                ),
                                                paste_app,
                                            },
                                            Err(e) => PasteRecord {
                                                status: DictationStatus::PasteFailed,
                                                error: Some(e),
                                                paste_ms: None,
                                                paste_app,
                                            },
                                        };
                                        if let Err(e) = hm_for_paste.record_paste(id, record) {
                                            error!("Failed to record the paste: {}", e);
                                        }
                                    }
                                    end_session(&ah_clone, session);
                                    // The bar says what happened: a skipped AI
                                    // step, the transform's changes, or the paste.
                                    crate::transforms::after_dictation_paste(
                                        &ah_clone,
                                        crate::transforms::AfterPaste {
                                            pasted,
                                            words,
                                            app: app_info,
                                            history_id: saved_id,
                                            transform: transform_run,
                                            problem: ai_problem,
                                            quiet,
                                        },
                                    );
                                    // The text is out: the next dictation's
                                    // paste may go.
                                    drop(turn);
                                })
                                .unwrap_or_else(|e| {
                                    error!("Failed to run paste on main thread: {:?}", e);
                                    end_session(&ah, session);
                                });
                            }
                        }
                        Err(err) => {
                            if cancelled() {
                                debug!(
                                    "Transcription operation cancelled after transcription error"
                                );
                                end_session(&ah, session);
                                return;
                            }

                            error!("Transcription failed: {}", err);
                            // Surface the failure to the UI (toast). The full
                            // message is also in legato.log via the line above.
                            let _ = ah.emit("transcription-error", err.to_string());
                            // Save entry with empty text so user can retry
                            if wav_saved && mode != DictationMode::Ask {
                                let app_info = started_in.as_ref().map(context::resolve);
                                if let Err(save_err) = hm.save_entry(NewEntry {
                                    file_name,
                                    post_process_requested: post_process,
                                    app: app_info.map(app_fields),
                                    speech_ms: Some(speech_ms),
                                    model_id: Some(model_id.clone()),
                                    model_name: model_name.clone(),
                                    status: Some(DictationStatus::Failed),
                                    ..Default::default()
                                }) {
                                    error!("Failed to save failed history entry: {}", save_err);
                                }
                            }
                            end_session(&ah, session);
                        }
                    }
                }
            } else {
                debug!("No samples retrieved from recording stop");
                // Tear down any streaming worker so its channel doesn't leak.
                tm.cancel_stream();
                end_session(&ah, session);
            }
        });

        debug!(
            "TranscribeAction::stop completed in {:?}",
            stop_time.elapsed()
        );
    }
}

// Cancel Action
struct CancelAction;

impl ShortcutAction for CancelAction {
    fn start(&self, app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        utils::cancel_current_operation(app);
    }

    fn stop(&self, _app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        // Nothing to do on stop for cancel
    }
}

// Test Action
struct TestAction;

impl ShortcutAction for TestAction {
    fn start(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str) {
        log::info!(
            "Shortcut ID '{}': Started - {} (App: {})", // Changed "Pressed" to "Started" for consistency
            binding_id,
            shortcut_str,
            app.package_info().name
        );
    }

    fn stop(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str) {
        log::info!(
            "Shortcut ID '{}': Stopped - {} (App: {})", // Changed "Released" to "Stopped" for consistency
            binding_id,
            shortcut_str,
            app.package_info().name
        );
    }
}

/// Runs on the shortcut's press; the release does nothing.
struct PressAction(fn(&AppHandle));

impl ShortcutAction for PressAction {
    fn start(&self, app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        (self.0)(app);
    }

    fn stop(&self, _app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {}
}

/// A transform's own shortcut (`transform:<id>`).
struct TransformAction;

impl ShortcutAction for TransformAction {
    fn start(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        if let Some(id) = crate::transform_rules::transform_id_of(binding_id) {
            crate::transforms::run_shortcut(app, id);
        }
    }

    fn stop(&self, _app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {}
}

static TRANSFORM_ACTION: Lazy<Arc<dyn ShortcutAction>> =
    Lazy::new(|| Arc::new(TransformAction) as Arc<dyn ShortcutAction>);

/// The action for a binding, including each transform's.
pub fn action_for(binding_id: &str) -> Option<Arc<dyn ShortcutAction>> {
    if crate::transform_rules::transform_id_of(binding_id).is_some() {
        return Some(TRANSFORM_ACTION.clone());
    }
    ACTION_MAP.get(binding_id).cloned()
}

/// Adds the text selected in another app to the Dictionary.
struct AddSelectionAction;

impl ShortcutAction for AddSelectionAction {
    fn start(&self, app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        crate::selection::add_selection_to_dictionary(app);
    }

    fn stop(&self, _app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {}
}

// Static Action Map
pub static ACTION_MAP: Lazy<HashMap<String, Arc<dyn ShortcutAction>>> = Lazy::new(|| {
    let mut map = HashMap::new();
    map.insert(
        "transcribe".to_string(),
        Arc::new(TranscribeAction {
            mode: DictationMode::Plain,
        }) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "transcribe_with_post_process".to_string(),
        Arc::new(TranscribeAction {
            mode: DictationMode::Transform,
        }) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "ask".to_string(),
        Arc::new(TranscribeAction {
            mode: DictationMode::Ask,
        }) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        crate::scratchpad::DICTATE_BINDING.to_string(),
        Arc::new(TranscribeAction {
            mode: DictationMode::Scratchpad,
        }) as Arc<dyn ShortcutAction>,
    );
    for (id, run) in [
        (
            "transform_selection",
            crate::transforms::pick_for_selection as fn(&AppHandle),
        ),
        ("view_changes", crate::transforms::view_last_changes),
        ("paste_last", crate::transforms::paste_last),
        ("copy_last", crate::transforms::copy_last),
        ("toggle_notes", crate::notes::toggle),
    ] {
        map.insert(
            id.to_string(),
            Arc::new(PressAction(run)) as Arc<dyn ShortcutAction>,
        );
    }
    map.insert(
        "cancel".to_string(),
        Arc::new(CancelAction) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "add_to_dictionary".to_string(),
        Arc::new(AddSelectionAction) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "test".to_string(),
        Arc::new(TestAction) as Arc<dyn ShortcutAction>,
    );
    map
});

#[cfg(test)]
mod tests {
    use super::{complete_unless_cancelled, is_blank_transcription, should_use_streaming_overlay};
    use crate::settings::OverlayStyle;
    use std::future;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn blank_transcription_is_detected() {
        assert!(is_blank_transcription(""));
        assert!(is_blank_transcription("   "));
        assert!(is_blank_transcription("\t\n  \r\n"));
    }

    #[test]
    fn non_blank_transcription_is_kept() {
        assert!(!is_blank_transcription("hello"));
        assert!(!is_blank_transcription("  hello  "));
    }

    #[test]
    fn completed_operation_returns_its_output() {
        let result = tauri::async_runtime::block_on(complete_unless_cancelled(
            future::ready("done"),
            || false,
        ));

        assert_eq!(result, Some("done"));
    }

    #[test]
    fn pending_operation_stops_after_cancellation() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancelled_for_thread = Arc::clone(&cancelled);
        let cancel_thread = thread::spawn(move || {
            thread::sleep(Duration::from_millis(10));
            cancelled_for_thread.store(true, Ordering::Release);
        });

        let result = tauri::async_runtime::block_on(complete_unless_cancelled(
            future::pending::<()>(),
            || cancelled.load(Ordering::Acquire),
        ));

        cancel_thread.join().unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn live_overlay_uses_streaming_states_only_for_streaming_models() {
        assert!(should_use_streaming_overlay(OverlayStyle::Live, true));
        assert!(!should_use_streaming_overlay(OverlayStyle::Live, false));
        assert!(!should_use_streaming_overlay(OverlayStyle::Minimal, true));
        assert!(!should_use_streaming_overlay(OverlayStyle::None, true));
    }
}
