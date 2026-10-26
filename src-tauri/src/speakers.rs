//! Telling voices apart in Notes: the speaker model, downloaded on request
//! from Settings › Context & calls, and who spoke when in a recording. The
//! model runs in a transcription worker of its own (`engine_supervisor`).
//! It holds four voices; after a meeting that fills all four, the recording
//! is heard again in one-minute windows whose voices are joined into the
//! meeting's, so a big call keeps everyone apart.
//!
//! Naming a speaker saves a short clip of their voice (on this PC only,
//! never synced). A later meeting plays each unnamed voice next to every
//! saved clip through the same model; when it hears one voice across both,
//! the speaker gets that name, marked as recognised.

use crate::managers::notes_store::{self as store, NoteLine, Voice};
use crate::managers::transcription::TranscriptionManager;
use crate::notes_rules::{self, Turn, RATE};
use crate::settings::get_settings;
use anyhow::{anyhow, Result};
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

/// NVIDIA's Streaming Sortformer (up to four voices), converted to GGUF for
/// transcribe.cpp; NVIDIA Open Model License. Pinned to one revision and
/// checked against its hash.
const REPO: &str = "handy-computer/diar_streaming_sortformer_4spk-v2.1-gguf";
const REVISION: &str = "60ebab3ef7cae9869be0abcde13360347e96ba03";
pub const FILE: &str = "diar_streaming_sortformer_4spk-v2.1-Q8_0.gguf";
const SIZE: u64 = 139_310_336;
const SHA256: &str = "a5dacdc650790266c7a362e54e6bf51952015487edaa606c4e11632bc32442a9";

/// The speaker model on this PC.
#[derive(Clone, Debug, Default, Serialize, Deserialize, Type)]
pub struct SpeakerModel {
    pub downloaded: bool,
    pub downloading: bool,
    /// Bytes downloaded so far.
    pub done: u64,
    pub size: u64,
    /// Why the last download failed.
    pub error: Option<String>,
}

/// The speaker model was downloaded, removed, or a download moved on.
#[derive(Clone, Debug, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct SpeakerModelChanged {
    pub model: SpeakerModel,
}

#[derive(Default)]
struct Download {
    running: bool,
    done: u64,
    error: Option<String>,
}

static DOWNLOAD: Mutex<Download> = Mutex::new(Download {
    running: false,
    done: 0,
    error: None,
});
static CANCEL: AtomicBool = AtomicBool::new(false);

fn lock() -> std::sync::MutexGuard<'static, Download> {
    DOWNLOAD.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn model_path(app: &AppHandle) -> Result<PathBuf> {
    Ok(crate::portable::app_data_dir(app)?
        .join("models")
        .join(FILE))
}

pub fn is_downloaded(app: &AppHandle) -> bool {
    model_path(app).is_ok_and(|p| p.exists())
}

pub fn state(app: &AppHandle) -> SpeakerModel {
    let download = lock();
    SpeakerModel {
        downloaded: is_downloaded(app),
        downloading: download.running,
        done: download.done,
        size: SIZE,
        error: download.error.clone(),
    }
}

fn announce(app: &AppHandle) {
    let _ = SpeakerModelChanged { model: state(app) }.emit(app);
}

/// Downloads the speaker model, reporting progress through
/// `SpeakerModelChanged`. Returns once it's in place, cancelled or failed.
pub async fn download(app: &AppHandle) -> Result<(), String> {
    {
        let mut download = lock();
        if download.running || is_downloaded(app) {
            return Ok(());
        }
        *download = Download {
            running: true,
            ..Download::default()
        };
    }
    CANCEL.store(false, Ordering::Relaxed);
    announce(app);
    let result = fetch(app).await;
    {
        let mut download = lock();
        download.running = false;
        download.error = match &result {
            Err(e) if !CANCEL.load(Ordering::Relaxed) => Some(e.to_string()),
            _ => None,
        };
    }
    announce(app);
    match result {
        Ok(()) => {
            info!("Downloaded the speaker model");
            Ok(())
        }
        Err(_) if CANCEL.load(Ordering::Relaxed) => Ok(()),
        Err(e) => {
            error!("The speaker model didn't download: {e}");
            Err(e.to_string())
        }
    }
}

