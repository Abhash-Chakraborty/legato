//! Cleanup at Light: rules that tidy a dictation without rewording it.
//! Spoken punctuation, self-corrections, numbers as digits, repeated words,
//! lists and paragraphs. Filler words are removed earlier, in the
//! transcription manager.
//! Pure functions, so every rule is tested without the app.

use serde::{Deserialize, Serialize};
use specta::Type;

/// How much tidying every dictation gets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum CleanupLevel {
    /// Exactly what the model wrote, after the Dictionary.
    None,
    /// The rules below.
    #[default]
    Light,
    /// The rules, then your AI model fixes grammar and false starts.
    Medium,
}

fn yes() -> bool {
    true
}

/// The Light rules, each with its own switch. Fillers have theirs in
/// `filler_word_removal_enabled`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct CleanupRules {
    #[serde(default = "yes")]
    pub repeats: bool,
    #[serde(default = "yes")]
    pub numbers: bool,
    #[serde(default = "yes")]
    pub punctuation: bool,
    #[serde(default = "yes")]
    pub corrections: bool,
    #[serde(default = "yes")]
    pub lists: bool,
    /// A long dictation is split into paragraphs where the thought changes.
    #[serde(default = "yes")]
    pub paragraphs: bool,
}

impl Default for CleanupRules {
    fn default() -> Self {
        Self {
            repeats: true,
            numbers: true,
            punctuation: true,
            corrections: true,
            lists: true,
            paragraphs: true,
        }
    }
}

/// How three or more things named in a row are laid out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListStyle {
    /// Left as a sentence.
    Inline,
    Bullets,
    Numbered,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Tok {
    Word(String),
    /// "\n" or "\n\n".
    Break(&'static str),
}

fn tokenize(text: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            match out.last_mut() {
                Some(Tok::Break(b)) => *b = "\n\n",
                _ => out.push(Tok::Break("\n")),
            }
        }
        out.extend(line.split_whitespace().map(|w| Tok::Word(w.to_string())));
    }
    while matches!(out.first(), Some(Tok::Break(_))) {
        out.remove(0);
    }
    while matches!(out.last(), Some(Tok::Break(_))) {
        out.pop();
    }
    out
}

fn render(toks: &[Tok]) -> String {
    let mut out = String::new();
    for tok in toks {
        match tok {
            Tok::Word(w) => {
                if !out.is_empty() && !out.ends_with('\n') {
                    out.push(' ');
                }
                out.push_str(w);
            }
            Tok::Break(b) => {
                let trimmed = out.trim_end_matches(' ').len();
                out.truncate(trimmed);
                out.push_str(b);
            }
        }
    }
    out
}

/// The word without punctuation around it, lower case.
fn core(word: &str) -> String {
    word.trim_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase()
}

fn word_at(toks: &[Tok], i: usize) -> Option<&str> {
    match toks.get(i) {
        Some(Tok::Word(w)) => Some(w),
        _ => None,
    }
}

fn ends_sentence(word: &str) -> bool {
    word.trim_end_matches(['"', '\'', ')', '”', '’'])
        .ends_with(['.', '?', '!'])
}

fn trailing_punct(word: &str) -> &str {
    let end = word.trim_end_matches(|c: char| !c.is_alphanumeric()).len();
    &word[end..]
}

fn strip_trailing_punct(word: &str) -> String {
    word.trim_end_matches(|c: char| {
        !c.is_alphanumeric() && !crate::snippet_rules::is_placeholder_char(c)
    })
    .to_string()
}

fn capitalize(word: &str) -> String {
    let mut out = String::with_capacity(word.len());
    let mut done = false;
    for c in word.chars() {
        if !done && c.is_alphabetic() {
            out.extend(c.to_uppercase());
            done = true;
        } else {
            out.push(c);
        }
    }
    out
}

/// Runs the switched-on Light rules.
pub fn light(text: &str, rules: &CleanupRules, list_style: ListStyle) -> String {
    let mut toks = tokenize(text);
    if rules.punctuation {
        toks = spoken_punctuation(toks);
        toks = spoken_addresses(toks);
    }
    if rules.corrections {
        toks = scratch_that(toks);
        toks = restarts(toks);
    }
    if rules.numbers {
        toks = numbers(toks);
    }
    if rules.corrections {
        toks = corrections(toks);
    }
    if rules.repeats {
        toks = repeats(toks);
    }
    if rules.lists {
        toks = enumerations(toks);
    }
    if list_style != ListStyle::Inline {
        toks = comma_lists(toks, list_style);
    }
    let text = render(&toks);
    if rules.paragraphs {
        paragraphs(&text)
    } else {
        text
    }
}

// ---- Paragraphs -------------------------------------------------------------

/// Openings that start a new thought; a long dictation breaks before them.
const NEW_THOUGHT: &[&str] = &[
    "also",
    "anyway",
    "anyways",
    "besides",
    "separately",
    "secondly",
    "thirdly",
    "finally",
    "lastly",
    "additionally",
    "meanwhile",
    "on another note",
    "on a different note",
    "on a separate note",
    "one more thing",
    "another thing",
    "moving on",
    "in addition",
    "as for",
    "regarding",
    "by the way",
    "other than that",
    "apart from that",
    "p.s",
    "let me know",
    "looking forward",
    "thanks",
    "thank you",
];

/// Shorter dictations stay one paragraph.
const PARAGRAPH_MIN_WORDS: usize = 40;
/// A paragraph never breaks before it has this many words.
const PARAGRAPH_MIN_RUN: usize = 12;
/// A paragraph this long breaks at the next sentence even without a cue.
const PARAGRAPH_SOFT_MAX: usize = 70;

/// The sentences of a paragraph, each with its closing mark.
fn sentences(par: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let bytes = par.as_bytes();
    for (i, c) in par.char_indices() {
        let after = i + c.len_utf8();
        if matches!(c, '.' | '?' | '!') && bytes.get(after).is_some_and(|b| b.is_ascii_whitespace())
        {
            let s = par[start..after].trim();
            if !s.is_empty() {
                out.push(s);
            }
            start = after;
        }
    }
    let tail = par[start..].trim();
    if !tail.is_empty() {
        out.push(tail);
    }
    out
}

fn starts_new_thought(sentence: &str) -> bool {
    let lower = sentence.to_lowercase();
    let s = lower.trim_start_matches(|c: char| !c.is_alphanumeric());
    NEW_THOUGHT.iter().any(|cue| {
        s.starts_with(cue)
            && !s[cue.len()..]
                .chars()
                .next()
                .is_some_and(char::is_alphanumeric)
    })
}

