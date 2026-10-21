//! Notes: meetings and voice memos recorded on this PC. A memo records the
//! microphone; a meeting adds what the PC plays (one app's sound, or
//! everything). Each track is cut into spoken lines at pauses, and every
//! line is transcribed while the recording runs, so a note is ready seconds
//! after it stops. Audio is kept as one WAV next to the history recordings
//! and deleted with the note.

use crate::loopback::{self, Capture, Source};
use crate::managers::notes_store::{
    self as store, DeletedRecording, NewNote, Note, NoteKind, NoteLine, NoteMatch, NoteStatus,
    Speaker,
};
use crate::managers::transcription::TranscriptionManager;
use crate::notes_rules::{self, Segmenter, Utterance, RATE};
use crate::settings::get_settings;
use crate::transforms::BarNotice;
use anyhow::{anyhow, Result};
use handy_recorder::{AudioChunk, Recorder, RecorderConfig, Sink};
use log::{debug, error, info, warn};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

/// How often the meters and the clock update.
const TICK: Duration = Duration::from_millis(125);
/// How often a track's WAV header is rewritten, so a crash keeps the audio.
const FLUSH_EVERY: Duration = Duration::from_secs(10);
/// A line being spoken is previewed once it's this long, then again each
/// time it grows by `PREVIEW_EVERY`, while the transcriber has nothing else.
const PREVIEW_AFTER: usize = RATE * 6 / 5;
const PREVIEW_EVERY: usize = RATE * 3 / 2;
/// System audio that falls this far behind the clock (nothing playing) is
/// filled with silence, so both tracks stay in step.
const PAD_AFTER: u64 = (RATE as u64) * 3 / 10;
/// A deleted note's audio is removed after this, unless Undo brought it back.
const DELETE_AUDIO_AFTER: Duration = Duration::from_secs(20);

/// Sent when a note is added, changed or removed.
#[derive(Clone, Debug, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct NotesChanged;

/// A line transcribed during a recording.
#[derive(Clone, Debug, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct NoteLineAdded {
    pub line: NoteLine,
}

/// The words of the line being spoken, before it ends, for the live
/// transcript. Empty text clears the preview.
#[derive(Clone, Debug, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct NotePreview {
    pub note_id: i64,
    pub who: Speaker,
    pub at_ms: i64,
    pub text: String,
}

/// A microphone line dropped because it was the speakers' echo of the call.
#[derive(Clone, Debug, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct NoteLineRemoved {
    pub note_id: i64,
    pub line_id: i64,
}

/// A recording started, paused, stopped or finished.
#[derive(Clone, Debug, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct NotesSessionChanged {
    pub session: Option<SessionInfo>,
}

/// The clock and the meters, a few times a second while recording.
#[derive(Clone, Debug, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct NotesTick {
    pub elapsed_ms: i64,
    pub you: f32,
    pub them: Option<f32>,
    pub paused: bool,
    /// Lines heard and not yet transcribed.
    pub pending: u32,
}

/// Dictated words for My notes, inserted at its cursor.
#[derive(Clone, Debug, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct NoteInsert {
    pub note_id: i64,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SessionPhase {
    Recording,
    /// Stopped; the last lines are being transcribed.
    Finishing,
}

/// The recording in progress, for the Notes page and the Legato bar.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct SessionInfo {
    pub note_id: i64,
    pub kind: NoteKind,
    pub title: String,
    pub started_at: i64,
    pub phase: SessionPhase,
    pub paused: bool,
    pub elapsed_ms: i64,
    pub pending: u32,
    pub microphone: String,
    /// What the meeting records as the other side, by name.
    pub system_audio: Option<String>,
    /// One app was asked for, but this Windows records everything.
    pub system_fallback: bool,
    /// The recording stops at this length; 0 is no limit.
    pub max_ms: i64,
}

/// What Record asks for.
#[derive(Clone, Debug, Deserialize, Type)]
pub struct StartNotes {
    pub kind: NoteKind,
    pub title: Option<String>,
    /// A microphone by name; None uses the one dictation uses.
    pub microphone: Option<String>,
    /// The app whose sound a meeting records; None records everything.
    pub app_pid: Option<u32>,
}

/// The note list, the transcript and the audio files.
pub struct NotesManager {
    app: AppHandle,
    db_path: PathBuf,
    dir: PathBuf,
}

impl NotesManager {
    /// Opens the notes in the history database, which `HistoryManager` has
    /// already migrated, and finishes notes a crash or a quit left open.
    pub fn new(app: &AppHandle) -> Result<Self> {
        let data = crate::portable::app_data_dir(app)?;
        let manager = Self {
            app: app.clone(),
            db_path: data.join("history.db"),
            dir: data.join("recordings"),
        };
        std::fs::create_dir_all(&manager.dir)?;
        manager.recover();
        Ok(manager)
    }

    fn conn(&self) -> Result<Connection> {
        Ok(Connection::open(&self.db_path)?)
    }

    pub(crate) fn changed(&self) {
        if let Err(e) = NotesChanged.emit(&self.app) {
            error!("Failed to emit notes-changed: {}", e);
        }
    }

