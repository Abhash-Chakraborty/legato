//! Snippets as the app uses them: the store behind the Snippets page and the
//! snapshot the dictation pipeline matches against.

use super::snippet_store::{self as store, Snippet, SnippetImport, SnippetInput};
use crate::snippet_rules::SnippetRule;
use anyhow::Result;
use chrono::Utc;
use log::error;
use once_cell::sync::Lazy;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tauri::AppHandle;
use tauri_specta::Event;

/// What the pipeline needs: the snippets and their match rules.
#[derive(Default)]
pub struct SnippetSet {
    pub snippets: Vec<Snippet>,
    pub rules: Vec<SnippetRule>,
}

impl SnippetSet {
    pub fn text_of(&self, id: i64) -> Option<&str> {
        self.snippets
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.text.as_str())
    }
}

static SNIPPETS: Lazy<RwLock<Arc<SnippetSet>>> = Lazy::new(Default::default);

/// The snippets as of the last change.
pub fn snippet_set() -> Arc<SnippetSet> {
    SNIPPETS
        .read()
        .map(|s| s.clone())
        .unwrap_or_else(|e| e.into_inner().clone())
}

/// Sent whenever a snippet is added, changed, used or removed.
#[derive(Clone, Debug, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct SnippetsChanged;

pub struct SnippetManager {
    app_handle: AppHandle,
    db_path: PathBuf,
}

impl SnippetManager {
    /// Opens the snippets in the history database, which `HistoryManager`
    /// has already migrated.
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        let db_path = crate::portable::app_data_dir(app_handle)?.join("history.db");
        let manager = Self {
            app_handle: app_handle.clone(),
            db_path,
        };
        manager.refresh()?;
        Ok(manager)
    }

    fn conn(&self) -> Result<Connection> {
        Ok(Connection::open(&self.db_path)?)
    }

    fn refresh(&self) -> Result<()> {
        let snippets = store::list(&self.conn()?)?;
        let rules = store::rules_of(&snippets);
        let set = Arc::new(SnippetSet { snippets, rules });
        match SNIPPETS.write() {
            Ok(mut slot) => *slot = set,
            Err(e) => *e.into_inner() = set,
        }
        Ok(())
    }

    /// Call after every write: the next dictation uses the change, and the
    /// UI reloads.
    pub(crate) fn changed(&self) {
        if let Err(e) = self.refresh() {
            error!("Failed to reload snippets: {}", e);
        }
        if let Err(e) = SnippetsChanged.emit(&self.app_handle) {
            error!("Failed to emit snippets-changed: {}", e);
        }
    }

    fn now() -> i64 {
        Utc::now().timestamp()
    }

    pub fn list(&self) -> Result<Vec<Snippet>> {
        store::list(&self.conn()?)
    }

    pub fn add(&self, input: &SnippetInput) -> Result<Snippet> {
        let snippet = store::add(&mut self.conn()?, input, Self::now())?;
        self.changed();
        Ok(snippet)
    }

    pub fn update(&self, id: i64, input: &SnippetInput) -> Result<Snippet> {
        let snippet = store::update(&mut self.conn()?, id, input)?;
        self.changed();
        Ok(snippet)
    }

    pub fn delete(&self, ids: &[i64]) -> Result<Vec<Snippet>> {
        let gone = store::delete(&mut self.conn()?, ids)?;
        self.changed();
        Ok(gone)
    }

    pub fn restore(&self, snippets: &[Snippet]) -> Result<()> {
        store::restore(&mut self.conn()?, snippets)?;
        self.changed();
        Ok(())
    }

    /// Counts the snippets a dictation pasted.
    pub fn record_use(&self, ids: &[i64], app: Option<(&str, Option<&str>)>) {
        if ids.is_empty() {
            return;
        }
        let result = self
            .conn()
            .and_then(|mut conn| store::record_use(&mut conn, ids, app, Self::now()));
        match result {
            Ok(()) => self.changed(),
            Err(e) => error!("Failed to count snippet uses: {}", e),
        }
    }

    pub fn import(&self, inputs: &[SnippetInput]) -> Result<SnippetImport> {
        let summary = store::import(&mut self.conn()?, inputs, Self::now())?;
        self.changed();
        Ok(summary)
    }
}
