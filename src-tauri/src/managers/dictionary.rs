//! The Dictionary as the app uses it: the store behind the Dictionary page,
//! the snapshot the transcription pipeline reads, and Undo for Fix a word.

use super::dictionary_store::{
    self as store, DictionaryEntry, DictionarySource, EntryInput, FixApp, ImportSummary, Learned,
    PendingWord,
};
use super::history_store::{AppliedFix, FixSource, TextSnapshot};
use crate::audio_toolkit::text::{CompiledDictionary, DictionaryCorrection};
use crate::learn_rules::Spelling;
use anyhow::Result;
use chrono::Utc;
use log::{error, info, warn};
use once_cell::sync::Lazy;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use tauri::AppHandle;
use tauri_specta::Event;

/// What the pipeline needs from the Dictionary for one transcription.
#[derive(Default)]
pub struct Lexicon {
    /// Words for the model's vocabulary, in priority order.
    pub hint: Vec<String>,
    pub corrector: CompiledDictionary,
}

static LEXICON: Lazy<RwLock<Arc<Lexicon>>> = Lazy::new(Default::default);

/// The Dictionary as of the last change.
pub fn lexicon() -> Arc<Lexicon> {
    LEXICON
        .read()
        .map(|l| l.clone())
        .unwrap_or_else(|e| e.into_inner().clone())
}

/// The pipeline's corrections, as history keeps them.
pub fn applied_fixes(fixes: &[DictionaryCorrection]) -> Vec<AppliedFix> {
    fixes
        .iter()
        .map(|f| AppliedFix {
            from: f.from.clone(),
            to: f.to.clone(),
            source: if f.heard {
                FixSource::Heard
            } else {
                FixSource::Match
            },
        })
        .collect()
}

/// Sent whenever a word is added, changed or removed.
#[derive(Clone, Debug, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct DictionaryChanged;

/// What a Fix a word or a review changed, kept until it is undone or
/// forgotten.
pub struct FixUndo {
    pub rows: Vec<TextSnapshot>,
    pub learned: Vec<Learned>,
    /// Words taken off Needs review.
    pub pending: Vec<PendingWord>,
}

/// Fixes that can still be undone; old ones are dropped.
const UNDO_KEEP: usize = 20;

pub struct DictionaryManager {
    app_handle: AppHandle,
    db_path: PathBuf,
    undo: Mutex<(u32, HashMap<u32, FixUndo>)>,
}