/// Stops the download in flight, keeping what arrived for next time.
static CANCEL_TOKEN: Mutex<Option<hf_hub::api::tokio::CancellationToken>> = Mutex::new(None);

/// Fetches the model, resuming what an earlier try left (a dropped
/// connection used to start all 139 MB again), checked by size and hash.
async fn fetch(app: &AppHandle) -> Result<()> {
    let path = model_path(app)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let partial = path.with_extension("gguf.partial");
    let url = format!("https://huggingface.co/{REPO}/resolve/{REVISION}/{FILE}");
    let token = hf_hub::api::tokio::CancellationToken::new();
    *CANCEL_TOKEN.lock().unwrap_or_else(|e| e.into_inner()) = Some(token.clone());
    let told = Mutex::new(Instant::now());
    let result = crate::managers::model::ModelManager::fetch_resumable(
        &url,
        &partial,
        SIZE,
        SHA256,
        &token,
        &|done| {
            let mut told = told.lock().unwrap_or_else(|e| e.into_inner());
            if told.elapsed() >= Duration::from_millis(250) {
                lock().done = done;
                announce(app);
                *told = Instant::now();
            }
        },
    )
    .await;
    CANCEL_TOKEN
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();
    match result {
        Ok(true) => {
            std::fs::rename(&partial, &path)?;
            Ok(())
        }
        Ok(false) => Err(anyhow!("Cancelled")),
        Err(e) => Err(e),
    }
}

pub fn cancel() {
    CANCEL.store(true, Ordering::Relaxed);
    if let Some(token) = CANCEL_TOKEN
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
    {
        token.cancel();
    }
}

pub fn delete(app: &AppHandle) -> Result<()> {
    let path = model_path(app)?;
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    announce(app);
    Ok(())
}

/// Who spoke when in the 16 kHz mono WAV at `audio`. Blocks until the
/// speaker model is done with it, which takes a while for a long meeting.
pub fn turns(app: &AppHandle, audio: &Path) -> Result<Vec<Turn>> {
    let started = Instant::now();
    let turns = turns_many(app, &[audio.to_path_buf()])?
        .into_iter()
        .next()
        .unwrap_or_default();
    info!(
        "Found {} speaker turns in {:?}",
        turns.len(),
        started.elapsed()
    );
    Ok(turns)
}

/// Who spoke when in each file, with the model loaded once for them all.
fn turns_many(app: &AppHandle, files: &[PathBuf]) -> Result<Vec<Vec<Turn>>> {
    let tm = app
        .try_state::<Arc<TranscriptionManager>>()
        .ok_or_else(|| anyhow!("Transcription isn't ready"))?;
    Ok(tm
        .diarize(model_path(app)?, files, false)?
        .into_iter()
        .map(|segments| {
            segments
                .into_iter()
                .filter(|s| s.speaker_id > 0 && s.t1_ms > s.t0_ms)
                .map(|s| Turn {
                    speaker: s.speaker_id,
                    t0_ms: s.t0_ms,
                    t1_ms: s.t1_ms,
                })
                .collect()
        })
        .collect())
}

// ---- More than four voices ---------------------------------------------

/// After a big meeting, at most this many comparisons run, each one voice
/// played before up to three others.
const MAX_COMPARE_FILES: usize = 120;
const PER_FILE: usize = 3;

fn samples_ms(samples: usize) -> i64 {
    samples as i64 * 1000 / RATE as i64
}

