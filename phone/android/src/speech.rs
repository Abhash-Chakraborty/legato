//! Speech on the phone: one transcribe-cpp model held in memory, CPU only.

use std::path::Path;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Mutex, Once};

use legato_phone::pieces;
use transcribe_cpp::{Model, RunOptions, Session, SessionOptions};

/// CPU threads for the model: every core, up to eight. Phones pair a few
/// fast cores with slow ones, and timed on a Redmi Pad (two fast, six slow)
/// eight threads wrote up 12 s of speech 20-25% sooner than four, in every
/// model tried; the work is over in a second or two either way.
fn threads() -> i32 {
    match THREADS.load(Ordering::Relaxed) {
        0 => {
            let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
            cores.clamp(2, 8) as i32
        }
        n => n,
    }
}

/// Threads asked for by `set_threads`; 0 picks them from the cores. Read
/// when a model loads.
static THREADS: AtomicI32 = AtomicI32::new(0);

/// Sets the threads the next model load uses (0 for the default), for
/// timing a model at several counts.
pub fn set_threads(n: i32) {
    THREADS.store(n.max(0), Ordering::Relaxed);
}

use crate::CoreError;

static BACKENDS: Once = Once::new();

#[derive(Default)]
pub struct Speech {
    session: Mutex<Option<Session>>,
}

impl Speech {
    /// Loads the model at `path` and keeps a session on it; a later call
    /// replaces the earlier model. Returns the model's architecture.
    pub fn load(&self, path: &Path) -> Result<String, CoreError> {
        BACKENDS.call_once(|| {
            transcribe_cpp::init_logging();
            // Static CPU backend: nothing to scan for, so a failure here only
            // shows up when the model fails to load below.
            let _ = transcribe_cpp::init_backends_default();
        });
        let model = Model::load(path)
            .map_err(|e| CoreError::Failed(format!("Couldn't load the model: {e}")))?;
        let arch = model.arch();
        let session = model
            .session_with(&SessionOptions {
                n_threads: threads(),
                ..SessionOptions::default()
            })
            .map_err(|e| CoreError::Failed(format!("Couldn't start the model: {e}")))?;
        *self.session.lock().unwrap() = Some(session);
        Ok(arch)
    }

    /// Lets go of the model, for the memory and the battery; the next
    /// recording loads it again.
    pub fn unload(&self) {
        *self.session.lock().unwrap() = None;
    }

    pub fn loaded(&self) -> bool {
        self.session.lock().unwrap().is_some()
    }

    /// The recording's text. A long one goes in pieces cut at its pauses: the
    /// small models stop writing after half a minute or so of speech and
    /// silently drop the rest.
    pub fn transcribe(
        &self,
        samples: &[f32],
        language: Option<String>,
    ) -> Result<String, CoreError> {
        let mut guard = self.session.lock().unwrap();
        let session = guard
            .as_mut()
            .ok_or_else(|| CoreError::Failed("No speech model loaded".into()))?;
        let options = RunOptions {
            language,
            ..RunOptions::default()
        };
        let mut texts = Vec::new();
        for range in pieces::pieces(samples) {
            let piece = &samples[range];
            if pieces::quiet(piece) {
                continue;
            }
            let transcript = session
                .run(piece, &options)
                .map_err(|e| CoreError::Failed(format!("Transcription failed: {e}")))?;
            texts.push(transcript.text);
        }
        Ok(pieces::join(&texts))
    }
}
