//! What the PC does to a dictation after its model, for text written up by
//! the phone's own model: the Dictionary, fillers, snippets, cleanup and the
//! Style tone for the kind of app, all from what syncs from the PC.

use anyhow::Result;
use chrono::{DateTime, Local};
use rusqlite::{Connection, OptionalExtension};

use crate::audio_toolkit::text::{
    normalize_transcription_output, remove_filler_words, OutputLanguageEvidence,
};
use crate::cleanup_rules::{self, CleanupRules};
use crate::managers::{dictionary_store, snippet_store};
use crate::snippet_rules::{self, Variables};
use crate::style_rules::{self, Tone};

/// How close a heard word must be to a Dictionary word to become it; the
/// PC's default.
const WORD_CORRECTION_THRESHOLD: f64 = 0.18;

/// The phone's model is English only.
const LANGUAGE: &str = "en";

/// Android apps by package, sorted into the PC's kinds of app.
const APPS: &[(&str, &str)] = &[
    ("com.whatsapp", "personal"),
    ("com.whatsapp.w4b", "personal"),
    ("org.telegram.messenger", "personal"),
    ("org.thoughtcrime.securesms", "personal"),
    ("com.google.android.apps.messaging", "personal"),
    ("com.facebook.orca", "personal"),
    ("com.instagram.android", "personal"),
    ("com.discord", "personal"),
    ("com.Slack", "work"),
    ("com.microsoft.teams", "work"),
    ("com.linkedin.android", "work"),
    ("com.google.android.gm", "email"),
    ("com.microsoft.office.outlook", "email"),
    ("ch.protonmail.android", "email"),
    ("com.fsck.k9", "email"),
    ("net.thunderbird.android", "email"),
    ("com.google.android.apps.docs.editors.docs", "documents"),
    ("com.google.android.keep", "documents"),
    ("com.microsoft.office.word", "documents"),
    ("com.microsoft.office.onenote", "documents"),
    ("notion.id", "documents"),
    ("md.obsidian", "documents"),
    ("com.openai.chatgpt", "ai"),
    ("com.anthropic.claude", "ai"),
    ("com.google.android.apps.bard", "ai"),
    ("ai.perplexity.app.android", "ai"),
    ("com.termux", "code"),
    ("com.github.android", "code"),
];

/// The kind of app a package is, as Style names it; "other" when unknown.
pub fn category_of(package: &str) -> &'static str {
    APPS.iter()
        .find(|(p, _)| *p == package)
        .map(|(_, c)| *c)
        .unwrap_or("other")
}

/// The tone Style has for a kind of app, as the PC last synced it.
fn tone(conn: &Connection, category: &str) -> Result<Tone> {
    let body: Option<String> = conn
        .query_row(
            "SELECT body FROM phone_extra WHERE kind = 'style' AND key = ?1",
            [category],
            |r| r.get(0),
        )
        .optional()?;
    Ok(body
        .and_then(|b| serde_json::from_str::<serde_json::Value>(&b).ok())
        .and_then(|b| serde_json::from_value(b["tone"].clone()).ok())
        .unwrap_or_else(|| style_rules::default_tone(category)))
}

/// A dictation as the PC would paste it into an app of `category`.
pub fn finish(
    conn: &Connection,
    raw: &str,
    category: &str,
    now: DateTime<Local>,
) -> Result<String> {
    let text = normalize_transcription_output(raw.trim());
    let (_, dictionary) = dictionary_store::compile(&dictionary_store::list(conn)?);
    let text = if dictionary.is_empty() {
        text
    } else {
        dictionary.apply(&text, false, WORD_CORRECTION_THRESHOLD).0
    };
    let language = OutputLanguageEvidence::ModelConstrained(LANGUAGE.into());
    let text = normalize_transcription_output(&remove_filler_words(&text, &language, &None, true));

    // Snippets become placeholders, so cleanup and Style never touch the
    // saved text.
    let snippets = snippet_store::list(conn)?;
    let marked = snippet_rules::mark(&text, &snippet_store::rules_of(&snippets));
    let mut styled = marked.text.clone();
    if !marked.whole {
        let tone = tone(conn, category)?;
        let defaults = CleanupRules::default();
        let lists = style_rules::list_style(category, tone, defaults.lists);
        let rules = style_rules::cleanup_rules_for(category, defaults);
        styled = cleanup_rules::light(&styled, &rules, lists);
        styled = style_rules::apply(&styled, category, tone);
    }
    if marked.hits.is_empty() {
        return Ok(styled);
    }
    let vars = Variables {
        now,
        clipboard: None,
        selection: None,
    };
    Ok(snippet_rules::unmark(&styled, &marked.hits, |id| {
        let text = snippets
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.text.as_str());
        snippet_rules::fill(text.unwrap_or_default(), &vars)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::managers::dictionary_store::{DictionarySource, EntryInput};
    use crate::managers::history_store::MIGRATIONS;
    use crate::managers::snippet_store::SnippetInput;
    use chrono::TimeZone;
    use rusqlite_migration::Migrations;

    fn db() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        Migrations::new(MIGRATIONS.to_vec())
            .to_latest(&mut conn)
            .unwrap();
        conn.execute_batch(
            "CREATE TABLE phone_extra (kind TEXT NOT NULL, key TEXT NOT NULL,
                body TEXT NOT NULL, PRIMARY KEY (kind, key));",
        )
        .unwrap();
        let word = EntryInput {
            text: "Netbird".into(),
            heard: vec!["net bird".into()],
            pinned: false,
            case_sensitive: false,
        };
        dictionary_store::add(&mut conn, &word, DictionarySource::Manual, 1).unwrap();
        let snippet = SnippetInput {
            trigger: "my calendar link".into(),
            text: "cal.com/abhash on {{date}}".into(),
            alone: false,
        };
        snippet_store::add(&mut conn, &snippet, 1).unwrap();
        conn
    }

    fn noon() -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 10, 21, 12, 0, 0).unwrap()
    }

    #[test]
    fn the_dictionary_and_fillers_apply_as_on_the_pc() {
        let text = finish(&db(), "um is net bird up", "other", noon()).unwrap();
        assert_eq!(text, "Is Netbird up");
    }

    #[test]
    fn a_snippet_is_pasted_as_saved_with_its_variables() {
        let text = finish(&db(), "here is my calendar link", "other", noon()).unwrap();
        assert_eq!(text, "Here is cal.com/abhash on 2026-10-21");
    }

    #[test]
    fn the_style_tone_synced_from_the_pc_applies() {
        let conn = db();
        let raw = "Is net bird up";
        assert_eq!(
            finish(&conn, raw, "personal", noon()).unwrap(),
            "Is Netbird up"
        );
        conn.execute(
            "INSERT INTO phone_extra (kind, key, body) VALUES ('style', 'personal', '{\"tone\":\"lower\"}')",
            [],
        )
        .unwrap();
        assert_eq!(
            finish(&conn, raw, "personal", noon()).unwrap(),
            "is netbird up"
        );
    }

    #[test]
    fn apps_sort_into_the_kinds_style_knows() {
        assert_eq!(category_of("com.whatsapp"), "personal");
        assert_eq!(category_of("com.google.android.gm"), "email");
        assert_eq!(category_of("com.android.chrome"), "other");
    }
}