    fn recover(&self) {
        let ids = match self.conn().and_then(|c| store::recover_unfinished(&c)) {
            Ok(ids) => ids,
            Err(e) => {
                error!("Couldn't check for unfinished notes: {}", e);
                return;
            }
        };
        for id in ids {
            info!("Note {id} was left unfinished; keeping what was transcribed");
            let you = self.track_path(id, Speaker::You);
            let them = self.track_path(id, Speaker::Them);
            if !you.exists() {
                continue;
            }
            let out = self.audio_path(id);
            match mix_tracks(&you, them.exists().then_some(them.as_path()), &out) {
                Ok(samples) => {
                    let duration = samples as i64 * 1000 / RATE as i64;
                    let file = file_name(&out);
                    if let Err(e) = self
                        .conn()
                        .and_then(|c| store::finish_recording(&c, id, duration, Some(&file), 0))
                    {
                        error!("Couldn't save the recovered note's audio: {e}");
                    }
                }
                Err(e) => warn!("The unfinished note's audio couldn't be kept: {e}"),
            }
        }
    }

    fn track_path(&self, id: i64, who: Speaker) -> PathBuf {
        self.dir.join(format!("note-{id}-{}.wav", who.as_str()))
    }

    /// Where recordings are kept.
    pub(crate) fn audio_dir(&self) -> &std::path::Path {
        &self.dir
    }

    fn audio_path(&self, id: i64) -> PathBuf {
        self.dir.join(format!("note-{id}.wav"))
    }

    pub fn list(&self) -> Result<Vec<Note>> {
        store::list(&self.conn()?)
    }

    pub fn get(&self, id: i64) -> Result<Option<Note>> {
        store::get(&self.conn()?, id)
    }

    pub fn lines(&self, id: i64) -> Result<Vec<NoteLine>> {
        store::lines(&self.conn()?, id)
    }

    pub fn search(&self, query: &str) -> Result<Vec<NoteMatch>> {
        store::search(&self.conn()?, query)
    }

    pub fn tags(&self) -> Result<Vec<String>> {
        store::tags(&self.conn()?)
    }

    pub fn rename(&self, id: i64, title: &str) -> Result<()> {
        store::rename(&self.conn()?, id, title)?;
        self.changed();
        Ok(())
    }

    /// Saves My notes without telling other windows; the page that typed it
    /// already has it.
    pub fn set_my_notes(&self, id: i64, text: &str) -> Result<()> {
        store::set_my_notes(&self.conn()?, id, text)
    }

    pub fn add_tag(&self, id: i64, tag: &str) -> Result<()> {
        store::add_tag(&self.conn()?, id, tag)?;
        self.changed();
        Ok(())
    }

    pub fn remove_tag(&self, id: i64, tag: &str) -> Result<()> {
        store::remove_tag(&self.conn()?, id, tag)?;
        self.changed();
        Ok(())
    }

    pub fn set_action_done(&self, action_id: i64, done: bool) -> Result<()> {
        store::set_action_done(&self.conn()?, action_id, done)?;
        self.changed();
        Ok(())
    }

    /// The audio file to play, if it is still there.
    pub fn audio_file(&self, id: i64) -> Result<Option<PathBuf>> {
        let Some(note) = self.get(id)? else {
            return Ok(None);
        };
        Ok(note
            .audio_file
            .map(|f| self.dir.join(f))
            .filter(|p| p.exists()))
    }

    pub fn markdown(&self, id: i64) -> Result<String> {
        let note = self.get(id)?.ok_or_else(|| anyhow!("That note is gone"))?;
        let lines = self.lines(id)?;
        Ok(store::markdown(&note, &lines, "You", "Them"))
    }

    /// Removes a note; its audio goes a little later, so Undo can bring it
    /// back.
    pub fn delete(&self, id: i64) -> Result<Option<DeletedRecording>> {
        if current_note() == Some(id) {
            return Err(anyhow!("Stop the recording before deleting it"));
        }
        let deleted = store::delete(&mut self.conn()?, id)?;
        if let Some(file) = deleted.as_ref().and_then(|d| d.note.audio_file.clone()) {
            let path = self.dir.join(file);
            let db_path = self.db_path.clone();
            std::thread::spawn(move || {
                std::thread::sleep(DELETE_AUDIO_AFTER);
                let back = Connection::open(&db_path)
                    .ok()
                    .and_then(|c| store::get(&c, id).ok().flatten())
                    .is_some();
                if !back {
                    if let Err(e) = std::fs::remove_file(&path) {
                        debug!("Couldn't remove a deleted note's audio: {e}");
                    }
                }
            });
        }
        self.changed();
        Ok(deleted)
    }

    pub fn restore(&self, deleted: &DeletedRecording) -> Result<()> {
        store::restore(&mut self.conn()?, deleted)?;
        self.changed();
        Ok(())
    }