/// Splits long paragraphs where the thought changes ("Also, …", "Anyway, …",
/// "Let me know …") or, failing a cue, once a paragraph runs long. Lines you
/// broke yourself, lists and short dictations stay as they are.
pub fn paragraphs(text: &str) -> String {
    text.split("\n\n")
        .map(|par| {
            if par.contains('\n') || par.split_whitespace().count() < PARAGRAPH_MIN_WORDS {
                return par.to_string();
            }
            let sents = sentences(par);
            if sents.len() < 3 {
                return par.to_string();
            }
            // Each paragraph, and whether a cue (not length) started it.
            let mut out: Vec<(Vec<&str>, bool)> = vec![(Vec::new(), false)];
            let mut run = 0;
            for s in sents {
                let cue = starts_new_thought(s);
                let current = &out[out.len() - 1].0;
                let long = run >= PARAGRAPH_SOFT_MAX && current.len() >= 2;
                if !current.is_empty() && ((cue && run >= PARAGRAPH_MIN_RUN) || long) {
                    out.push((Vec::new(), cue));
                    run = 0;
                }
                if let Some(last) = out.last_mut() {
                    last.0.push(s);
                }
                run += s.split_whitespace().count();
            }
            // A short tail left by a length break goes back where it was.
            if out.len() > 1 {
                let (tail, cued) = &out[out.len() - 1];
                let words: usize = tail.iter().map(|s| s.split_whitespace().count()).sum();
                if !cued && words < PARAGRAPH_MIN_RUN {
                    if let Some((tail, _)) = out.pop() {
                        if let Some(last) = out.last_mut() {
                            last.0.extend(tail);
                        }
                    }
                }
            }
            out.iter()
                .map(|(p, _)| p.join(" "))
                .collect::<Vec<_>>()
                .join("\n\n")
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

// ---- Spoken punctuation ---------------------------------------------------

const SPOKEN: &[(&[&str], &str)] = &[
    (&["new", "paragraph"], "\n\n"),
    (&["new", "line"], "\n"),
    (&["question", "mark"], "?"),
    (&["exclamation", "mark"], "!"),
    (&["exclamation", "point"], "!"),
    (&["full", "stop"], "."),
    (&["period"], "."),
    (&["comma"], ","),
    (&["semicolon"], ";"),
    (&["colon"], ":"),
];

/// Words after which "period", "comma" or "colon" is the noun, not the mark:
/// "the trial period", "a comma".
const NOUN_BEFORE: &[&str] = &[
    "a",
    "an",
    "the",
    "this",
    "that",
    "these",
    "those",
    "my",
    "your",
    "his",
    "her",
    "its",
    "our",
    "their",
    "each",
    "every",
    "per",
    "one",
    "no",
    "any",
    "some",
    "trial",
    "grace",
    "waiting",
    "billing",
    "notice",
    "cooling",
    "same",
    "test",
    "probation",
    "transition",
    "holding",
    "time",
    "oxford",
];

/// Words after which "period" or "colon" is the noun: "the period of",
/// "colon cancer".
const NOUN_AFTER: &[&str] = &[
    "of",
    "is",
    "was",
    "has",
    "had",
    "will",
    "would",
    "cancer",
    "cleanse",
    "health",
    "screening",
    "surgery",
    "polyps",
    "key",
    "sign",
    "symbol",
    "character",
    "button",
];

/// Words after which "new line" or "new paragraph" is something new, not a
/// break: "a new line of products".
const BREAK_NOUN_BEFORE: &[&str] = &[
    "a", "an", "the", "this", "that", "another", "each", "every", "any", "no", "our", "my", "your",
    "his", "her", "their", "its", "one", "some", "whole", "entire",
];

fn spoken_punctuation(toks: Vec<Tok>) -> Vec<Tok> {
    let mut out: Vec<Tok> = Vec::with_capacity(toks.len());
    let mut capitalize_next = false;
    let mut i = 0;
    while i < toks.len() {
        let matched = SPOKEN.iter().find(|(words, _)| {
            words
                .iter()
                .enumerate()
                .all(|(k, w)| word_at(&toks, i + k).is_some_and(|t| core(t) == *w))
        });
        let prev = out.iter().rev().find_map(|t| match t {
            Tok::Word(w) => Some(w.clone()),
            Tok::Break(_) => None,
        });
        let at_line_start = !matches!(out.last(), Some(Tok::Word(_)));
        if let Some((words, mark)) = matched {
            let is_break = mark.starts_with('\n');
            let next = word_at(&toks, i + words.len()).map(core);
            let noun = if is_break {
                prev.as_deref()
                    .is_some_and(|p| BREAK_NOUN_BEFORE.contains(&core(p).as_str()))
                    || next.as_deref() == Some("of")
            } else if words.len() == 1 && *mark == "." {
                // "period accurate": a full stop starts a new sentence, which
                // the model writes with a capital.
                prev.as_deref()
                    .is_some_and(|p| NOUN_BEFORE.contains(&core(p).as_str()))
                    || word_at(&toks, i + 1)
                        .is_some_and(|n| n.starts_with(|c: char| c.is_lowercase()))
            } else {
                words.len() == 1
                    && (prev
                        .as_deref()
                        .is_some_and(|p| NOUN_BEFORE.contains(&core(p).as_str()))
                        || next.as_deref().is_some_and(|n| NOUN_AFTER.contains(&n)))
            };
            if !noun && (is_break || !at_line_start) {
                if is_break {
                    if let Some(Tok::Word(w)) = out.last_mut() {
                        // "dinner, new line" → the comma the model added goes.
                        if w.ends_with(',') {
                            w.pop();
                        }
                    }
                    if !out.is_empty() {
                        out.push(Tok::Break(mark));
                    }
                } else if let Some(Tok::Word(w)) = out.last_mut() {
                    *w = strip_trailing_punct(w) + mark;
                }
                capitalize_next = is_break || matches!(*mark, "." | "?" | "!");
                i += words.len();
                continue;
            }
        }
        match &toks[i] {
            Tok::Word(w) => {
                let w = if capitalize_next {
                    capitalize(w)
                } else {
                    w.clone()
                };
                capitalize_next = false;
                out.push(Tok::Word(w));
            }
            Tok::Break(b) => out.push(Tok::Break(b)),
        }
        i += 1;
    }
    out
}

// ---- Spoken addresses ---------------------------------------------------

/// Endings that make "<word> dot <ending>" an address or a file name.
const DOT_ENDINGS: &[&str] = &[
    "com", "org", "net", "io", "dev", "ai", "app", "co", "in", "de", "uk", "eu", "me", "info",
    "edu", "gov", "us", "ca", "au", "xyz", "tech", "site", "id", "md", "txt", "pdf", "rs", "ts",
    "tsx", "js", "jsx", "json", "py", "html", "css", "toml", "yaml", "yml", "csv", "png", "jpg",
    "exe", "zip", "sh", "docx", "xlsx",
];

/// Whether a word is an address this pass made: letters, digits and dots.
fn is_address(word: &str) -> bool {
    word.contains('.')
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '/' | '_'))
}

/// "github dot com slash abhash" → github.com/abhash, "abhash at gmail dot
/// com" → abhash@gmail.com, "readme dot md" → readme.md, "C plus plus" → C++.
fn spoken_addresses(mut toks: Vec<Tok>) -> Vec<Tok> {
    let plain = |w: &str| {
        !w.is_empty()
            && w.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    };
    let mut i = 1;
    while i + 1 < toks.len() {
        let (Some(l), Some(m), Some(r)) = (
            word_at(&toks, i - 1),
            word_at(&toks, i),
            word_at(&toks, i + 1),
        ) else {
            i += 1;
            continue;
        };
        let (lc, mc, rc) = (core(l), core(m), core(r));
        let joined = trailing_punct(l).is_empty() && trailing_punct(m).is_empty();
        let lead = &l[..l.len() - l.trim_start_matches(|c: char| !c.is_alphanumeric()).len()];
        let merged = match mc.as_str() {
            // "the dot com bubble" is a word, not an address.
            "dot"
                if joined
                    && !BREAK_NOUN_BEFORE.contains(&lc.as_str())
                    && (plain(&lc) || is_address(&lc))
                    && DOT_ENDINGS.contains(&rc.as_str()) =>
            {
                Some(format!("{lead}{lc}.{rc}{}", trailing_punct(r)))
            }
            "slash" if joined && is_address(&lc) && plain(&rc) => {
                Some(format!("{lead}{lc}/{rc}{}", trailing_punct(r)))
            }
            "plus" if lc == "c" && rc == "plus" => Some(format!("{lead}C++{}", trailing_punct(r))),
            _ => None,
        };
        if let Some(word) = merged {
            toks.splice(i - 1..i + 2, [Tok::Word(word)]);
            // The address may go on: "www dot google dot com".
            i = i.saturating_sub(1).max(1);
            continue;
        }
        i += 1;
    }
    // "abhash at gmail.com" → abhash@gmail.com, once the domain is whole.
    let mut i = 1;
    while i + 1 < toks.len() {
        let hit = word_at(&toks, i).is_some_and(|w| w == "at")
            && word_at(&toks, i - 1).is_some_and(plain)
            && word_at(&toks, i + 1).is_some_and(|w| {
                let c = strip_trailing_punct(w);
                is_address(&c) && !c.contains('/')
            });
        if hit {
            let name = word_at(&toks, i - 1).unwrap_or_default().to_lowercase();
            let domain = word_at(&toks, i + 1).unwrap_or_default().to_string();
            toks.splice(i - 1..i + 2, [Tok::Word(format!("{name}@{domain}"))]);
        }
        i += 1;
    }
    // "C sharp" → C#
    let mut i = 0;
    while i + 1 < toks.len() {
        let hit = word_at(&toks, i).is_some_and(|w| w == "C")
            && word_at(&toks, i + 1).is_some_and(|w| core(w) == "sharp");
        if hit {
            let tail = trailing_punct(word_at(&toks, i + 1).unwrap_or_default()).to_string();
            toks.splice(i..i + 2, [Tok::Word(format!("C#{tail}"))]);
        }
        i += 1;
    }
    toks
}

// ---- Self-corrections -----------------------------------------------------

/// Index of the first word of the sentence that holds `i`.
fn sentence_start(toks: &[Tok], i: usize) -> usize {
    let mut j = i;
    while j > 0 {
        match &toks[j - 1] {
            Tok::Break(_) => break,
            Tok::Word(w) if ends_sentence(w) => break,
            _ => j -= 1,
        }
    }
    j
}

/// "scratch that" drops what came before it in the sentence, or the whole
/// previous sentence when it opens one.
fn scratch_that(mut toks: Vec<Tok>) -> Vec<Tok> {
    let mut i = 0;
    while i + 1 < toks.len() {
        let hit = word_at(&toks, i).is_some_and(|w| core(w) == "scratch")
            && word_at(&toks, i + 1).is_some_and(|w| core(w) == "that");
        if !hit {
            i += 1;
            continue;
        }
        let mut start = sentence_start(&toks, i);
        if start == i && start > 0 && matches!(toks[start - 1], Tok::Word(_)) {
            start = sentence_start(&toks, start - 1);
        }
        toks.drain(start..i + 2);
        if let Some(Tok::Word(w)) = toks.get_mut(start) {
            *w = capitalize(w.trim_start_matches(|c: char| !c.is_alphanumeric()));
            if w.is_empty() {
                toks.remove(start);
            }
        }
        i = start;
    }
    toks
}

/// Words that take back what was just said, longest first.
const RESTART_MARKERS: &[&[&str]] = &[
    &["no", "wait"],
    &["i", "mean"],
    &["or", "rather"],
    &["no"],
    &["actually"],
    &["sorry"],
    &["wait"],
];

/// Words that show a sentence describes or quotes speech, where a restart
/// is meant to stay: "like, if I say at 12, no, at 4, it should…".
const QUOTING: &[&str] = &[
    "like", "say", "says", "said", "saying", "example", "quote", "if", "when",
];

/// "A meeting at 12, no, I want it at 4": when the words after a marker
/// say again at least two words from earlier in the sentence, the speaker
/// started over, so the first try and the marker go.
fn restarts(mut toks: Vec<Tok>) -> Vec<Tok> {
    let mut i = 1;
    while i < toks.len() {
        let Some(marker) = RESTART_MARKERS.iter().find(|m| {
            m.iter()
                .enumerate()
                .all(|(k, w)| word_at(&toks, i + k).is_some_and(|t| core(t) == *w))
        }) else {
            i += 1;
            continue;
        };
        let start = sentence_start(&toks, i);
        let before: Vec<String> = (start..i)
            .filter_map(|j| word_at(&toks, j))
            .map(core)
            .collect();
        if before.is_empty() || before.iter().any(|w| QUOTING.contains(&w.as_str())) {
            i += 1;
            continue;
        }
        let after = i + marker.len();
        let mut end = after;
        while let Some(Tok::Word(w)) = toks.get(end) {
            end += 1;
            if ends_sentence(w) {
                break;
            }
        }
        let again: Vec<String> = (after..end)
            .filter_map(|j| word_at(&toks, j))
            .map(core)
            .collect();
        let found = (0..before.len()).find(|&p| {
            before[p..]
                .iter()
                .zip(&again)
                .take_while(|(a, b)| a == b)
                .count()
                >= 2
        });
        let Some(p) = found else {
            i += 1;
            continue;
        };
        let from = start + p;
        let opened = word_at(&toks, from)
            .and_then(|w| w.chars().find(|c| c.is_alphabetic()))
            .is_some_and(char::is_uppercase);
        toks.drain(from..after);
        if opened {
            if let Some(Tok::Word(w)) = toks.get_mut(from) {
                *w = capitalize(w);
            }
        }
        i = from + 1;
    }
    toks
}

fn is_numeric(word: &str) -> bool {
    let c = core(word);
    !c.is_empty()
        && (c.chars().next().is_some_and(|ch| ch.is_ascii_digit()) || number_word(&c).is_some())
}

/// "at 3, actually 4" keeps 4; "Karol, I mean Kasia" keeps Kasia.
fn corrections(toks: Vec<Tok>) -> Vec<Tok> {
    let mut out: Vec<Tok> = Vec::with_capacity(toks.len());
    let mut i = 0;
    while i < toks.len() {
        if let Some(prev) = out.last().and_then(|t| match t {
            Tok::Word(w) => Some(w.clone()),
            _ => None,
        }) {
            let marker = [
                (&["actually"][..], true),
                (&["sorry"][..], true),
                (&["no", "wait"][..], true),
                (&["no"][..], true),
                (&["i", "mean"][..], false),
            ]
            .into_iter()
            .find(|(words, _)| {
                words
                    .iter()
                    .enumerate()
                    .all(|(k, w)| word_at(&toks, i + k).is_some_and(|t| core(t) == *w))
            });
            if let Some((words, numbers_only)) = marker {
                let next = word_at(&toks, i + words.len());
                let fits = match next {
                    Some(next) if numbers_only => is_numeric(&prev) && is_numeric(next),
                    // "I mean" needs the comma before it; without one it is
                    // usually meant ("I mean it").
                    Some(_) => prev.ends_with(','),
                    None => false,
                };
                if fits {
                    let mut next = next.unwrap_or_default().to_string();
                    // "Karol, I mean Kasia, tomorrow": the second comma
                    // closed the aside.
                    if prev.ends_with(',') && next.ends_with(',') {
                        next.pop();
                    }
                    out.pop();
                    out.push(Tok::Word(next));
                    i += words.len() + 1;
                    continue;
                }
            }
        }
        out.push(toks[i].clone());
        i += 1;
    }
    out
}

// ---- Numbers ----------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum NumKind {
    Unit,
    Tens,
    Hundred,
    Scale,
}

