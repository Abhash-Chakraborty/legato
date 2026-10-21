//! Snippets: say a trigger phrase, paste saved text. Pure functions, so the
//! matching is tested without the app.
//!
//! Matching runs after the Dictionary. Each trigger is compared with word
//! windows of the same length, longest trigger first, and a window close
//! enough (normalised Levenshtein of 0.85 or more) is swapped for a
//! placeholder. Cleanup, the AI steps and Style see the placeholder, never the
//! saved text, and it is put back just before pasting, with its variables
//! filled in.

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use specta::Type;

/// How close a spoken window must be to a trigger.
pub const MATCH_THRESHOLD: f64 = 0.85;
pub const MAX_TEXT_CHARS: usize = 4000;
pub const MAX_TRIGGER_CHARS: usize = 60;

const OPEN: char = '\u{E000}';
const CLOSE: char = '\u{E001}';

/// What the matcher needs of a snippet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnippetRule {
    pub id: i64,
    pub trigger: String,
    /// Only when the trigger is the whole dictation.
    pub alone: bool,
}

/// A dictation with its snippets swapped for placeholders.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Marked {
    pub text: String,
    /// The snippet behind each placeholder, by its number.
    pub hits: Vec<i64>,
    /// The whole dictation was one snippet; it is pasted as saved.
    pub whole: bool,
}

/// Lower case, letters and digits only, single spaces: how triggers compare.
pub fn normalize(text: &str) -> String {
    text.split_whitespace()
        .map(core_word)
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ")
}

/// A word without the punctuation around it ("off." → "off"). Apostrophes
/// inside a word stay ("today's").
fn core_word(word: &str) -> &str {
    word.trim_matches(|c: char| !c.is_alphanumeric())
}

fn similar(a: &str, b: &str) -> bool {
    !a.is_empty() && !b.is_empty() && strsim::normalized_levenshtein(a, b) >= MATCH_THRESHOLD
}

/// A word of the dictation and where it sits, without trailing punctuation.
struct Word {
    start: usize,
    end: usize,
    norm: String,
}

fn words(text: &str) -> Vec<Word> {
    let mut out = Vec::new();
    let mut offset = 0;
    for raw in text.split_inclusive(char::is_whitespace) {
        let token = raw.trim_end();
        let lead = token.len()
            - token
                .trim_start_matches(|c: char| !c.is_alphanumeric())
                .len();
        let core = core_word(token);
        if !core.is_empty() && !token.contains(OPEN) {
            out.push(Word {
                start: offset + lead,
                end: offset + lead + core.len(),
                norm: core.to_lowercase(),
            });
        }
        offset += raw.len();
    }
    out
}

fn placeholder(n: usize) -> String {
    format!("{OPEN}{n}{CLOSE}")
}

