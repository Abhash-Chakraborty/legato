//! What the PC does to a dictation after its model, for text written up by
//! the phone's own model: the Dictionary, fillers, snippets, cleanup and the
//! Style tone for the kind of app, all from what syncs from the PC.

use anyhow::Result;
use chrono::{DateTime, Local};
use rusqlite::{Connection, OptionalExtension};

use crate::ai_rules::{self, PromptWord};
use crate::audio_toolkit::text::{
    normalize_transcription_output, remove_filler_words, OutputLanguageEvidence,
};
use crate::cleanup_rules::{self, CleanupRules};
use crate::managers::dictionary_store::{self, DictionaryEntry};
use crate::managers::snippet_store::{self, Snippet};
use crate::snippet_rules::{self, Marked, Variables};
use crate::style_rules::{self, Tone};
use crate::sync::protocol::{AiShare, SharedCleanup, SharedProvider};
use crate::transform_rules;

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
    finish_cleaned(conn, raw, category, now, None)
}

/// What the phone sends its AI provider to clean up a dictation, as the PC's
/// Cleanup at Medium would. The app makes the request with the key it keeps.
#[derive(Clone, Debug, PartialEq)]
pub struct CleanupAsk {
    pub provider: SharedProvider,
    pub model: String,
    pub system: String,
    /// The user message: the transcript, with the Dictionary when the PC
    /// sends it along.
    pub message: String,
}

/// A dictation up to the AI step: the Dictionary, fillers and rule-based
/// cleanup done, snippets standing as markers.
struct Draft {
    marked: Marked,
    snippets: Vec<Snippet>,
    tone: Tone,
}

fn draft(conn: &Connection, raw: &str, category: &str) -> Result<Draft> {
    let text = normalize_transcription_output(raw.trim());
    let (_, dictionary) = dictionary_store::compile(&dictionary_store::list(conn)?);
    let text = if dictionary.is_empty() {
        text
    } else {
        dictionary.apply(&text, false, WORD_CORRECTION_THRESHOLD).0
    };
    let language = OutputLanguageEvidence::ModelConstrained(LANGUAGE.into());
    let text = normalize_transcription_output(&remove_filler_words(&text, &language, &None, true));

    // Snippets become placeholders, so cleanup, the AI and Style never touch
    // the saved text.
    let snippets = snippet_store::list(conn)?;
    let mut marked = snippet_rules::mark(&text, &snippet_store::rules_of(&snippets));
    let tone = tone(conn, category)?;
    if !marked.whole {
        let defaults = CleanupRules::default();
        let lists = style_rules::list_style(category, tone, defaults.lists);
        let rules = style_rules::cleanup_rules_for(category, defaults);
        marked.text = cleanup_rules::light(&marked.text, &rules, lists);
    }
    Ok(Draft {
        marked,
        snippets,
        tone,
    })
}

/// The Dictionary as the cleanup prompt lists it, pinned and most used
/// first. The PC also marks heard forms that are ordinary words by its
/// spell checker; the phone has none, so it leaves that note out.
fn prompt_entries(conn: &Connection, share: &SharedCleanup) -> Result<Vec<DictionaryEntry>> {
    if !share.dictionary {
        return Ok(Vec::new());
    }
    let mut entries = dictionary_store::list(conn)?;
    entries.sort_by(|a, b| b.pinned.cmp(&a.pinned).then(b.hits.cmp(&a.hits)));
    Ok(entries)
}

fn prompt_words(entries: &[DictionaryEntry]) -> Vec<PromptWord<'_>> {
    entries
        .iter()
        .map(|e| PromptWord {
            text: &e.text,
            heard: &e.heard,
            ordinary: Vec::new(),
            kind: e.kind.map(|k| k.noun()),
        })
        .collect()
}

/// The request that cleans up `raw` as the PC would, or None when the PC
/// doesn't run Cleanup at Medium with a provider the phone can reach, or the
/// dictation is too short or a snippet on its own.
pub fn cleanup_ask(
    conn: &Connection,
    raw: &str,
    category: &str,
    share: &AiShare,
) -> Result<Option<CleanupAsk>> {
    let Some(cleanup) = &share.cleanup else {
        return Ok(None);
    };
    let Some(provider) = share.providers.iter().find(|p| p.id == cleanup.provider_id) else {
        return Ok(None);
    };
    let draft = draft(conn, raw, category)?;
    let text = &draft.marked.text;
    if draft.marked.whole
        || text.trim().is_empty()
        || (text.split_whitespace().count() as u32) < cleanup.min_words
    {
        return Ok(None);
    }
    let entries = prompt_entries(conn, cleanup)?;
    let words = prompt_words(&entries);
    Ok(Some(CleanupAsk {
        provider: provider.clone(),
        model: cleanup.model.clone(),
        system: ai_rules::cleanup_system_prompt(draft.marked.hits.len()),
        message: ai_rules::cleanup_request(&words, &snippet_rules::to_ai(text)),
    }))
}