const UNITS: [&str; 20] = [
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
];

const TENS: [&str; 8] = [
    "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];

/// The ordinals of the units, for "twenty first".
const UNIT_ORDINALS: [&str; 9] = [
    "first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth",
];

fn number_word(word: &str) -> Option<(u64, NumKind)> {
    if let Some(v) = UNITS.iter().position(|u| *u == word) {
        return Some((v as u64, NumKind::Unit));
    }
    if let Some(v) = TENS.iter().position(|t| *t == word) {
        return Some((20 + 10 * v as u64, NumKind::Tens));
    }
    match word {
        "hundred" => Some((100, NumKind::Hundred)),
        "thousand" => Some((1_000, NumKind::Scale)),
        "million" => Some((1_000_000, NumKind::Scale)),
        "billion" => Some((1_000_000_000, NumKind::Scale)),
        "trillion" => Some((1_000_000_000_000, NumKind::Scale)),
        _ => None,
    }
}

/// A digit said as a word: "zero" to "nine".
fn digit_word(word: &str) -> Option<char> {
    UNITS[..10]
        .iter()
        .position(|u| *u == word)
        .map(|d| char::from(b'0' + d as u8))
}

fn unit_ordinal(word: &str) -> Option<u64> {
    UNIT_ORDINALS
        .iter()
        .position(|o| *o == word)
        .map(|p| p as u64 + 1)
}

fn ordinal_suffix(n: u64) -> &'static str {
    match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    }
}