impl DictionaryManager {
    /// Opens the Dictionary in the history database, which `HistoryManager`
    /// has already migrated.
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        let db_path = crate::portable::app_data_dir(app_handle)?.join("history.db");
        let manager = Self {
            app_handle: app_handle.clone(),
            db_path,
            undo: Mutex::new((0, HashMap::new())),
        };
        manager.refresh()?;
        Ok(manager)
    }

    fn conn(&self) -> Result<Connection> {
        Ok(Connection::open(&self.db_path)?)
    }

    /// Rebuilds the pipeline's snapshot from the database.
    fn refresh(&self) -> Result<()> {
        let conn = self.conn()?;
        let entries = store::list(&conn)?;
        let context = store::context_for(&conn, &entries, &crate::spelling::is_word)
            .unwrap_or_else(|e| {
                warn!("Couldn't gather the Dictionary's context words: {e}");
                Default::default()
            });
        let (hint, corrector) = store::compile_with(&entries, &context);
        let corrector = corrector.with_word_check(crate::spelling::checker());
        let lexicon = Arc::new(Lexicon { hint, corrector });
        match LEXICON.write() {
            Ok(mut slot) => *slot = lexicon,
            Err(e) => *e.into_inner() = lexicon,
        }
        Ok(())
    }

    /// Call after every write: the next dictation uses the change, and the
    /// UI reloads.
    pub(crate) fn changed(&self) {
        if let Err(e) = self.refresh() {
            error!("Failed to reload the Dictionary: {}", e);
        }
        if let Err(e) = DictionaryChanged.emit(&self.app_handle) {
            error!("Failed to emit dictionary-changed: {}", e);
        }
    }

    fn now() -> i64 {
        Utc::now().timestamp()
    }

    pub fn list(&self) -> Result<Vec<DictionaryEntry>> {
        store::list(&self.conn()?)
    }

    pub fn add(&self, input: &EntryInput, source: DictionarySource) -> Result<DictionaryEntry> {
        let entry = store::add(&mut self.conn()?, input, source, Self::now())?;
        self.changed();
        Ok(entry)
    }

    pub fn update(&self, id: i64, input: &EntryInput) -> Result<DictionaryEntry> {
        let entry = store::update(&mut self.conn()?, id, input)?;
        self.changed();
        Ok(entry)
    }

    pub fn set_pinned(&self, id: i64, pinned: bool) -> Result<DictionaryEntry> {
        let entry = store::set_pinned(&mut self.conn()?, id, pinned)?;
        self.changed();
        Ok(entry)
    }

    pub fn delete(&self, ids: &[i64]) -> Result<Vec<DictionaryEntry>> {
        let removed = store::delete(&mut self.conn()?, ids)?;
        self.changed();
        Ok(removed)
    }

    pub fn restore(&self, entries: &[DictionaryEntry]) -> Result<()> {
        store::restore(&mut self.conn()?, entries)?;
        self.changed();
        Ok(())
    }

    pub fn import(&self, rows: &[EntryInput]) -> Result<ImportSummary> {
        let summary = store::import(
            &mut self.conn()?,
            rows,
            DictionarySource::Imported,
            Self::now(),
        )?;
        info!(
            "Imported words: {} added, {} updated, {} skipped",
            summary.added, summary.updated, summary.skipped
        );
        self.changed();
        Ok(summary)
    }

    /// Counts the fixes a dictation got, in the app it went to.
    pub fn record_fixes(&self, words: &[&str], app: FixApp<'_>) {
        if words.is_empty() {
            return;
        }
        match self.conn() {
            Ok(mut conn) => match store::record_fixes(&mut conn, words, app, Self::now()) {
                Ok(()) => self.changed(),
                Err(e) => error!("Failed to count Dictionary fixes: {}", e),
            },
            Err(e) => error!("Failed to open the Dictionary: {}", e),
        }
    }

    pub fn learn(
        &self,
        word: &str,
        heard: Option<&str>,
        source: DictionarySource,
        fixed: bool,
        app: FixApp<'_>,
    ) -> Result<Learned> {
        let learned = store::learn(
            &mut self.conn()?,
            word,
            heard,
            source,
            fixed,
            app,
            Self::now(),
        )?;
        self.changed();
        Ok(learned)
    }

    pub fn unlearn(&self, learned: &Learned) -> Result<()> {
        store::unlearn(&mut self.conn()?, learned)?;
        self.changed();
        Ok(())
    }

    pub fn pending(&self) -> Result<Vec<PendingWord>> {
        store::pending(&self.conn()?)
    }

    pub fn never(&self) -> Result<Vec<String>> {
        store::never(&self.conn()?)
    }

    /// Puts corrections under Needs review; returns the ones that weren't
    /// already covered.
    pub fn add_pending(
        &self,
        words: &[(String, String, String)],
        app: FixApp<'_>,
    ) -> Result<Vec<PendingWord>> {
        let conn = self.conn()?;
        let mut added = Vec::new();
        for (word, heard, context) in words {
            if let Some(p) = store::add_pending(&conn, word, heard, context, app, Self::now())? {
                added.push(p);
            }
        }
        if !added.is_empty() {
            self.changed();
        }
        Ok(added)
    }

    pub fn is_known(&self, word: &str, heard: &str) -> Result<bool> {
        store::is_known(&self.conn()?, word, heard)
    }

    pub fn remove_pending(&self, ids: &[i64]) -> Result<Vec<PendingWord>> {
        let gone = store::remove_pending(&mut self.conn()?, ids)?;
        self.changed();
        Ok(gone)
    }

    pub fn restore_pending(&self, words: &[PendingWord]) -> Result<()> {
        store::restore_pending(&mut self.conn()?, words)?;
        self.changed();
        Ok(())
    }

    /// Terms the model wrote more than one way lately.
    pub fn spellings(&self) -> Result<Vec<Spelling>> {
        store::spellings(&self.conn()?, Self::now())
    }

    /// Adds a term written more than one way, with its other forms as ways
    /// it's heard.
    pub fn add_spelling(&self, word: &str, forms: &[String]) -> Result<DictionaryEntry> {
        let input = EntryInput {
            text: word.to_string(),
            heard: forms
                .iter()
                .filter(|f| !f.eq_ignore_ascii_case(word))
                .cloned()
                .collect(),
            ..Default::default()
        };
        self.add(&input, DictionarySource::Spelled)
    }

    pub fn add_never(&self, word: &str) -> Result<()> {
        store::add_never(&mut self.conn()?, word, Self::now())?;
        self.changed();
        Ok(())
    }

    pub fn remove_never(&self, word: &str) -> Result<()> {
        store::remove_never(&self.conn()?, word)?;
        self.changed();
        Ok(())
    }

    /// Keeps a fix for Undo and returns its token.
    pub fn stash_undo(&self, undo: FixUndo) -> u32 {
        let mut guard = self.undo.lock().unwrap_or_else(|e| e.into_inner());
        let (next, stash) = &mut *guard;
        *next = next.wrapping_add(1);
        let token = *next;
        stash.insert(token, undo);
        if stash.len() > UNDO_KEEP {
            if let Some(oldest) = stash.keys().copied().min() {
                stash.remove(&oldest);
            }
        }
        token
    }

    pub fn take_undo(&self, token: u32) -> Option<FixUndo> {
        self.undo
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .1
            .remove(&token)
    }
}