/// The provider's cleaned-up text, when the PC would take it: not the
/// model's reasoning, not a rewrite that lost or added half the words, no
/// Dictionary word brought in from nowhere, every snippet kept.
fn take_cleanup(conn: &Connection, draft: &Draft, answer: &str) -> Result<Option<String>> {
    let input = snippet_rules::to_ai(&draft.marked.text);
    let cleaned = transform_rules::clean_answer(answer, &input);
    if transform_rules::leaks_reasoning(&cleaned, &input)
        || !ai_rules::plausible_cleanup(&input, &cleaned)
    {
        return Ok(None);
    }
    let entries = dictionary_store::list(conn)?;
    if ai_rules::invents_words(&input, &cleaned, &prompt_words(&entries)) {
        return Ok(None);
    }
    Ok(snippet_rules::from_ai(&cleaned, draft.marked.hits.len()))
}

/// [`finish`], with the provider's answer to [`cleanup_ask`] in place of the
/// rule-based text when it passes the PC's checks.
pub fn finish_cleaned(
    conn: &Connection,
    raw: &str,
    category: &str,
    now: DateTime<Local>,
    answer: Option<&str>,
) -> Result<String> {
    let draft = draft(conn, raw, category)?;
    let mut styled = draft.marked.text.clone();
    if !draft.marked.whole {
        if let Some(cleaned) = answer
            .map(|a| take_cleanup(conn, &draft, a))
            .transpose()?
            .flatten()
        {
            styled = cleaned;
        }
        styled = style_rules::apply(&styled, category, draft.tone);
    }
    if draft.marked.hits.is_empty() {
        return Ok(styled);
    }
    let vars = Variables {
        now,
        clipboard: None,
        selection: None,
    };
    Ok(snippet_rules::unmark(&styled, &draft.marked.hits, |id| {
        let text = draft
            .snippets
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
            ..Default::default()
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

    fn share(dictionary: bool) -> AiShare {
        AiShare {
            providers: vec![SharedProvider {
                id: "openai".into(),
                label: "OpenAI".into(),
                base_url: "https://api.openai.com/v1".into(),
                api_format: "openai".into(),
                model: "gpt-5-nano".into(),
                key: None,
            }],
            own_models: Vec::new(),
            cleanup: Some(SharedCleanup {
                provider_id: "openai".into(),
                model: "gpt-5-mini".into(),
                min_words: 3,
                dictionary,
            }),
        }
    }

    #[test]
    fn cleanup_asks_as_the_pc_does_with_snippets_as_markers() {
        let conn = db();
        let ask = cleanup_ask(
            &conn,
            "um so here is my calendar link for net bird",
            "other",
            &share(true),
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            (ask.provider.id.as_str(), ask.model.as_str()),
            ("openai", "gpt-5-mini")
        );
        assert_eq!(ask.system, ai_rules::cleanup_system_prompt(1));
        assert!(
            ask.message.contains("Netbird (misheard as: net bird)"),
            "{}",
            ask.message
        );
        assert!(ask.message.contains("[[0]]"), "{}", ask.message);
        let plain = cleanup_ask(&conn, "is net bird up today", "other", &share(false))
            .unwrap()
            .unwrap();
        assert!(!plain.message.contains("Words:"), "{}", plain.message);
    }

    #[test]
    fn no_cleanup_without_a_provider_or_for_short_dictations() {
        let conn = db();
        assert!(
            cleanup_ask(&conn, "is net bird up today", "other", &AiShare::default())
                .unwrap()
                .is_none()
        );
        assert!(cleanup_ask(&conn, "is it", "other", &share(true))
            .unwrap()
            .is_none());
        assert!(
            cleanup_ask(&conn, "my calendar link", "other", &share(true))
                .unwrap()
                .is_none()
        );
        let mut elsewhere = share(true);
        elsewhere.cleanup.as_mut().unwrap().provider_id = "anthropic".into();
        assert!(
            cleanup_ask(&conn, "is net bird up today", "other", &elsewhere)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn a_cleaned_answer_is_used_when_the_pc_would_take_it() {
        let conn = db();
        let raw = "so um the build is ready and we we ship it after the standup";
        let text = finish_cleaned(
            &conn,
            raw,
            "other",
            noon(),
            Some("So the build is ready, and we ship it after the standup."),
        )
        .unwrap();
        // Style runs after the AI, as on the PC: no full stop for this kind of app.
        assert_eq!(
            text,
            "So the build is ready, and we ship it after the standup"
        );
        // Half the words gone: the rule-based text stays.
        let short = finish_cleaned(&conn, raw, "other", noon(), Some("Build ready.")).unwrap();
        assert_eq!(short, finish(&conn, raw, "other", noon()).unwrap());
        // A dropped snippet marker: the rule-based text stays, snippet filled.
        let raw = "please send my calendar link to the team today";
        let lost = finish_cleaned(
            &conn,
            raw,
            "other",
            noon(),
            Some("Please send it to the team today."),
        )
        .unwrap();
        assert!(lost.contains("cal.com/abhash on 2026-10-21"), "{lost}");
        let kept = finish_cleaned(
            &conn,
            raw,
            "other",
            noon(),
            Some("Please send [[0]] to the team today."),
        )
        .unwrap();
        assert_eq!(
            kept,
            "Please send cal.com/abhash on 2026-10-21 to the team today"
        );
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