/// One spoken number: its value and how many words said it.
struct Group {
    value: u64,
    words: usize,
    /// The last word was a tens word or a teen, so it can pair as a year.
    two_digit: bool,
    last: Option<NumKind>,
}

impl Group {
    /// "hundred" or "million" said on its own: "a couple of hundred".
    fn bare(&self) -> bool {
        self.words == 1 && matches!(self.last, Some(NumKind::Hundred | NumKind::Scale))
    }
}

#[derive(Default)]
struct Builder {
    total: u64,
    current: u64,
    words: usize,
    last: Option<NumKind>,
    last_value: u64,
}

impl Builder {
    /// Whether this word continues the number being said.
    fn fits(&self, value: u64, kind: NumKind) -> bool {
        match (self.last, kind) {
            (None, _) => true,
            (Some(NumKind::Unit), NumKind::Unit | NumKind::Tens) => false,
            (Some(NumKind::Tens), NumKind::Tens) => false,
            (Some(NumKind::Tens), NumKind::Unit) => value < 10 && value > 0,
            (Some(NumKind::Hundred), NumKind::Hundred) => false,
            _ => true,
        }
    }

    fn push(&mut self, value: u64, kind: NumKind) {
        match kind {
            NumKind::Unit | NumKind::Tens => self.current += value,
            NumKind::Hundred => self.current = self.current.max(1) * 100,
            NumKind::Scale => {
                self.total += self.current.max(1) * value;
                self.current = 0;
            }
        }
        self.words += 1;
        self.last = Some(kind);
        self.last_value = value;
    }

    fn finish(self) -> Group {
        Group {
            value: self.total + self.current,
            words: self.words,
            two_digit: self.words <= 2 && matches!(self.last, Some(NumKind::Tens))
                || (self.last == Some(NumKind::Unit) && self.last_value >= 10 && self.words == 1)
                || (self.words == 2 && self.total == 0 && (21..100).contains(&self.current)),
            last: self.last,
        }
    }
}