/// Swaps spoken triggers for placeholders.
pub fn mark(text: &str, snippets: &[SnippetRule]) -> Marked {
    let mut sorted: Vec<&SnippetRule> = snippets
        .iter()
        .filter(|s| !normalize(&s.trigger).is_empty())
        .collect();
    sorted.sort_by_key(|s| std::cmp::Reverse(normalize(&s.trigger).len()));

    // Said alone: the whole dictation is the trigger.
    let whole = normalize(text);
    if let Some(s) = sorted
        .iter()
        .find(|s| similar(&whole, &normalize(&s.trigger)))
    {
        return Marked {
            text: placeholder(0),
            hits: vec![s.id],
            whole: true,
        };
    }

    let words = words(text);
    let mut claimed = vec![false; words.len()];
    // (start byte, end byte, snippet id)
    let mut found: Vec<(usize, usize, i64)> = Vec::new();
    for s in sorted.iter().filter(|s| !s.alone) {
        let trigger = normalize(&s.trigger);
        let n = trigger.split(' ').count();
        // Speech splits and joins words ("linked in" for "LinkedIn"), so
        // windows one word longer or shorter are tried too.
        let lengths = [n, n + 1, n.saturating_sub(1)];
        let best_at = |i: usize, claimed: &[bool]| {
            lengths
                .iter()
                .filter(|&&len| len > 0 && i + len <= words.len())
                .filter(|&&len| !claimed[i..i + len].iter().any(|c| *c))
                .map(|&len| {
                    let window = words[i..i + len]
                        .iter()
                        .map(|w| w.norm.as_str())
                        .collect::<Vec<_>>()
                        .join(" ");
                    (len, strsim::normalized_levenshtein(&window, &trigger))
                })
                .filter(|(_, score)| *score >= MATCH_THRESHOLD)
                .max_by(|a, b| a.1.total_cmp(&b.1))
        };
        let mut i = 0;
        while i < words.len() {
            match best_at(i, &claimed) {
                // A longer window can match a word early ("my sign off"
                // for "sign off"); the trigger starts at the next word when
                // that matches as well.
                Some((_, score))
                    if best_at(i + 1, &claimed).is_some_and(|(_, next)| next >= score) =>
                {
                    i += 1
                }
                Some((len, _)) => {
                    claimed[i..i + len].iter_mut().for_each(|c| *c = true);
                    found.push((words[i].start, words[i + len - 1].end, s.id));
                    i += len;
                }
                None => i += 1,
            }
        }
    }
    if found.is_empty() {
        return Marked {
            text: text.to_string(),
            ..Default::default()
        };
    }

    found.sort_by_key(|f| f.0);
    let mut out = String::with_capacity(text.len());
    let mut hits = Vec::with_capacity(found.len());
    let mut last = 0;
    for (start, end, id) in found {
        out.push_str(&text[last..start]);
        out.push_str(&placeholder(hits.len()));
        hits.push(id);
        last = end;
    }
    out.push_str(&text[last..]);
    Marked {
        text: out,
        hits,
        whole: false,
    }
}

/// Whether a placeholder ends the text, so Style adds no period after it.
pub fn ends_with_placeholder(text: &str) -> bool {
    text.trim_end().ends_with(CLOSE)
}

pub fn is_placeholder_char(c: char) -> bool {
    c == OPEN || c == CLOSE
}

/// Puts each snippet's text back where its placeholder is.
pub fn unmark(text: &str, hits: &[i64], text_of: impl Fn(i64) -> String) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find(OPEN) {
        out.push_str(&rest[..open]);
        let after = &rest[open + OPEN.len_utf8()..];
        let Some(close) = after.find(CLOSE) else {
            out.push_str(&rest[open..]);
            return out;
        };
        let replacement = after[..close]
            .parse::<usize>()
            .ok()
            .and_then(|n| hits.get(n))
            .map(|id| text_of(*id));
        match replacement {
            Some(r) => out.push_str(&r),
            None => out.push_str(&rest[open..open + OPEN.len_utf8() + close + CLOSE.len_utf8()]),
        }
        rest = &after[close + CLOSE.len_utf8()..];
    }
    out.push_str(rest);
    out
}

/// Placeholders as the model sees them: "[[1]]".
pub fn to_ai(text: &str) -> String {
    text.replace(OPEN, "[[").replace(CLOSE, "]]")
}

/// The model's answer with its placeholders turned back, or None when it
/// dropped, changed or repeated one; then the AI step is skipped.
pub fn from_ai(answer: &str, count: usize) -> Option<String> {
    for n in 0..count {
        if answer.matches(&format!("[[{n}]]")).count() != 1 {
            return None;
        }
    }
    let mut out = answer.to_string();
    for n in 0..count {
        out = out.replace(&format!("[[{n}]]"), &placeholder(n));
    }
    Some(out)
}

/// What a snippet's variables are filled with.
pub struct Variables {
    pub now: DateTime<Local>,
    pub clipboard: Option<String>,
    pub selection: Option<String>,
}

pub const VARIABLES: &[&str] = &[
    "{{date}}",
    "{{date:long}}",
    "{{time}}",
    "{{clipboard}}",
    "{{selection}}",
];

pub fn uses_clipboard(text: &str) -> bool {
    text.contains("{{clipboard}}")
}

pub fn uses_selection(text: &str) -> bool {
    text.contains("{{selection}}")
}