    /// Saves a line, unless it's the microphone hearing the call through the
    /// speakers; a call line drops the microphone's echo of it.
    fn save_line(
        &self,
        note_id: i64,
        meeting: bool,
        who: Speaker,
        utterance: &Utterance,
        text: &str,
    ) -> Result<()> {
        let conn = self.conn()?;
        let (at, end) = (utterance.start_ms(), utterance.end_ms());
        if meeting {
            let other = match who {
                Speaker::You => Speaker::Them,
                Speaker::Them => Speaker::You,
            };
            for line in store::lines_near(&conn, note_id, other, at - 2_000, end + 2_000)? {
                let theirs = (line.text.as_str(), line.at_ms, line.end_ms);
                match who {
                    Speaker::You if notes_rules::is_echo((text, at, end), theirs) => {
                        debug!("Dropped a microphone line that echoed the call");
                        return Ok(());
                    }
                    Speaker::Them if notes_rules::is_echo(theirs, (text, at, end)) => {
                        store::remove_line(&conn, line.id)?;
                        let _ = NoteLineRemoved {
                            note_id,
                            line_id: line.id,
                        }
                        .emit(&self.app);
                    }
                    _ => {}
                }
            }
        }
        let line = store::add_line(&conn, note_id, at, end, who, text)?;
        let _ = NoteLineAdded { line }.emit(&self.app);
        Ok(())
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub fn manager(app: &AppHandle) -> Arc<NotesManager> {
    app.state::<Arc<NotesManager>>().inner().clone()
}

/* ------------------------------------------------------------ recording */

/// Recorded time, without pauses.
struct Clock {
    started: Instant,
    paused_for: Duration,
    paused_at: Option<Instant>,
}

impl Clock {
    fn elapsed(&self) -> Duration {
        let now = self.paused_at.unwrap_or_else(Instant::now);
        now.saturating_duration_since(self.started)
            .saturating_sub(self.paused_for)
    }
}

/// A level that the tracks raise and the ticker lets fall.
#[derive(Default)]
struct Level(AtomicU32);

impl Level {
    fn raise(&self, value: f32) {
        // Levels are never negative, and non-negative floats order like
        // their bits.
        self.0
            .fetch_max(value.max(0.0).to_bits(), Ordering::Relaxed);
    }

    fn take(&self) -> f32 {
        f32::from_bits(self.0.swap(0f32.to_bits(), Ordering::Relaxed))
    }
}

/// What a recording shares between its threads.
struct Shared {
    clock: Mutex<Clock>,
    paused: AtomicBool,
    pending: AtomicU32,
    /// A preview is queued or being transcribed.
    previewing: AtomicBool,
    you: Level,
    them: Level,
    stop_ticker: AtomicBool,
}

impl Shared {
    fn elapsed(&self) -> Duration {
        self.clock
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .elapsed()
    }
}

struct MicSink {
    tx: Sender<Vec<f32>>,
    shared: Arc<Shared>,
}

impl Sink for MicSink {
    fn process_chunk(&mut self, chunk: AudioChunk<'_>) {
        if self.shared.paused.load(Ordering::Relaxed) {
            return;
        }
        let real = chunk.valid_frames * chunk.channels as usize;
        let _ = self.tx.send(chunk.samples[..real].to_vec());
    }
}

struct Job {
    who: Speaker,
    utterance: Utterance,
    /// The line so far, for the live transcript: shown, not saved.
    preview: bool,
}

struct Session {
    info: SessionInfo,
    shared: Arc<Shared>,
    recorder: Recorder<MicSink>,
    capture: Option<Capture>,
    tracks: Vec<JoinHandle<Result<u64>>>,
    transcriber: JoinHandle<(u32, u32)>,
}

/// After Stop, while the last lines are transcribed.
struct Finishing {
    info: SessionInfo,
    shared: Arc<Shared>,
}

static SESSION: Mutex<Option<Session>> = Mutex::new(None);
static FINISHING: Mutex<Option<Finishing>> = Mutex::new(None);

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Recording now.
pub fn is_recording() -> bool {
    lock(&SESSION).is_some()
}

/// Recording, or transcribing the end of a recording.
pub fn is_busy() -> bool {
    is_recording() || lock(&FINISHING).is_some()
}

fn current_note() -> Option<i64> {
    lock(&SESSION)
        .as_ref()
        .map(|s| s.info.note_id)
        .or_else(|| lock(&FINISHING).as_ref().map(|f| f.info.note_id))
}

/// The recording in progress, with its clock up to date.
pub fn session() -> Option<SessionInfo> {
    let fresh = |info: &SessionInfo, shared: &Shared| SessionInfo {
        elapsed_ms: shared.elapsed().as_millis() as i64,
        paused: shared.paused.load(Ordering::Relaxed),
        pending: shared.pending.load(Ordering::Relaxed),
        ..info.clone()
    };
    if let Some(s) = lock(&SESSION).as_ref() {
        return Some(fresh(&s.info, &s.shared));
    }
    lock(&FINISHING).as_ref().map(|f| fresh(&f.info, &f.shared))
}

/// The bar's Notes pill shows while a recording runs and its side panel
/// is closed.
/// The bar's Notes pill shows while notes record with the panel closed,
/// unless the bubble is on: then the bubble shows them instead.
pub fn refresh_pill(app: &AppHandle) {
    let recording = session().is_some_and(|s| s.phase == SessionPhase::Recording);
    crate::overlay::set_notes_pill(
        app,
        recording && !crate::notes_panel::is_open() && !crate::bubble::takes_dictation(app),
    );
}

fn announce(app: &AppHandle) {
    let session = session();
    refresh_pill(app);
    // The panel goes with the recording.
    if session.is_none() && crate::notes_panel::is_open() {
        crate::notes_panel::hide(app);
    }
    let _ = NotesSessionChanged { session }.emit(app);
}

fn default_title(kind: NoteKind) -> String {
    let when = chrono::Local::now().format("%b %-d, %-I:%M %p");
    match kind {
        NoteKind::Meeting => format!("Meeting · {when}"),
        NoteKind::Memo => format!("Voice memo · {when}"),
    }
}

/// The microphone's device id from its name, or the system default.
fn microphone_device(name: Option<&str>) -> Option<String> {
    let name = name?;
    handy_recorder::list_input_devices()
        .ok()?
        .into_iter()
        .find(|d| d.name == name)
        .map(|d| d.id)
}

/// Starts a recording.
pub fn start(app: &AppHandle, options: StartNotes) -> Result<Note> {
    let mut slot = lock(&SESSION);
    if slot.is_some() {
        return Err(anyhow!("A recording is already running"));
    }
    if lock(&FINISHING).is_some() {
        return Err(anyhow!(
            "The last recording is still being transcribed; try again in a moment"
        ));
    }
    let settings = get_settings(app);
    if settings.selected_model.is_empty() {
        return Err(anyhow!("Download a speech model in Models first"));
    }
    // Settings › Context & calls › Stop recording after; 0 is no limit.
    let max = (settings.calls.max_minutes > 0)
        .then(|| Duration::from_secs(settings.calls.max_minutes as u64 * 60));
    let m = manager(app);
    let shared = Arc::new(Shared {
        clock: Mutex::new(Clock {
            started: Instant::now(),
            paused_for: Duration::ZERO,
            paused_at: None,
        }),
        paused: AtomicBool::new(false),
        pending: AtomicU32::new(0),
        previewing: AtomicBool::new(false),
        you: Level::default(),
        them: Level::default(),
        stop_ticker: AtomicBool::new(false),
    });

    let microphone = options
        .microphone
        .clone()
        .or_else(|| settings.selected_microphone.clone());
    let recorder: Recorder<MicSink> = Recorder::open(RecorderConfig {
        device: microphone_device(microphone.as_deref()),
        ..RecorderConfig::speech()
    })
    .map_err(|e| anyhow!("The microphone didn't open: {e}"))?;
    let microphone = recorder.info().device.name.clone();

    let (them_tx, them_rx) = channel::<Vec<f32>>();
    let capture = match options.kind {
        NoteKind::Meeting => {
            let source = options.app_pid.map_or(Source::Everything, Source::App);
            let shared = Arc::clone(&shared);
            let capture = loopback::start(source, move |samples| {
                if !shared.paused.load(Ordering::Relaxed) {
                    let _ = them_tx.send(samples.to_vec());
                }
            })
            .map_err(|e| anyhow!("System audio didn't start: {e}"))?;
            Some(capture)
        }
        NoteKind::Memo => {
            drop(them_tx);
            None
        }
    };
    let app_info = options
        .app_pid
        .filter(|_| capture.as_ref().is_some_and(|c| !c.fallback))
        .and_then(crate::context::app_for_process);

    let title = options
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        // A meeting on the calendar names the note.
        .or_else(|| {
            (options.kind == NoteKind::Meeting)
                .then(|| crate::calendar::meeting_now(app))
                .flatten()
                .map(|m| m.title)
        })
        .unwrap_or_else(|| default_title(options.kind));
    let started_at = chrono::Utc::now().timestamp();
    let note = store::create(
        &m.conn()?,
        &NewNote {
            title: &title,
            kind: options.kind,
            app: app_info.as_ref().map(|a| a.name.as_str()),
            app_path: app_info.as_ref().map(|a| a.exe_path.as_str()),
            microphone: Some(&microphone),
            system_audio: capture.as_ref().map(|c| c.name.as_str()),
            started_at,
        },
    )?;
    let meeting = options.kind == NoteKind::Meeting;

    let (jobs_tx, jobs_rx) = channel::<Job>();
    let transcriber = {
        let app = app.clone();
        let shared = Arc::clone(&shared);
        std::thread::Builder::new()
            .name("notes-transcribe".into())
            .spawn(move || transcribe_lines(&app, note.id, meeting, jobs_rx, &shared))?
    };
    let (mic_tx, mic_rx) = channel::<Vec<f32>>();
    let mut tracks = vec![spawn_track(
        app,
        Speaker::You,
        m.track_path(note.id, Speaker::You),
        mic_rx,
        false,
        &shared,
        jobs_tx.clone(),
    )?];
    if capture.is_some() {
        tracks.push(spawn_track(
            app,
            Speaker::Them,
            m.track_path(note.id, Speaker::Them),
            them_rx,
            true,
            &shared,
            jobs_tx.clone(),
        )?);
    }
    drop(jobs_tx);
    if let Err(e) = recorder.start(MicSink {
        tx: mic_tx,
        shared: Arc::clone(&shared),
    }) {
        let _ = store::delete(&mut m.conn()?, note.id);
        return Err(anyhow!("The microphone didn't start: {}", e.error));
    }

    let info = SessionInfo {
        note_id: note.id,
        kind: options.kind,
        title,
        started_at,
        phase: SessionPhase::Recording,
        paused: false,
        elapsed_ms: 0,
        pending: 0,
        microphone,
        system_audio: capture.as_ref().map(|c| c.name.clone()),
        system_fallback: capture.as_ref().is_some_and(|c| c.fallback),
        max_ms: max.map_or(0, |m| m.as_millis() as i64),
    };
    info!(
        "Notes started: {:?}{}",
        options.kind,
        info.system_audio
            .as_deref()
            .map(|s| format!(" with {s}"))
            .unwrap_or_default()
    );
    spawn_ticker(app, Arc::clone(&shared), meeting, max);
    *slot = Some(Session {
        info,
        shared,
        recorder,
        capture,
        tracks,
        transcriber,
    });
    drop(slot);
    // The model loads now, so the first line doesn't wait for it.
    if let Some(tm) = app.try_state::<Arc<TranscriptionManager>>() {
        tm.initiate_model_load();
    }
    m.changed();
    announce(app);
    Ok(note)
}

/// Pauses or resumes the recording; paused time isn't recorded.
pub fn set_paused(app: &AppHandle, paused: bool) -> Result<()> {
    {
        let slot = lock(&SESSION);
        let s = slot
            .as_ref()
            .ok_or_else(|| anyhow!("Nothing is recording"))?;
        let mut clock = lock(&s.shared.clock);
        match (paused, clock.paused_at) {
            (true, None) => clock.paused_at = Some(Instant::now()),
            (false, Some(at)) => {
                clock.paused_for += at.elapsed();
                clock.paused_at = None;
            }
            _ => {}
        }
        s.shared.paused.store(paused, Ordering::Relaxed);
    }
    announce(app);
    Ok(())
}

/// Stops the recording; the note is ready once its last lines are
/// transcribed.
pub fn stop(app: &AppHandle) -> Result<()> {
    let session = lock(&SESSION)
        .take()
        .ok_or_else(|| anyhow!("Nothing is recording"))?;
    session.shared.stop_ticker.store(true, Ordering::Relaxed);
    crate::calls::notes_stopped(app);
    *lock(&FINISHING) = Some(Finishing {
        info: SessionInfo {
            phase: SessionPhase::Finishing,
            ..session.info.clone()
        },
        shared: Arc::clone(&session.shared),
    });
    announce(app);
    let app = app.clone();
    std::thread::spawn(move || {
        finish(&app, session);
        *lock(&FINISHING) = None;
        announce(&app);
    });
    Ok(())
}

/// Starts a meeting with the defaults, or stops the recording: the Start or
/// stop notes shortcut.
pub fn toggle(app: &AppHandle) {
    let result = if is_recording() {
        stop(app)
    } else {
        let app_pid = loopback::audio_apps()
            .into_iter()
            .find(|a| a.call)
            .map(|a| a.pid);
        start(
            app,
            StartNotes {
                kind: NoteKind::Meeting,
                title: None,
                microphone: None,
                app_pid,
            },
        )
        .map(|_| ())
    };
    if let Err(e) = result {
        warn!("Start or stop notes: {e}");
        crate::overlay::show_overlay_notice(
            app,
            &BarNotice::NotesFailed {
                reason: e.to_string(),
            },
        );
    }
}

fn spawn_ticker(app: &AppHandle, shared: Arc<Shared>, meeting: bool, max: Option<Duration>) {
    let app = app.clone();
    std::thread::spawn(move || {
        while !shared.stop_ticker.load(Ordering::Relaxed) {
            std::thread::sleep(TICK);
            let elapsed = shared.elapsed();
            let _ = NotesTick {
                elapsed_ms: elapsed.as_millis() as i64,
                you: shared.you.take(),
                them: meeting.then(|| shared.them.take()),
                paused: shared.paused.load(Ordering::Relaxed),
                pending: shared.pending.load(Ordering::Relaxed),
            }
            .emit(&app);
            if max.is_some_and(|max| elapsed >= max) {
                info!("Notes reached the recording limit; stopping");
                let _ = stop(&app);
                break;
            }
        }
    });
}

/// One track: writes it to a WAV and cuts it into lines for the
/// transcriber.
struct Track {
    who: Speaker,
    writer: hound::WavWriter<std::io::BufWriter<std::fs::File>>,
    vad: Option<Box<dyn crate::audio_toolkit::SpeechScorer>>,
    frame: usize,
    buffer: Vec<f32>,
    segmenter: Segmenter,
    /// How much of the open line the last preview covered.
    previewed: usize,
    written: u64,
    shared: Arc<Shared>,
    jobs: Sender<Job>,
}

impl Track {
    fn feed(&mut self, samples: &[f32]) -> Result<()> {
        for &s in samples {
            self.writer
                .write_sample((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)?;
        }
        self.written += samples.len() as u64;
        self.buffer.extend_from_slice(samples);
        while self.buffer.len() >= self.frame {
            let frame: Vec<f32> = self.buffer.drain(..self.frame).collect();
            let level = notes_rules::level(&frame);
            match self.who {
                Speaker::You => self.shared.you.raise(level),
                Speaker::Them => self.shared.them.raise(level),
            }
            let speech = match self.vad.as_mut() {
                Some(vad) => vad.is_voice(&frame).unwrap_or(false),
                // Without a VAD, loud enough counts as speech.
                None => level > 0.35,
            };
            if let Some(utterance) = self.segmenter.push(&frame, speech) {
                self.previewed = 0;
                self.send(utterance);
            }
        }
        self.preview();
        Ok(())
    }

    fn send(&self, utterance: Utterance) {
        self.shared.pending.fetch_add(1, Ordering::Relaxed);
        if self
            .jobs
            .send(Job {
                who: self.who,
                utterance,
                preview: false,
            })
            .is_err()
        {
            self.shared.pending.fetch_sub(1, Ordering::Relaxed);
        }
    }

    /// Hands the line being spoken to the transcriber for a preview, when
    /// it has grown enough and nothing waits to be transcribed. Lines only
    /// end at a pause, so without this a long turn shows nothing until
    /// its speaker stops.
    fn preview(&mut self) {
        let Some((start, samples)) = self.segmenter.open_line() else {
            return;
        };
        let due = if self.previewed == 0 {
            PREVIEW_AFTER
        } else {
            self.previewed + PREVIEW_EVERY
        };
        if samples.len() < due || self.shared.pending.load(Ordering::Relaxed) > 0 {
            return;
        }
        if self.shared.previewing.swap(true, Ordering::Relaxed) {
            return;
        }
        self.previewed = samples.len();
        let job = Job {
            who: self.who,
            utterance: Utterance {
                start,
                samples: samples.to_vec(),
            },
            preview: true,
        };
        if self.jobs.send(job).is_err() {
            self.shared.previewing.store(false, Ordering::Relaxed);
        }
    }
}

fn spawn_track(
    app: &AppHandle,
    who: Speaker,
    path: PathBuf,
    rx: Receiver<Vec<f32>>,
    pad: bool,
    shared: &Arc<Shared>,
    jobs: Sender<Job>,
) -> Result<JoinHandle<Result<u64>>> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: RATE as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let writer = hound::WavWriter::create(&path, spec)?;
    let vad = match crate::managers::audio::create_detector(get_settings(app).vad_backend, app) {
        Ok(vad) => Some(vad),
        Err(e) => {
            warn!("Notes has no voice detection ({e}); cutting lines by loudness");
            None
        }
    };
    let frame = vad.as_ref().map_or(512, |v| v.frame_samples());
    let mut track = Track {
        who,
        writer,
        vad,
        frame,
        buffer: Vec::new(),
        segmenter: Segmenter::new(),
        previewed: 0,
        written: 0,
        shared: Arc::clone(shared),
        jobs,
    };
    Ok(std::thread::Builder::new()
        .name(format!("notes-{}", who.as_str()))
        .spawn(move || {
            let mut flushed = Instant::now();
            loop {
                match rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(samples) => track.feed(&samples)?,
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }
                if pad && !track.shared.paused.load(Ordering::Relaxed) {
                    let expected = track.shared.elapsed().as_millis() as u64 * RATE as u64 / 1000;
                    if expected > track.written + PAD_AFTER {
                        let silence = vec![0.0; (expected - track.written) as usize];
                        track.feed(&silence)?;
                    }
                }
                if flushed.elapsed() >= FLUSH_EVERY {
                    track.writer.flush()?;
                    flushed = Instant::now();
                }
            }
            if let Some(utterance) = track.segmenter.finish() {
                track.send(utterance);
            }
            let written = track.written;
            track.writer.finalize()?;
            Ok(written)
        })?)
}

/// Waits for the speech model, loading it if needed.
pub(crate) fn ensure_model(tm: &TranscriptionManager) -> Result<()> {
    if tm.is_model_loaded() {
        return Ok(());
    }
    tm.initiate_model_load();
    let until = Instant::now() + Duration::from_secs(120);
    while Instant::now() < until {
        std::thread::sleep(Duration::from_millis(200));
        if tm.is_model_loaded() {
            return Ok(());
        }
    }
    Err(anyhow!("The speech model didn't load"))
}

/// Transcribes lines as the tracks hand them over, until both tracks end.
/// Returns how many lines were saved and how many failed.
fn transcribe_lines(
    app: &AppHandle,
    note_id: i64,
    meeting: bool,
    jobs: Receiver<Job>,
    shared: &Shared,
) -> (u32, u32) {
    let m = manager(app);
    let Some(tm) = app.try_state::<Arc<TranscriptionManager>>() else {
        return (0, 0);
    };
    let (mut saved, mut failed) = (0, 0);
    // A failed line keeps its place, so it can be tried again from the audio.
    let keep_failed = |job: &Job| {
        let (at, end) = (job.utterance.start_ms(), job.utterance.end_ms());
        if let Err(e) = m
            .conn()
            .and_then(|c| store::add_failed_line(&c, note_id, at, end, job.who))
        {
            error!("Couldn't keep a failed Notes line: {e}");
        }
    };
    let clear = |who: Speaker| {
        let _ = NotePreview {
            note_id,
            who,
            at_ms: 0,
            text: String::new(),
        }
        .emit(app);
    };
    for job in jobs {
        if job.preview {
            // Only with the model already loaded and the live transcript
            // shown; a preview is never worth a wait.
            if tm.is_model_loaded() && get_settings(app).notes_live_transcript {
                if let Ok(f) = tm.transcribe(job.utterance.samples.clone()) {
                    let _ = NotePreview {
                        note_id,
                        who: job.who,
                        at_ms: job.utterance.start_ms(),
                        text: f.text.trim().to_string(),
                    }
                    .emit(app);
                }
            }
            shared.previewing.store(false, Ordering::Relaxed);
            continue;
        }
        // A cancelled dictation can stop a run in flight; one retry covers it.
        let text = ensure_model(&tm)
            .and_then(|_| tm.transcribe(job.utterance.samples.clone()))
            .or_else(|e| {
                debug!("A Notes line failed once ({e}); retrying");
                ensure_model(&tm).and_then(|_| tm.transcribe(job.utterance.samples.clone()))
            })
            .map(|f| f.text);
        shared.pending.fetch_sub(1, Ordering::Relaxed);
        match text {
            Ok(text) if text.trim().is_empty() => {}
            Ok(text) => match m.save_line(note_id, meeting, job.who, &job.utterance, text.trim()) {
                Ok(()) => saved += 1,
                Err(e) => {
                    error!("Couldn't save a Notes line: {e}");
                    keep_failed(&job);
                    failed += 1;
                }
            },
            Err(e) => {
                warn!("A Notes line couldn't be transcribed: {e}");
                keep_failed(&job);
                failed += 1;
            }
        }
        // The line is in (or dropped as an echo): its preview goes.
        clear(job.who);
    }
    (saved, failed)
}

/// What trying a note's failed lines again came to.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Type)]
pub struct RetryResult {
    pub transcribed: u32,
    pub failed: u32,
}

/// Tries the lines that failed again, from the note's audio. In a meeting
/// the audio is the mix of both tracks, so a line can pick up the other
/// side; the echo check drops what repeats a line already there.
pub fn retry_failed_lines(app: &AppHandle, note_id: i64) -> Result<RetryResult> {
    let m = manager(app);
    let note = m
        .get(note_id)?
        .ok_or_else(|| anyhow!("That note is gone"))?;
    if note.status != NoteStatus::Ready {
        return Err(anyhow!("The note is still being worked on"));
    }
    let path = m
        .audio_file(note_id)?
        .ok_or_else(|| anyhow!("The note's audio isn't kept"))?;
    let tm = app
        .try_state::<Arc<TranscriptionManager>>()
        .ok_or_else(|| anyhow!("The speech model isn't ready"))?;
    let meeting = note.kind == NoteKind::Meeting;
    let mut reader = hound::WavReader::open(&path)?;
    let total = reader.duration();
    let mut result = RetryResult {
        transcribed: 0,
        failed: 0,
    };
    for line in store::failed_lines(&m.conn()?, note_id)? {
        let at = |ms: i64| (ms.max(0) as u64 * RATE as u64 / 1000).min(total as u64) as u32;
        let (start, end) = (at(line.at_ms), at(line.end_ms));
        reader.seek(start)?;
        let samples: Vec<f32> = reader
            .samples::<i16>()
            .take(end.saturating_sub(start) as usize)
            .map_while(Result::ok)
            .map(|s| s as f32 / i16::MAX as f32)
            .collect();
        let text = if samples.is_empty() {
            Ok(String::new())
        } else {
            ensure_model(&tm)
                .and_then(|_| tm.transcribe(samples.clone()))
                .map(|f| f.text)
        };
        match text {
            Ok(text) => {
                let utterance = Utterance {
                    start: start as usize,
                    samples,
                };
                if !text.trim().is_empty() {
                    m.save_line(note_id, meeting, line.who, &utterance, text.trim())?;
                }
                store::resolve_failed_line(&m.conn()?, &line)?;
                result.transcribed += 1;
            }
            Err(e) => {
                warn!("A Notes line failed again: {e}");
                result.failed += 1;
            }
        }
    }
    m.changed();
    Ok(result)
}

/// Mixes the tracks into the note's audio file and removes them. Returns
/// the samples written.
fn mix_tracks(you: &Path, them: Option<&Path>, out: &Path) -> Result<u64> {
    let Some(them) = them else {
        let samples = hound::WavReader::open(you)?.len() as u64;
        std::fs::rename(you, out)?;
        return Ok(samples);
    };
    let mut a = hound::WavReader::open(you)?;
    let mut b = hound::WavReader::open(them)?;
    let mut writer = hound::WavWriter::create(out, a.spec())?;
    let mut a = a.samples::<i16>().map_while(Result::ok);
    let mut b = b.samples::<i16>().map_while(Result::ok);
    let mut written = 0u64;
    loop {
        let x: Vec<i16> = a.by_ref().take(RATE).collect();
        let y: Vec<i16> = b.by_ref().take(RATE).collect();
        if x.is_empty() && y.is_empty() {
            break;
        }
        for s in notes_rules::mix(&x, &y) {
            writer.write_sample(s)?;
            written += 1;
        }
    }
    writer.finalize()?;
    let _ = std::fs::remove_file(you);
    let _ = std::fs::remove_file(them);
    Ok(written)
}

fn finish(app: &AppHandle, session: Session) {
    let m = manager(app);
    let note_id = session.info.note_id;
    let meeting = session.info.kind == NoteKind::Meeting;
    if let Err(e) = m
        .conn()
        .and_then(|c| store::set_status(&c, note_id, NoteStatus::Transcribing))
    {
        error!("Couldn't mark the note as transcribing: {e}");
    }
    m.changed();
    if let Err(e) = session.recorder.stop() {
        warn!("The Notes microphone didn't stop cleanly: {e}");
    }
    drop(session.recorder);
    drop(session.capture);
    for track in session.tracks {
        match track.join() {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => error!("A Notes track failed: {e}"),
            Err(_) => error!("A Notes track panicked"),
        }
    }
    let (saved, failed) = session.transcriber.join().unwrap_or((0, 0));

    let you = m.track_path(note_id, Speaker::You);
    let them = m.track_path(note_id, Speaker::Them);
    let out = m.audio_path(note_id);
    let audio = mix_tracks(&you, meeting.then_some(them.as_path()), &out);
    let duration_ms = match &audio {
        Ok(samples) => *samples as i64 * 1000 / RATE as i64,
        Err(e) => {
            error!("Couldn't keep the Notes audio: {e}");
            session.shared.elapsed().as_millis() as i64
        }
    };

    if saved == 0 && failed == 0 {
        info!("Notes heard nothing; the note isn't kept");
        let _ = m.conn().and_then(|mut c| store::delete(&mut c, note_id));
        let _ = std::fs::remove_file(&out);
        m.changed();
        crate::overlay::show_overlay_notice(app, &BarNotice::NoteEmpty { duration_ms });
        return;
    }
    let file = audio.is_ok().then(|| file_name(&out));
    let saved_note = m.conn().and_then(|c| {
        store::finish_recording(&c, note_id, duration_ms, file.as_deref(), failed as i64)?;
        store::set_status(&c, note_id, NoteStatus::Ready)
    });
    if let Err(e) = saved_note {
        error!("Couldn't finish the note: {e}");
    }
    info!("Notes ready: {saved} lines, {failed} failed, {duration_ms} ms");
    m.changed();
    tauri::async_runtime::block_on(summarise(app, note_id));
    crate::overlay::show_overlay_notice_for(
        app,
        &BarNotice::NoteReady {
            id: note_id,
            duration_ms,
        },
        5000,
    );
    crate::calls::note_ready(app, note_id);
}

/// Why a note has no summary, as Notes words it: "short", or how the
/// provider answered.
fn problem_code(kind: crate::ai::AiStatusKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| "failed".to_string())
}