/// Who spoke when, for any number of voices. A recording where the model
/// filled all four of its voices is heard again in short windows, and the
/// windows' voices are joined into the meeting's: by who talks where two
/// windows overlap, then by playing voices that never share a window one
/// after the other through the model. With at most `cap` people to tell
/// apart, fewer than the model holds, the whole recording is enough.
/// Blocks for a while.
pub fn turns_wide(app: &AppHandle, audio: &Path, cap: Option<usize>) -> Result<Vec<Turn>> {
    let whole = turns(app, audio)?;
    let mut voices: Vec<i32> = whole.iter().map(|t| t.speaker).collect();
    voices.sort_unstable();
    voices.dedup();
    let reader = hound::WavReader::open(audio)?;
    let duration = samples_ms(reader.duration() as usize);
    drop(reader);
    let windows = notes_rules::voice_windows(duration);
    let few = cap.is_some_and(|c| c < notes_rules::MODEL_VOICES);
    if voices.len() < notes_rules::MODEL_VOICES || few || windows.len() < 2 {
        return Ok(whole);
    }
    let started = Instant::now();
    let dir = voices_dir(app)?.join(format!("windows-{}", now()));
    std::fs::create_dir_all(&dir)?;
    let result = windowed(app, audio, &windows, &dir);
    let _ = std::fs::remove_dir_all(&dir);
    let turns = result?;
    let mut found: Vec<i32> = turns.iter().map(|t| t.speaker).collect();
    found.sort_unstable();
    found.dedup();
    info!(
        "Heard {} windows and found {} voices in {:?}",
        windows.len(),
        found.len(),
        started.elapsed()
    );
    Ok(turns)
}

fn windowed(app: &AppHandle, audio: &Path, spans: &[(i64, i64)], dir: &Path) -> Result<Vec<Turn>> {
    let mut files = Vec::new();
    for (i, &(from, to)) in spans.iter().enumerate() {
        let path = dir.join(format!("w{i}.wav"));
        write_wav(&path, &read_wav(audio, from, to - from)?)?;
        files.push(path);
    }
    let windows: Vec<notes_rules::WindowTurns> = spans
        .iter()
        .zip(turns_many(app, &files)?)
        .map(|(&(from, to), turns)| notes_rules::WindowTurns {
            start_ms: from,
            end_ms: to,
            turns: turns
                .into_iter()
                .map(|t| Turn {
                    speaker: t.speaker,
                    t0_ms: t.t0_ms + from,
                    t1_ms: t.t1_ms + from,
                })
                .collect(),
        })
        .collect();
    let groups = check_links(
        app,
        audio,
        &windows,
        notes_rules::link_windows(&windows),
        dir,
    )?;
    let apart: Vec<usize> = (0..groups.count).collect();
    let linked = notes_rules::stitch_turns(&windows, &groups, &apart);
    let clips = notes_rules::clearest_turns(&linked, groups.count, MIN_CLIP_MS, COMPARE_MS);
    let has_clip: Vec<bool> = clips.iter().map(Option::is_some).collect();

    // Each voice, then up to three it might be, in one file.
    let mut jobs: Vec<(usize, Vec<usize>)> = Vec::new();
    for (a, b) in notes_rules::voice_pairs(
        &groups,
        &has_clip,
        &notes_rules::speech_ms(&linked, groups.count),
    ) {
        match jobs.last_mut() {
            Some((first, others)) if *first == a && others.len() < PER_FILE => others.push(b),
            _ => jobs.push((a, vec![b])),
        }
    }
    if jobs.len() > MAX_COMPARE_FILES {
        warn!(
            "{} voice comparisons needed; running the first {MAX_COMPARE_FILES}",
            jobs.len()
        );
        jobs.truncate(MAX_COMPARE_FILES);
    }
    let clip_of = |g: usize| -> Result<Vec<i16>> {
        let (from, to) = clips[g].ok_or_else(|| anyhow!("No clip for that voice"))?;
        read_wav(audio, from, to - from)
    };
    let gap = vec![0i16; (GAP_MS * RATE as i64 / 1000) as usize];
    let mut files = Vec::new();
    let mut layout: Vec<(i64, Vec<(i64, i64)>)> = Vec::new();
    for (k, (a, others)) in jobs.iter().enumerate() {
        let mut samples = clip_of(*a)?;
        let a_end = samples_ms(samples.len());
        let mut spans = Vec::new();
        for &b in others {
            samples.extend_from_slice(&gap);
            let start = samples_ms(samples.len());
            samples.extend(clip_of(b)?);
            spans.push((start, samples_ms(samples.len())));
        }
        let path = dir.join(format!("c{k}.wav"));
        write_wav(&path, &samples)?;
        files.push(path);
        layout.push((a_end, spans));
    }
    let heard = if files.is_empty() {
        Vec::new()
    } else {
        turns_many(app, &files)?
    };
    let mut scores = Vec::new();
    for (((a, others), (a_end, spans)), turns) in jobs.iter().zip(&layout).zip(&heard) {
        for (&b, score) in others
            .iter()
            .zip(notes_rules::same_voices(turns, *a_end, spans))
        {
            scores.push((*a, b, score));
        }
    }
    let voice = notes_rules::join_voices(&groups, &scores);
    Ok(notes_rules::stitch_turns(&windows, &groups, &voice))
}