/// Fills {{date}}, {{date:long}}, {{time}}, {{clipboard}} and {{selection}}.
pub fn fill(text: &str, vars: &Variables) -> String {
    if !text.contains("{{") {
        return text.to_string();
    }
    text.replace("{{date:long}}", &vars.now.format("%B %-d, %Y").to_string())
        .replace("{{date}}", &vars.now.format("%Y-%m-%d").to_string())
        .replace("{{time}}", &vars.now.format("%H:%M").to_string())
        .replace("{{clipboard}}", vars.clipboard.as_deref().unwrap_or(""))
        .replace("{{selection}}", vars.selection.as_deref().unwrap_or(""))
}

/// A trigger on the editor's terms: single spaces, no padding.
pub fn clean_trigger(trigger: &str) -> String {
    trigger.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// What the editor's test field shows for a phrase.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SnippetTest {
    /// The dictation with each match swapped for its text.
    pub text: String,
    /// Ids of the snippets that matched, in order.
    pub matched: Vec<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(id: i64, trigger: &str, alone: bool) -> SnippetRule {
        SnippetRule {
            id,
            trigger: trigger.into(),
            alone,
        }
    }

    fn expand(text: &str, rules: &[SnippetRule]) -> String {
        let marked = mark(text, rules);
        unmark(&marked.text, &marked.hits, |id| format!("<{id}>"))
    }

    #[test]
    fn alone_needs_the_whole_dictation() {
        let rules = [rule(3, "sign off", true)];
        let marked = mark("Sign off.", &rules);
        assert!(marked.whole);
        assert_eq!(marked.hits, vec![3]);
        assert_eq!(expand("Sign off.", &rules), "<3>");
        assert_eq!(
            expand("Please sign off on the budget.", &rules),
            "Please sign off on the budget."
        );
    }

    #[test]
    fn anywhere_expands_inside_a_sentence() {
        let rules = [rule(1, "my LinkedIn", false)];
        assert_eq!(
            expand("Here is my LinkedIn, thanks.", &rules),
            "Here is <1>, thanks."
        );
        // Close misses count; distant words don't.
        assert_eq!(expand("here's my linked in", &rules), "here's <1>");
        assert_eq!(expand("here is my laptop", &rules), "here is my laptop");
    }

    #[test]
    fn the_word_before_a_trigger_stays() {
        let rules = [rule(1, "legato test signature", false)];
        assert_eq!(
            expand("Here is my legato test signature.", &rules),
            "Here is my <1>."
        );
        let rules = [rule(2, "linkedin", false)];
        assert_eq!(expand("see my linked in", &rules), "see my <2>");
    }

    #[test]
    fn longest_trigger_wins() {
        let rules = [
            rule(1, "my email", false),
            rule(2, "my college email", false),
        ];
        assert_eq!(
            expand("send it to my college email", &rules),
            "send it to <2>"
        );
        assert_eq!(
            expand("my email and my college email", &rules),
            "<1> and <2>"
        );
    }

    #[test]
    fn placeholders_survive_the_ai_round_trip() {
        let marked = mark("ping me at my email.", &[rule(9, "my email", false)]);
        let ai = to_ai(&marked.text);
        assert_eq!(ai, "ping me at [[0]].");
        let back = from_ai("Ping me at [[0]].", 1).unwrap();
        assert_eq!(
            unmark(&back, &marked.hits, |_| "a@b.c".into()),
            "Ping me at a@b.c."
        );
        assert!(from_ai("Ping me at my email.", 1).is_none());
        assert!(from_ai("[[0]] and [[0]]", 1).is_none());
        assert!(ends_with_placeholder(
            &mark("my email", &[rule(9, "my email", true)]).text
        ));
    }

    #[test]
    fn variables_fill_at_paste_time() {
        use chrono::TimeZone;
        let vars = Variables {
            now: Local.with_ymd_and_hms(2026, 10, 17, 9, 5, 0).unwrap(),
            clipboard: Some("diff".into()),
            selection: None,
        };
        assert_eq!(fill("{{date:long}}", &vars), "October 17, 2026");
        assert_eq!(fill("{{date}} {{time}}", &vars), "2026-10-17 09:05");
        assert_eq!(
            fill("Review:\n\n{{clipboard}}{{selection}}", &vars),
            "Review:\n\ndiff"
        );
    }
}