/// The transcript as the prompts see it.
fn prompt_lines(lines: &[NoteLine]) -> Vec<notes_rules::PromptLine<'_>> {
    lines
        .iter()
        .map(|l| notes_rules::PromptLine {
            at_ms: l.at_ms,
            you: l.who == Speaker::You,
            text: &l.text,
        })
        .collect()
}

/// The AI to use for Notes, or why there is none: "off" when summaries
/// aren't allowed, "none" when no model is set up.
fn notes_ai(app: &AppHandle) -> Result<crate::ai::AiConfig, String> {
    let settings = get_settings(app);
    if !settings.ai_notes_enabled {
        return Err("off".to_string());
    }
    crate::ai::AiConfig::from_settings(&settings).ok_or_else(|| "none".to_string())
}

/// Writes the note's summary with your AI, when Notes may use it. Without
/// an AI the note simply has none; a failure is saved, so the page can say
/// why.
pub async fn summarise(app: &AppHandle, note_id: i64) {
    let Ok(config) = notes_ai(app) else {
        return;
    };
    let m = manager(app);
    let lines = match m.lines(note_id) {
        Ok(lines) => lines,
        Err(e) => {
            error!("Couldn't read the note to summarise it: {e}");
            return;
        }
    };
    let prompt = prompt_lines(&lines);
    let save = |result: Result<()>| {
        if let Err(e) = result {
            error!("Couldn't save the note's summary: {e}");
        }
    };
    if notes_rules::transcript_words(&prompt) < notes_rules::MIN_SUMMARY_WORDS {
        save(
            m.conn()
                .and_then(|c| store::set_summary_error(&c, note_id, Some("short"))),
        );
        m.changed();
        return;
    }
    save(
        m.conn()
            .and_then(|c| store::set_status(&c, note_id, NoteStatus::Summarising)),
    );
    m.changed();
    let started = Instant::now();
    match crate::ai::summarise_note(app, &config, &prompt).await {
        Ok(summary) => {
            let local = if crate::ai::is_local(&config.provider) {
                "local"
            } else {
                "cloud"
            };
            let model = format!("{} · {} · {local}", config.provider.label, config.model);
            let at = chrono::Utc::now().timestamp();
            save(
                m.conn()
                    .and_then(|mut c| store::set_summary(&mut c, note_id, &summary, &model, at)),
            );
            info!("Note summarised in {:?}", started.elapsed());
        }
        Err(problem) => {
            warn!("No summary for the note: {:?}", problem.kind);
            let code = problem_code(problem.kind);
            save(
                m.conn()
                    .and_then(|c| store::set_summary_error(&c, note_id, Some(&code))),
            );
        }
    }
    save(
        m.conn()
            .and_then(|c| store::set_status(&c, note_id, NoteStatus::Ready)),
    );
    m.changed();
}

