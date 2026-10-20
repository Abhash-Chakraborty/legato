//! Speech on the phone: one transcribe-cpp model held in memory, CPU only.

use std::path::Path;
use std::sync::{Mutex, Once};

use transcribe_cpp::{Model, RunOptions, Session};

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
            .session()
            .map_err(|e| CoreError::Failed(format!("Couldn't start the model: {e}")))?;
        *self.session.lock().unwrap() = Some(session);
        Ok(arch)
    }

    pub fn transcribe(&self, samples: &[f32]) -> Result<String, CoreError> {
        let mut guard = self.session.lock().unwrap();
        let session = guard
            .as_mut()
            .ok_or_else(|| CoreError::Failed("No speech model loaded".into()))?;
        let transcript = session
            .run(samples, &RunOptions::default())
            .map_err(|e| CoreError::Failed(format!("Transcription failed: {e}")))?;
        Ok(transcript.text.trim().to_string())
    }
}