fn with_commas(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (k, c) in digits.chars().enumerate() {
        if k > 0 && (digits.len() - k).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// How a value is written: "2.5 million" for round millions and up, commas
/// from 10,000, and plain digits below (so years stay "2026").
fn format_value(value: u64) -> String {
    for (scale, name) in [
        (1_000_000_000_000, "trillion"),
        (1_000_000_000, "billion"),
        (1_000_000, "million"),
    ] {
        if value >= scale && value.is_multiple_of(scale / 10) {
            let (whole, tenth) = (value / scale, value % scale / (scale / 10));
            return if tenth == 0 {
                format!("{whole} {name}")
            } else {
                format!("{whole}.{tenth} {name}")
            };
        }
    }
    if value >= 10_000 {
        with_commas(value)
    } else {
        value.to_string()
    }
}

fn format_groups(groups: &[Group], percent: bool) -> Option<String> {
    if let [a, b] = groups {
        // "twelve thirty" is a time; "twenty twenty six" and "nineteen
        // ninety" are years.
        if a.words == 1 && (1..=12).contains(&a.value) && b.two_digit && (10..60).contains(&b.value)
        {
            return Some(format!("{}:{:02}", a.value, b.value));
        }
        if a.two_digit && (13..100).contains(&a.value) && b.two_digit && b.value < 100 {
            return Some(format!("{}{:02}", a.value, b.value));
        }
    }
    if groups.iter().any(Group::bare) {
        return None;
    }
    let one = groups.len() == 1;
    let parts: Vec<String> = groups
        .iter()
        .map(|g| {
            if g.words >= 2 || g.value >= 10 || (percent && one) {
                format_value(g.value)
            } else {
                String::new()
            }
        })
        .collect();
    if parts.iter().any(|p| p.is_empty()) {
        return None;
    }
    Some(parts.join(" "))
}

/// Three or more digits said one by one, as in a code or a phone number:
/// "one two three four" → 1234, "four oh four" → 404. Returns the digits
/// with the last word's punctuation, and where the run ends.
fn digit_run(toks: &[Tok], i: usize) -> Option<(String, usize)> {
    let mut digits = String::new();
    let mut j = i;
    let mut tail = "";
    while let Some(w) = word_at(toks, j) {
        let c = core(w);
        let d = match digit_word(&c) {
            Some(d) => d,
            // "oh" between two digits is a zero.
            None if c == "oh"
                && !digits.is_empty()
                && word_at(toks, j + 1).is_some_and(|n| digit_word(&core(n)).is_some()) =>
            {
                '0'
            }
            None => break,
        };
        // An opening mark ("(") starts something else.
        if j > i && !w.starts_with(|c: char| c.is_alphanumeric()) {
            break;
        }
        digits.push(d);
        tail = trailing_punct(w);
        j += 1;
        if !tail.is_empty() {
            break;
        }
    }
    (digits.len() >= 3).then(|| (format!("{digits}{tail}"), j))
}

/// "twenty-first" → 21st.
fn hyphen_ordinal(word: &str) -> Option<String> {
    let (a, b) = word.split_once('-')?;
    let (tens, NumKind::Tens) = number_word(a)? else {
        return None;
    };
    let n = tens + unit_ordinal(b)?;
    Some(format!("{n}{}", ordinal_suffix(n)))
}

/// Numbers as a person would type them: "twenty five percent" → 25%,
/// "two hundred" → 200, "twenty first" → 21st, "two point five million" →
/// 2.5 million, "twelve thirty" → 12:30. Single numbers under ten stay words
/// ("one of them"), and so do "a million" and a lone "hundred".
fn numbers(toks: Vec<Tok>) -> Vec<Tok> {
    let mut out = Vec::with_capacity(toks.len());
    let mut i = 0;
    while i < toks.len() {
        let Some(first) = word_at(&toks, i) else {
            out.push(toks[i].clone());
            i += 1;
            continue;
        };
        let lead = &first[..first.len()
            - first
                .trim_start_matches(|c: char| !c.is_alphanumeric())
                .len()];
        let first_core = core(first);
        if let Some((digits, next)) = digit_run(&toks, i) {
            out.push(Tok::Word(format!("{lead}{digits}")));
            i = next;
            continue;
        }
        if let Some(ordinal) = hyphen_ordinal(&first_core) {
            out.push(Tok::Word(format!(
                "{lead}{ordinal}{}",
                trailing_punct(first)
            )));
            i += 1;
            continue;
        }
        let starts_with_a = first_core == "a"
            && word_at(&toks, i + 1)
                .and_then(|w| number_word(&core(w)))
                .is_some_and(|(_, k)| matches!(k, NumKind::Hundred | NumKind::Scale));
        if number_word_parts(&first_core).is_none() && !starts_with_a {
            out.push(toks[i].clone());
            i += 1;
            continue;
        }

        // Read the run of number words, split into numbers.
        let mut groups: Vec<Group> = Vec::new();
        let mut b = Builder::default();
        let mut j = i;
        let mut tail = "";
        while let Some(w) = word_at(&toks, j) {
            let c = core(w);
            let parts = if j == i && starts_with_a {
                Some(vec![(1, NumKind::Unit)])
            } else if c == "and" && matches!(b.last, Some(NumKind::Hundred | NumKind::Scale)) {
                // "two hundred and five", "two thousand and eight"
                let next_is_number = word_at(&toks, j + 1)
                    .and_then(|n| number_word_parts(&core(n)))
                    .is_some();
                if next_is_number && tail.is_empty() {
                    j += 1;
                    continue;
                }
                None
            } else {
                number_word_parts(&c)
            };
            let Some(parts) = parts else { break };
            if !tail.is_empty() {
                break;
            }
            for (value, kind) in parts {
                if !b.fits(value, kind) {
                    groups.push(std::mem::take(&mut b).finish());
                }
                b.push(value, kind);
            }
            tail = trailing_punct(w);
            j += 1;
            if !tail.is_empty() {
                break;
            }
        }
        if b.words > 0 {
            groups.push(b.finish());
        }

        let mut text: Option<String> = None;
        if let ([g], true) = (&groups[..], tail.is_empty()) {
            let next = word_at(&toks, j);
            let ordinal = next.and_then(|w| unit_ordinal(&core(w)));
            if let (Some(NumKind::Tens), Some(n), Some(w)) = (g.last, ordinal, next) {
                // "twenty first" → 21st
                let v = g.value + n;
                text = Some(format!("{v}{}", ordinal_suffix(v)));
                tail = trailing_punct(w);
                j += 1;
            } else if !g.bare() && next.is_some_and(|w| core(w) == "point") {
                // "two point five" → 2.5, "two point five million" → 2.5 million
                let mut digits = String::new();
                let mut k = j + 1;
                let mut point_tail = "";
                while let Some(w) = word_at(&toks, k) {
                    let Some(d) = digit_word(&core(w)) else { break };
                    digits.push(d);
                    point_tail = trailing_punct(w);
                    k += 1;
                    if !point_tail.is_empty() {
                        break;
                    }
                }
                if !digits.is_empty() {
                    let mut decimal = format!("{}.{digits}", g.value);
                    tail = point_tail;
                    j = k;
                    if let (Some(w), true) = (word_at(&toks, j), tail.is_empty()) {
                        let c = core(w);
                        if matches!(c.as_str(), "million" | "billion" | "trillion") {
                            decimal = format!("{decimal} {c}");
                            tail = trailing_punct(w);
                            j += 1;
                        }
                    }
                    text = Some(decimal);
                }
            }
        }
        // "the eighteen hundreds" and "a million" read better as words.
        let plural_scale = tail.is_empty()
            && word_at(&toks, j).is_some_and(|w| {
                matches!(
                    core(w).as_str(),
                    "hundreds" | "thousands" | "millions" | "billions"
                )
            });
        let a_scale = starts_with_a
            && matches!(&groups[..], [g] if g.words == 2 && g.last == Some(NumKind::Scale));
        let percent = tail.is_empty()
            && word_at(&toks, j)
                .is_some_and(|w| matches!(core(w).as_str(), "percent" | "per cent"));
        let formatted = if text.is_some() {
            text
        } else if plural_scale || a_scale {
            None
        } else {
            format_groups(&groups, percent)
        };
        match formatted {
            Some(digits) => {
                if percent {
                    let p = word_at(&toks, j).unwrap_or_default();
                    out.push(Tok::Word(format!("{lead}{digits}%{}", trailing_punct(p))));
                    j += 1;
                } else {
                    out.push(Tok::Word(format!("{lead}{digits}{tail}")));
                }
                i = j;
            }
            None => {
                out.extend(toks[i..j.max(i + 1)].iter().cloned());
                i = j.max(i + 1);
            }
        }
    }
    pair_numbers(out)
}

/// A number word, or a hyphenated pair ("twenty-five").
fn number_word_parts(word: &str) -> Option<Vec<(u64, NumKind)>> {
    if let Some(n) = number_word(word) {
        return Some(vec![n]);
    }
    let (a, b) = word.split_once('-')?;
    let (a, b) = (number_word(a)?, number_word(b)?);
    (a.1 == NumKind::Tens && b.1 == NumKind::Unit && b.0 > 0 && b.0 < 10).then(|| vec![a, b])
}

/// Both sides of "16 by 9", "10 out of 10" or "5 to 10" in the same form:
/// a small number left as a word next to one in digits becomes a digit.
fn pair_numbers(mut toks: Vec<Tok>) -> Vec<Tok> {
    let is_digits = |w: &str| core(w).starts_with(|c: char| c.is_ascii_digit());
    let small = |w: &str| digit_word(&core(w));
    let mut i = 0;
    while i < toks.len() {
        let width = match (word_at(&toks, i).map(core), word_at(&toks, i + 1).map(core)) {
            (Some(a), Some(b)) if a == "out" && b == "of" => 2,
            (Some(a), _) if matches!(a.as_str(), "by" | "x" | "to" | "or") => 1,
            _ => {
                i += 1;
                continue;
            }
        };
        let (Some(l), Some(r)) = (
            i.checked_sub(1).and_then(|p| word_at(&toks, p)),
            word_at(&toks, i + width),
        ) else {
            i += 1;
            continue;
        };
        // Only when nothing but the connector sits between them.
        if !trailing_punct(l).is_empty() {
            i += 1;
            continue;
        }
        let digit_word_in = |w: &str, d: char| {
            let lead_len = w.len() - w.trim_start_matches(|c: char| !c.is_alphanumeric()).len();
            format!("{}{d}{}", &w[..lead_len], trailing_punct(w))
        };
        if is_digits(l) {
            if let Some(d) = small(r) {
                let replaced = digit_word_in(r, d);
                toks[i + width] = Tok::Word(replaced);
            }
        } else if is_digits(r) {
            if let Some(d) = small(l) {
                let replaced = digit_word_in(l, d);
                toks[i - 1] = Tok::Word(replaced);
            }
        }
        i += width;
    }
    toks
}

// ---- Repeated words -------------------------------------------------------

/// Words people double on purpose: "bye bye", "very very", "C plus plus".
pub(crate) const MEANT_TWICE: &[&str] = &[
    "had", "that", "very", "really", "so", "bye", "okay", "ok", "yeah", "yes", "no", "plus", "ha",
    "haha", "hey", "oh", "wow", "please", "more", "far", "much", "many", "knock", "blah", "la",
    "go", "now", "again", "little", "tick", "ding", "night", "well", "hip", "chop", "win",
];

/// "the the" → "the". Words in [`MEANT_TWICE`] stay doubled.
fn repeats(toks: Vec<Tok>) -> Vec<Tok> {
    let mut out: Vec<Tok> = Vec::with_capacity(toks.len());
    for tok in toks {
        if let (Tok::Word(w), Some(Tok::Word(prev))) = (&tok, out.last()) {
            let same = core(w) == core(prev) && !core(w).is_empty();
            let joined = trailing_punct(prev).is_empty();
            let meant = MEANT_TWICE.contains(&core(w).as_str()) || is_numeric(w);
            if same && joined && !meant {
                let keep_punct = trailing_punct(w).to_string();
                if let Some(Tok::Word(prev)) = out.last_mut() {
                    prev.push_str(&keep_punct);
                }
                continue;
            }
        }
        out.push(tok);
    }
    out
}

// ---- Lists ------------------------------------------------------------------

const ORDINALS: &[&str] = &[
    "first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth", "tenth",
];

fn ordinal(word: &str) -> Option<usize> {
    let c = core(word);
    let c = c.strip_suffix("ly").unwrap_or(&c);
    ORDINALS.iter().position(|o| *o == c)
}

fn item_words(toks: &[Tok]) -> Vec<String> {
    let mut words: Vec<String> = toks
        .iter()
        .filter_map(|t| match t {
            Tok::Word(w) => Some(w.clone()),
            _ => None,
        })
        .collect();
    while words
        .first()
        .is_some_and(|w| matches!(core(w).as_str(), "and" | "then" | ""))
    {
        words.remove(0);
    }
    while words
        .last()
        .is_some_and(|w| matches!(core(w).as_str(), "and" | "then"))
    {
        words.pop();
    }
    if let Some(last) = words.last_mut() {
        *last = strip_trailing_punct(last);
    }
    if let Some(first) = words.first_mut() {
        *first = capitalize(first);
    }
    words
}

/// "first …, second …, third …" becomes a numbered list under its lead-in.
fn enumerations(toks: Vec<Tok>) -> Vec<Tok> {
    let mut marks: Vec<usize> = Vec::new();
    for (i, tok) in toks.iter().enumerate() {
        let Tok::Word(w) = tok else { continue };
        if ordinal(w) != Some(marks.len()) {
            continue;
        }
        // "First of all", "first off" and "first and foremost" open a
        // point, not a list.
        let after = |k: usize| word_at(&toks, i + k).map(core);
        if marks.is_empty()
            && trailing_punct(w).is_empty()
            && matches!(
                (after(1).as_deref(), after(2).as_deref()),
                (Some("of"), Some("all")) | (Some("off"), _) | (Some("and"), Some("foremost"))
            )
        {
            return toks;
        }
        // "first" can sit anywhere ("three things first the keys"); the
        // ones after it open a clause.
        let clause_start = marks.is_empty()
            || match i.checked_sub(1).map(|p| &toks[p]) {
                None | Some(Tok::Break(_)) => true,
                Some(Tok::Word(p)) => {
                    p.ends_with([',', '.', ':', ';', '?', '!'])
                        || matches!(core(p).as_str(), "and" | "then")
                }
            };
        if clause_start {
            marks.push(i);
        }
    }
    if marks.len() < 2 {
        return toks;
    }

    // The last item ends with its sentence; the rest follows the list.
    let last = *marks.last().unwrap_or(&0);
    let mut end = toks.len();
    for (k, tok) in toks.iter().enumerate().skip(last) {
        match tok {
            Tok::Break(_) => {
                end = k;
                break;
            }
            Tok::Word(w) if ends_sentence(w) => {
                end = k + 1;
                break;
            }
            _ => {}
        }
    }

    let mut out: Vec<Tok> = Vec::new();
    let mut lead: Vec<Tok> = toks[..marks[0]].to_vec();
    while matches!(lead.last(), Some(Tok::Word(w)) if matches!(core(w).as_str(), "and" | "then")) {
        lead.pop();
    }
    if let Some(Tok::Word(w)) = lead.last_mut() {
        *w = strip_trailing_punct(w) + ":";
    }
    let has_lead = !lead.is_empty();
    out.extend(lead);
    for (n, &m) in marks.iter().enumerate() {
        let stop = marks.get(n + 1).copied().unwrap_or(end);
        let words = item_words(&toks[m + 1..stop]);
        if words.is_empty() {
            continue;
        }
        if has_lead || n > 0 {
            out.push(Tok::Break("\n"));
        }
        out.push(Tok::Word(format!("{}.", n + 1)));
        out.extend(words.into_iter().map(Tok::Word));
    }
    if end < toks.len() {
        out.push(Tok::Break("\n\n"));
        out.extend(
            toks[end..]
                .iter()
                .skip_while(|t| matches!(t, Tok::Break(_)))
                .cloned(),
        );
    }
    out
}

/// Splits a sentence into its lead-in and three or more things named in a
/// row: "I need milk, eggs, bread and butter".
fn named_items(sentence: &[String]) -> Option<(Vec<String>, Vec<Vec<String>>)> {
    let mut parts: Vec<Vec<String>> = vec![Vec::new()];
    for w in sentence {
        let ends_part = w.ends_with(',');
        parts.last_mut()?.push(w.trim_end_matches(',').to_string());
        if ends_part {
            parts.push(Vec::new());
        }
    }
    parts.retain(|p| !p.is_empty());
    if parts.len() < 2 {
        return None;
    }
    // The last part: "bread and butter", or "and butter" after a comma.
    let last = parts.pop()?;
    let and = last
        .iter()
        .rposition(|w| matches!(core(w).as_str(), "and" | "or"))?;
    if and == 0 {
        parts.push(last[1..].to_vec());
    } else {
        parts.push(last[..and].to_vec());
        parts.push(last[and + 1..].to_vec());
    }
    if parts.len() < 3 || parts.iter().any(|p| p.is_empty()) {
        return None;
    }
    let size = parts[1..].iter().map(Vec::len).max()?;
    if size > 4 {
        return None;
    }
    let first = parts[0].clone();
    if first.len() <= size {
        return None;
    }
    let lead = first[..first.len() - size].to_vec();
    let mut items = vec![first[first.len() - size..].to_vec()];
    items.extend(parts[1..].iter().cloned());
    Some((lead, items))
}

/// In documents (and the Notes and Structured tones), three or more things
/// named in a row become a list under their lead-in.
fn comma_lists(toks: Vec<Tok>, style: ListStyle) -> Vec<Tok> {
    let mut out: Vec<Tok> = Vec::with_capacity(toks.len());
    let mut sentence: Vec<String> = Vec::new();
    let flush = |sentence: &mut Vec<String>, out: &mut Vec<Tok>| {
        if sentence.is_empty() {
            return;
        }
        let is_list_line = sentence
            .first()
            .is_some_and(|w| w == "-" || w.trim_end_matches('.').parse::<u32>().is_ok());
        match (!is_list_line).then(|| named_items(sentence)).flatten() {
            Some((lead, items)) => {
                if matches!(out.last(), Some(Tok::Word(_))) {
                    out.push(Tok::Break("\n\n"));
                }
                let n = lead.len();
                for (k, w) in lead.into_iter().enumerate() {
                    out.push(Tok::Word(if k + 1 == n {
                        strip_trailing_punct(&w) + ":"
                    } else {
                        w
                    }));
                }
                for (k, item) in items.into_iter().enumerate() {
                    out.push(Tok::Break("\n"));
                    out.push(Tok::Word(match style {
                        ListStyle::Numbered => format!("{}.", k + 1),
                        _ => "-".to_string(),
                    }));
                    let words = item_words(&item.into_iter().map(Tok::Word).collect::<Vec<_>>());
                    out.extend(words.into_iter().map(Tok::Word));
                }
                out.push(Tok::Break("\n\n"));
            }
            None => out.extend(sentence.iter().cloned().map(Tok::Word)),
        }
        sentence.clear();
    };
    for tok in toks {
        match tok {
            Tok::Word(w) => {
                let end = ends_sentence(&w);
                sentence.push(w);
                if end {
                    flush(&mut sentence, &mut out);
                }
            }
            Tok::Break(b) => {
                flush(&mut sentence, &mut out);
                match out.last_mut() {
                    Some(Tok::Break(prev)) => {
                        if b == "\n\n" {
                            *prev = "\n\n";
                        }
                    }
                    _ => out.push(Tok::Break(b)),
                }
            }
        }
    }
    flush(&mut sentence, &mut out);
    while matches!(out.last(), Some(Tok::Break(_))) {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {

    #[test]
    fn long_dictation_breaks_where_the_thought_changes() {
        let text = "Hey Karol, I looked at the Datev export this morning and the VAT numbers still don't match the invoices from September. I think the rounding happens twice, once in the export and once in the import. Also, can you send me the workflow doc before Thursday so I can review it with Samir? Let me know if the call at three still works for you.";
        let out = paragraphs(text);
        let parts: Vec<&str> = out.split("\n\n").collect();
        assert_eq!(parts.len(), 3, "{out}");
        assert!(parts[1].starts_with("Also,"));
        assert!(parts[2].starts_with("Let me know"));
    }

    #[test]
    fn short_dictations_and_lists_stay_put() {
        let short = "Hey are you free for lunch tomorrow? Let's do 12. Also bring the charger.";
        assert_eq!(paragraphs(short), short);
        let list = "Groceries:\n- oat milk\n- eggs\n- spinach also bread";
        assert_eq!(paragraphs(list), list);
        let one_line = "Also ".repeat(50);
        assert_eq!(paragraphs(one_line.trim()), one_line.trim());
    }

    #[test]
    fn a_run_on_breaks_once_it_runs_long() {
        let sentence = "We moved the staging app from Coolify to Dokploy and kept the Netbird route for the ARM box.";
        let text = [sentence; 8].join(" ");
        let out = paragraphs(&text);
        assert!(out.contains("\n\n"), "{out}");
        for part in out.split("\n\n") {
            assert!(part.split_whitespace().count() >= PARAGRAPH_MIN_RUN);
        }
    }

    #[test]
    fn paragraphs_follow_their_switch() {
        let text = "I looked at the Datev export this morning and the VAT numbers still don't match. I think the rounding happens twice, once in the export and once in the import. Also, can you send me the workflow doc before Thursday so I can review it?";
        let mut rules = CleanupRules::default();
        assert!(light(text, &rules, ListStyle::Inline).contains("\n\nAlso,"));
        rules.paragraphs = false;
        assert!(!light(text, &rules, ListStyle::Inline).contains('\n'));
    }

    use super::*;

    fn all(text: &str) -> String {
        light(text, &CleanupRules::default(), ListStyle::Inline)
    }

    #[test]
    fn spoken_punctuation_types_the_marks() {
        assert_eq!(
            all("are you free comma Karol question mark"),
            "are you free, Karol?"
        );
        assert_eq!(
            all("Dear Robert new paragraph thanks period"),
            "Dear Robert\n\nThanks."
        );
        assert_eq!(all("milk, new line eggs"), "milk\nEggs");
        // The noun stays a noun.
        assert_eq!(
            all("the trial period ends soon"),
            "the trial period ends soon"
        );
        assert_eq!(all("add a comma here"), "add a comma here");
    }

    #[test]
    fn scratch_that_drops_the_sentence() {
        assert_eq!(
            all("Let's meet Monday. Scratch that, Tuesday works."),
            "Tuesday works."
        );
        assert_eq!(
            all("send it to Karol scratch that send it to Kasia"),
            "Send it to Kasia"
        );
    }

    #[test]
    fn corrections_keep_the_second_thing() {
        assert_eq!(
            all("let's meet at three, actually four"),
            "let's meet at four"
        );
        assert_eq!(all("twelve, actually one"), "one");
        assert_eq!(
            all("ask Karol, I mean Kasia, about it"),
            "ask Kasia about it"
        );
        assert_eq!(all("I mean it"), "I mean it");
        assert_eq!(all("at twelve no sixteen"), "at 16");
    }

    #[test]
    fn a_restarted_sentence_keeps_the_second_try() {
        assert_eq!(
            all("I want to do a meeting at 12 no I want to do it at 16 o'clock"),
            "I want to do it at 16 o'clock"
        );
        assert_eq!(
            all("send it to Karol, no, send it to Kasia"),
            "send it to Kasia"
        );
        assert_eq!(
            all("Okay. We meet Monday, actually we meet Tuesday."),
            "Okay. We meet Tuesday."
        );
        // Described, not meant: both stay.
        let described = "like if I say at 12 no I want it at 16 it should keep the second";
        assert_eq!(all(described), described);
        // Nothing said again: no restart.
        assert_eq!(all("there is no way, no way"), "there is no way, no way");
        assert_eq!(all("I'm sorry I'm late"), "I'm sorry I'm late");
    }

    #[test]
    fn numbers_become_digits() {
        assert_eq!(all("twenty five percent of two hundred"), "25% of 200");
        assert_eq!(
            all("one of them took ten minutes"),
            "one of them took 10 minutes"
        );
        assert_eq!(all("two hundred and five people"), "205 people");
        assert_eq!(all("in twenty twenty six"), "in 2026");
        assert_eq!(all("a hundred times"), "100 times");
        assert_eq!(all("twenty-five, thirty"), "25, 30");
        assert_eq!(all("three thousand four hundred"), "3400");
        assert_eq!(all("five percent"), "5%");
    }

    #[test]
    fn big_numbers_read_as_people_write_them() {
        assert_eq!(all("one million in funding"), "1 million in funding");
        assert_eq!(all("two point five million users"), "2.5 million users");
        assert_eq!(all("roughly one point four billion"), "roughly 1.4 billion");
        assert_eq!(all("three hundred thousand"), "300,000");
        assert_eq!(all("twenty five thousand"), "25,000");
        assert_eq!(all("two thousand and eight"), "2008");
        assert_eq!(
            all("I've told you a million times"),
            "I've told you a million times"
        );
        assert_eq!(
            all("it costs a thousand rupees"),
            "it costs a thousand rupees"
        );
        assert_eq!(
            all("a couple of hundred people"),
            "a couple of hundred people"
        );
        assert_eq!(
            all("the late eighteen hundreds"),
            "the late eighteen hundreds"
        );
    }

    #[test]
    fn decimals_ordinals_times_and_codes() {
        assert_eq!(all("version two point one"), "version 2.1");
        assert_eq!(all("zero point five percent"), "0.5%");
        assert_eq!(all("the twenty first of October"), "the 21st of October");
        assert_eq!(all("the twenty-second"), "the 22nd");
        assert_eq!(all("at three thirty"), "at 3:30");
        assert_eq!(all("it's twelve thirty"), "it's 12:30");
        assert_eq!(all("born in nineteen ninety eight"), "born in 1998");
        assert_eq!(all("my PIN is one two three four"), "my PIN is 1234");
        assert_eq!(all("room four oh four"), "room 404");
        assert_eq!(all("one, two, three"), "one, two, three");
    }

    #[test]
    fn numbers_side_by_side_match() {
        assert_eq!(all("sixteen by nine"), "16 by 9");
        assert_eq!(all("ten out of ten"), "10 out of 10");
        assert_eq!(all("one to two weeks"), "one to two weeks");
        assert_eq!(all("from five to ten"), "from 5 to 10");
    }

    #[test]
    fn punctuation_words_used_as_words_stay() {
        assert_eq!(all("He has colon cancer"), "He has colon cancer");
        assert_eq!(
            all("We launched a new line of products"),
            "We launched a new line of products"
        );
        assert_eq!(
            all("Start a new paragraph when"),
            "Start a new paragraph when"
        );
        assert_eq!(
            all("That's period accurate design"),
            "That's period accurate design"
        );
        assert_eq!(all("Thanks, period."), "Thanks.");
        assert_eq!(all("Hey, new line, how are you?"), "Hey\nHow are you?");
    }

    #[test]
    fn spoken_addresses_are_written_out() {
        assert_eq!(
            all("go to github dot com slash abhash"),
            "go to github.com/abhash"
        );
        assert_eq!(
            all("email abhash at gmail dot com."),
            "email abhash@gmail.com."
        );
        assert_eq!(all("open readme dot md"), "open readme.md");
        assert_eq!(all("I write C plus plus and C sharp"), "I write C++ and C#");
        assert_eq!(all("a polka dot dress"), "a polka dot dress");
        assert_eq!(all("the dot com bubble"), "the dot com bubble");
        assert_eq!(all("we meet at noon"), "we meet at noon");
    }

    #[test]
    fn meant_repeats_and_discourse_stay() {
        assert_eq!(all("bye bye, see you"), "bye bye, see you");
        assert_eq!(all("it's very very important"), "it's very very important");
        assert_eq!(all("it's a win win"), "it's a win win");
        assert_eq!(
            all("So, first of all, thank you. Second, the budget is tight."),
            "So, first of all, thank you. Second, the budget is tight."
        );
        assert_eq!(
            all("meet Karol, I mean Kasia, tomorrow"),
            "meet Kasia tomorrow"
        );
    }

    #[test]
    fn repeated_words_go() {
        assert_eq!(all("the the report"), "the report");
        assert_eq!(all("no, no, no"), "no, no, no");
        assert_eq!(all("he had had enough"), "he had had enough");
    }

    #[test]
    fn enumerations_become_numbered_lists() {
        assert_eq!(
            all("We need three things first the keys, second the routes and third the backup. Then ship it."),
            "We need three things:\n1. The keys\n2. The routes\n3. The backup\n\nThen ship it."
        );
    }

    #[test]
    fn named_things_become_bullets_in_documents() {
        let rules = CleanupRules::default();
        assert_eq!(
            light(
                "I need to buy milk, eggs, bread and butter.",
                &rules,
                ListStyle::Bullets
            ),
            "I need to buy:\n- Milk\n- Eggs\n- Bread\n- Butter"
        );
        assert_eq!(
            light(
                "Check the logs, the routes, and the certs. Then deploy.",
                &rules,
                ListStyle::Numbered
            ),
            "Check:\n1. The logs\n2. The routes\n3. The certs\n\nThen deploy."
        );
        // Long clauses stay a sentence.
        let prose =
            "I went home after the long meeting, ate a very late dinner with friends, and slept.";
        assert_eq!(light(prose, &rules, ListStyle::Bullets), prose);
    }

    #[test]
    fn switched_off_rules_do_nothing() {
        let off = CleanupRules {
            repeats: false,
            numbers: false,
            punctuation: false,
            corrections: false,
            lists: false,
            paragraphs: false,
        };
        let text = "the the twenty five comma scratch that";
        assert_eq!(light(text, &off, ListStyle::Inline), text);
    }
}