/// Plays each voice linked across two windows next to the voice it was
/// linked to, on stretches the windows don't share, and drops the links
/// the model hears as two people: a window that mixed up two voices
/// mustn't hand the mix-up on.
fn check_links(
    app: &AppHandle,
    audio: &Path,
    windows: &[notes_rules::WindowTurns],
    groups: notes_rules::VoiceGroups,
    dir: &Path,
) -> Result<notes_rules::VoiceGroups> {
    let links = notes_rules::window_links(&groups);
    let clips = notes_rules::link_clips(windows, &links, MIN_CLIP_MS, COMPARE_MS);
    let gap = vec![0i16; (GAP_MS * RATE as i64 / 1000) as usize];
    let mut files = Vec::new();
    let mut checks = Vec::new();
    for (k, (link, clip)) in links.iter().zip(&clips).enumerate() {
        let Some(((a0, a1), (b0, b1))) = *clip else {
            continue;
        };
        let mut samples = read_wav(audio, a0, a1 - a0)?;
        let a_end = samples_ms(samples.len());
        samples.extend_from_slice(&gap);
        let b_start = samples_ms(samples.len());
        samples.extend(read_wav(audio, b0, b1 - b0)?);
        let path = dir.join(format!("l{k}.wav"));
        write_wav(&path, &samples)?;
        files.push(path);
        checks.push((*link, a_end, b_start, samples_ms(samples.len())));
    }
    if files.is_empty() {
        return Ok(groups);
    }
    let broken: Vec<(usize, i32)> = checks
        .iter()
        .zip(turns_many(app, &files)?)
        .filter(|((_, a_end, b_start, b_end), turns)| {
            notes_rules::same_voice(turns, *a_end, *b_start, *b_end) < notes_rules::LINK_DOUBT
        })
        .map(|(((w, voice, _), ..), _)| (*w, *voice))
        .collect();
    if !broken.is_empty() {
        info!(
            "{} of {} window links didn't hold up",
            broken.len(),
            checks.len()
        );
    }
    Ok(notes_rules::unlink(&groups, &broken))
}

// ---- Saved voices -------------------------------------------------------

/// Up to this much of a voice is saved, and compared.
const CLIP_MS: i64 = 8_000;
/// A line shorter than this is too little of a voice to keep.
const MIN_CLIP_MS: i64 = 2_500;
/// Each side of a comparison, and the silence between them.
const COMPARE_MS: i64 = 6_000;
const GAP_MS: i64 = 600;
/// The saved voices a meeting is compared with, the most recently heard.
const COMPARE_VOICES: usize = 12;

fn voices_dir(app: &AppHandle) -> Result<PathBuf> {
    Ok(crate::portable::app_data_dir(app)?.join("voices"))
}

