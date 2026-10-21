//! Telling voices apart in Notes: the speaker model, downloaded on request
//! from Settings › Context & calls, and who spoke when in a recording. The
//! model runs in a transcription worker of its own (`engine_supervisor`).
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
use futures_util::StreamExt;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use specta::Type;
use std::io::Write;
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
const FILE: &str = "diar_streaming_sortformer_4spk-v2.1-Q8_0.gguf";
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

async fn fetch(app: &AppHandle) -> Result<()> {
    let path = model_path(app)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let partial = path.with_extension("gguf.partial");
    let url = format!("https://huggingface.co/{REPO}/resolve/{REVISION}/{FILE}");
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        // A stalled transfer fails instead of hanging forever.
        .read_timeout(Duration::from_secs(60))
        .build()?;
    let response = client.get(&url).send().await?.error_for_status()?;
    let mut file = std::fs::File::create(&partial)?;
    let mut hasher = Sha256::new();
    let mut done = 0u64;
    let mut told = Instant::now();
    let mut stream = response.bytes_stream();
    let result: Result<()> = async {
        while let Some(chunk) = stream.next().await {
            if CANCEL.load(Ordering::Relaxed) {
                return Err(anyhow!("Cancelled"));
            }
            let chunk = chunk?;
            file.write_all(&chunk)?;
            hasher.update(&chunk);
            done += chunk.len() as u64;
            if told.elapsed() >= Duration::from_millis(250) {
                lock().done = done;
                announce(app);
                told = Instant::now();
            }
        }
        file.flush()?;
        let hash = format!("{:x}", hasher.finalize_reset());
        if done != SIZE || hash != SHA256 {
            return Err(anyhow!("The download arrived damaged; try again"));
        }
        Ok(())
    }
    .await;
    drop(file);
    match result {
        Ok(()) => {
            std::fs::rename(&partial, &path)?;
            Ok(())
        }
        Err(e) => {
            let _ = std::fs::remove_file(&partial);
            Err(e)
        }
    }
}

pub fn cancel() {
    CANCEL.store(true, Ordering::Relaxed);
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