/// Writes the summary again, with the AI as it is set up now.
pub async fn resummarise(app: &AppHandle, note_id: i64) -> Result<(), String> {
    notes_ai(app)?;
    if current_note() == Some(note_id) {
        return Err("recording".to_string());
    }
    summarise(app, note_id).await;
    let note = manager(app)
        .get(note_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "gone".to_string())?;
    match note.summary_error {
        Some(code) => Err(code),
        None => Ok(()),
    }
}

/// Answers a question from the note's transcript. Errors are codes the page
/// words: "off", "none", "empty", or how the provider answered.
pub async fn ask(app: &AppHandle, note_id: i64, question: &str) -> Result<String, String> {
    let config = notes_ai(app)?;
    let lines = manager(app).lines(note_id).map_err(|e| e.to_string())?;
    if lines.is_empty() {
        return Err("empty".to_string());
    }
    crate::ai::ask_note(app, &config, &prompt_lines(&lines), question)
        .await
        .map_err(|p| problem_code(p.kind))
}

/// Dictated words for My notes: into the page when it's open, else saved at
/// the end of the note's My notes.
pub fn receive_dictation(app: &AppHandle, note_id: i64, text: String) {
    if text.trim().is_empty() {
        return;
    }
    // The side panel first: it's the one in front while you take notes.
    let open = |label: &str| {
        app.get_webview_window(label)
            .and_then(|w| w.is_visible().ok())
            .unwrap_or(false)
    };
    for label in [crate::notes_panel::WINDOW, "main"] {
        if open(label) {
            let _ = NoteInsert { note_id, text }.emit_to(app, label);
            return;
        }
    }
    let m = manager(app);
    let result = m.get(note_id).and_then(|note| {
        let note = note.ok_or_else(|| anyhow!("That note is gone"))?;
        let joined = if note.my_notes.trim().is_empty() {
            text.trim().to_string()
        } else {
            format!("{}\n{}", note.my_notes.trim_end(), text.trim())
        };
        m.set_my_notes(note_id, &joined)
    });
    match result {
        Ok(()) => m.changed(),
        Err(e) => error!("Couldn't save the dictation in My notes: {e}"),
    }
}