const WAV: hound::WavSpec = hound::WavSpec {
    channels: 1,
    sample_rate: RATE as u32,
    bits_per_sample: 16,
    sample_format: hound::SampleFormat::Int,
};

fn write_wav(path: &Path, samples: &[i16]) -> Result<()> {
    let mut writer = hound::WavWriter::create(path, WAV)?;
    for &s in samples {
        writer.write_sample(s)?;
    }
    writer.finalize()?;
    Ok(())
}

fn read_wav(path: &Path, from_ms: i64, ms: i64) -> Result<Vec<i16>> {
    let mut reader = hound::WavReader::open(path)?;
    let total = reader.duration();
    let at = |ms: i64| (ms.max(0) as u64 * RATE as u64 / 1000).min(total as u64) as u32;
    let (start, end) = (at(from_ms), at(from_ms + ms));
    reader.seek(start)?;
    Ok(reader
        .samples::<i16>()
        .take(end.saturating_sub(start) as usize)
        .map_while(Result::ok)
        .collect())
}

/// The clearest stretch of one voice in `audio`: its longest line that no
/// one else talks over, cut to `max_ms`. None while it has no such line.
fn clip(audio: &Path, lines: &[NoteLine], speaker: i64, max_ms: i64) -> Result<Option<Vec<i16>>> {
    let clear = |l: &&NoteLine| {
        !lines
            .iter()
            .any(|o| o.speaker != Some(speaker) && o.at_ms < l.end_ms && o.end_ms > l.at_ms)
    };
    let Some(best) = lines
        .iter()
        .filter(|l| l.speaker == Some(speaker) && l.end_ms - l.at_ms >= MIN_CLIP_MS)
        .filter(clear)
        .max_by_key(|l| (l.end_ms - l.at_ms).min(max_ms))
    else {
        return Ok(None);
    };
    let samples = read_wav(audio, best.at_ms, (best.end_ms - best.at_ms).min(max_ms))?;
    Ok((samples.len() as i64 >= MIN_CLIP_MS * RATE as i64 / 1000).then_some(samples))
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Keeps a clip of the voice just named, from `audio` (the recording, or
/// its side with the voices while it runs), so later meetings know it. A
/// saved voice keeps its clip unless this one is longer.
pub fn remember(
    app: &AppHandle,
    note_id: i64,
    speaker: i64,
    name: &str,
    audio: &Path,
) -> Result<()> {
    let name = name.trim();
    if name.is_empty() || !get_settings(app).notes_remember_voices {
        return Ok(());
    }
    let m = crate::notes::manager(app);
    let conn = m.conn()?;
    let lines = store::lines(&conn, note_id)?;
    let Some(samples) = clip(audio, &lines, speaker, CLIP_MS)? else {
        return Ok(());
    };
    let ms = samples.len() as i64 * 1000 / RATE as i64;
    if store::voice_named(&conn, name)?.is_some_and(|v| v.ms > ms) {
        return Ok(());
    }
    let dir = voices_dir(app)?;
    std::fs::create_dir_all(&dir)?;
    let file = format!("voice-{note_id}-{speaker}-{}.wav", now());
    write_wav(&dir.join(&file), &samples)?;
    if let Some(old) = store::save_voice(&conn, name, &file, ms, now())? {
        let _ = std::fs::remove_file(dir.join(old));
    }
    info!("Saved a voice clip of {ms} ms");
    Ok(())
}

/// Voices already compared in a note, so each live pass doesn't ask again.
static TRIED: Mutex<Vec<(i64, i64)>> = Mutex::new(Vec::new());

/// Names the note's unnamed voices that sound like a saved one. Returns
/// how many were recognised.
pub fn recognise(app: &AppHandle, note_id: i64, audio: &Path) -> Result<usize> {
    if !get_settings(app).notes_remember_voices {
        return Ok(0);
    }
    let m = crate::notes::manager(app);
    let conn = m.conn()?;
    let Some(note) = store::get(&conn, note_id)? else {
        return Ok(0);
    };
    let named: Vec<String> = note
        .speakers
        .iter()
        .map(|s| s.name.to_lowercase())
        .collect();
    let mut saved: Vec<Voice> = store::voices(&conn)?
        .into_iter()
        .filter(|v| !named.contains(&v.name.to_lowercase()))
        .collect();
    saved.truncate(COMPARE_VOICES);
    if saved.is_empty() {
        return Ok(0);
    }
    let lines = store::lines(&conn, note_id)?;
    let mut voices: Vec<i64> = lines.iter().filter_map(|l| l.speaker).collect();
    voices.sort_unstable();
    voices.dedup();
    let tried = TRIED.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let mut unnamed: Vec<(i64, Vec<i16>)> = Vec::new();
    for voice in voices {
        if note.speakers.iter().any(|s| s.speaker == voice) || tried.contains(&(note_id, voice)) {
            continue;
        }
        if let Some(samples) = clip(audio, &lines, voice, COMPARE_MS)? {
            unnamed.push((voice, samples));
        }
    }
    if unnamed.is_empty() {
        return Ok(0);
    }

    // Each unnamed voice, a breath of silence, then each saved voice.
    let dir = voices_dir(app)?.join(format!("compare-{note_id}"));
    std::fs::create_dir_all(&dir)?;
    let result = (|| -> Result<Vec<Vec<f64>>> {
        let clips: Vec<Vec<i16>> = saved
            .iter()
            .map(|v| read_wav(&voices_dir(app)?.join(&v.file), 0, COMPARE_MS))
            .collect::<Result<_>>()?;
        let gap = vec![0i16; (GAP_MS * RATE as i64 / 1000) as usize];
        let mut files = Vec::new();
        let mut spans = Vec::new();
        for (i, (_, a)) in unnamed.iter().enumerate() {
            for (j, b) in clips.iter().enumerate() {
                let path = dir.join(format!("{i}-{j}.wav"));
                write_wav(&path, &[a.as_slice(), &gap, b].concat())?;
                let a_end = a.len() as i64 * 1000 / RATE as i64;
                let b_start = a_end + GAP_MS;
                spans.push((
                    a_end,
                    b_start,
                    b_start + b.len() as i64 * 1000 / RATE as i64,
                ));
                files.push(path);
            }
        }
        let turns = turns_many(app, &files)?;
        Ok(unnamed
            .iter()
            .enumerate()
            .map(|(i, _)| {
                (0..clips.len())
                    .map(|j| {
                        let k = i * clips.len() + j;
                        let (a_end, b_start, b_end) = spans[k];
                        turns
                            .get(k)
                            .map_or(0.0, |t| notes_rules::same_voice(t, a_end, b_start, b_end))
                    })
                    .collect()
            })
            .collect())
    })();
    let _ = std::fs::remove_dir_all(&dir);
    let scores = result?;

    TRIED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .extend(unnamed.iter().map(|(voice, _)| (note_id, *voice)));
    let mut found = 0;
    for ((voice, _), pick) in unnamed.iter().zip(notes_rules::pick_voices(&scores)) {
        if let Some(j) = pick {
            store::recognise_speaker(&conn, note_id, *voice, &saved[j], now())?;
            found += 1;
        }
    }
    if found > 0 {
        info!("Recognised {found} saved voices");
        m.changed();
    }
    Ok(found)
}

/// The saved voices, for Settings.
pub fn list(app: &AppHandle) -> Result<Vec<Voice>> {
    store::voices(&crate::notes::manager(app).conn()?)
}

/// Forgets a saved voice and removes its clip.
pub fn forget(app: &AppHandle, id: i64) -> Result<()> {
    let conn = crate::notes::manager(app).conn()?;
    if let Some(file) = store::delete_voice(&conn, id)? {
        if let Err(e) = std::fs::remove_file(voices_dir(app)?.join(file)) {
            warn!("Couldn't remove a voice clip: {e}");
        }
    }
    Ok(())
}
