//! Style: capitals and punctuation for the kind of app, by rules. No AI.
//! Each kind of app offers three tones; the pipeline runs the chosen one after
//! cleanup. Pure functions, tested without the app.

use crate::cleanup_rules::{CleanupRules, ListStyle};
use crate::snippet_rules::{ends_with_placeholder, is_placeholder_char};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    /// Capitals and full punctuation; in email, greeting and sign-off on
    /// their own lines.
    Formal,
    /// Capitals, lighter punctuation; in email, one paragraph.
    Casual,
    /// No capitals, lighter punctuation.
    Lower,
    /// Email: warmer, exclamation marks allowed.
    Excited,
    /// Documents: lists when you enumerate, no final periods.
    Notes,
    /// Exactly what cleanup left.
    Raw,
    /// "Tidy": sentences and capitals only.
    Light,
    /// AI prompts: paragraphs, numbered lists when you enumerate.
    Structured,
    /// Code: a capital and a final period.
    Sentence,
}

/// The tones a kind of app offers, in the order the Style page lists them.
pub fn tones_for(category: &str) -> [Tone; 3] {
    match category {
        "email" => [Tone::Formal, Tone::Casual, Tone::Excited],
        "documents" => [Tone::Formal, Tone::Casual, Tone::Notes],
        "ai" => [Tone::Raw, Tone::Light, Tone::Structured],
        "code" => [Tone::Raw, Tone::Lower, Tone::Sentence],
        _ => [Tone::Formal, Tone::Casual, Tone::Lower],
    }
}

/// The tone a kind of app starts with.
pub fn default_tone(category: &str) -> Tone {
    match category {
        "personal" | "other" => Tone::Casual,
        "ai" => Tone::Light,
        "code" => Tone::Lower,
        _ => Tone::Formal,
    }
}

/// How cleanup lays out three or more things named in a row, for this kind
/// of app and tone.
pub fn list_style(category: &str, tone: Tone, lists_rule: bool) -> ListStyle {
    match tone {
        Tone::Notes => ListStyle::Bullets,
        Tone::Structured => ListStyle::Numbered,
        _ if category == "documents" && lists_rule => ListStyle::Bullets,
        _ => ListStyle::Inline,
    }
}

/// The cleanup rules as they apply in this kind of app: code and terminals
/// never get paragraphs, since a blank line there is a command or a commit
/// body, not a pause for breath.
pub fn cleanup_rules_for(category: &str, rules: CleanupRules) -> CleanupRules {
    CleanupRules {
        paragraphs: rules.paragraphs && category != "code",
        ..rules
    }
}

fn is_list_line(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("- ")
        || t.split_once(". ")
            .is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}

const ABBREVIATIONS: &[&str] = &[
    "e.g.", "i.e.", "etc.", "vs.", "mr.", "mrs.", "ms.", "dr.", "approx.", "cf.",
];

/// Capitals at the start, after . ? ! and on every line; "i" as "I".
fn capitalize_sentences(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut upper_next = true;
    let chars: Vec<char> = text.chars().collect();
    for (k, &c) in chars.iter().enumerate() {
        if c.is_alphabetic() {
            // Addresses keep their case: "github.com", "abhash@gmail.com".
            let word: String = chars[k..]
                .iter()
                .take_while(|c| !c.is_whitespace())
                .collect();
            let stripped = word.trim_end_matches(|c: char| !c.is_alphanumeric());
            let address = stripped.contains('@')
                || stripped.split_once('.').is_some_and(|(name, rest)| {
                    !name.is_empty()
                        && rest.len() >= 2
                        && rest.starts_with(|c: char| c.is_alphanumeric())
                });
            if upper_next && !address {
                out.extend(c.to_uppercase());
            } else {
                out.push(c);
            }
            upper_next = false;
            continue;
        }
        out.push(c);
        // "10 out of 10": a sentence that opens with a number is under way.
        if c.is_ascii_digit() {
            upper_next = false;
        }
        match c {
            '\n' => upper_next = true,
            '.' | '?' | '!' => {
                // Not inside a URL (a space follows) or after "e.g.".
                let word = out.rsplit(char::is_whitespace).next().unwrap_or("");
                let abbreviation = ABBREVIATIONS.contains(&word.to_lowercase().as_str());
                upper_next = !abbreviation && chars.get(k + 1).is_some_and(|n| n.is_whitespace());
            }
            c if is_placeholder_char(c) => upper_next = false,
            _ => {}
        }
    }
    fix_i(&out)
}

/// "i", "i'm", "i'll", "i've", "i'd" as written English has them.
fn fix_i(text: &str) -> String {
    text.split(' ')
        .map(|w| {
            let core = w.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '\'');
            match core {
                "i" | "i'm" | "i'll" | "i've" | "i'd" => {
                    let mut s = String::from("I");
                    s.push_str(&w[1..]);
                    s
                }
                _ => w.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn last_line(text: &str) -> &str {
    text.rsplit('\n').next().unwrap_or(text)
}

/// A period at the end, unless it already ends with a mark, a list or a
/// snippet.
fn final_period(text: &str) -> String {
    let t = text.trim_end();
    let ends_word = t.chars().last().is_some_and(|c| c.is_alphanumeric());
    if !ends_word || ends_with_placeholder(t) || is_list_line(last_line(t)) {
        return t.to_string();
    }
    format!("{t}.")
}

fn drop_final_period(text: &str) -> String {
    let t = text.trim_end();
    match t.strip_suffix('.') {
        Some(rest) if !rest.ends_with('.') => rest.to_string(),
        _ => t.to_string(),
    }
}

const GREETINGS: &[&str] = &[
    "hi",
    "hello",
    "hey",
    "dear",
    "good morning",
    "good afternoon",
    "good evening",
];

/// Words that address people together after a greeting.
const GROUP_NAMES: &[&str] = &[
    "team",
    "all",
    "everyone",
    "everybody",
    "guys",
    "folks",
    "there",
    "both",
    "friends",
    "y'all",
];

/// Titles before a name, with their period.
const TITLES: &[&str] = &["mr.", "mrs.", "ms.", "dr.", "prof."];

/// The opening greeting and the name after it ("Hi Robert"), and where the
/// rest of the text starts.
fn greeting(text: &str) -> Option<(String, &str)> {
    let lower = text.to_lowercase();
    let word = GREETINGS.iter().find(|g| {
        lower.starts_with(*g)
            && lower[g.len()..]
                .chars()
                .next()
                .is_some_and(|c| c == ' ' || c == ',')
    })?;
    let after = &text[word.len()..];
    // The name: capitalised words up to a comma or the next lower-case word.
    let mut end = 0;
    let mut name_words = 0;
    for part in after.split_inclusive(' ') {
        let w = part.trim();
        if w.is_empty() {
            end += part.len();
            continue;
        }
        let bare = w.trim_end_matches([',', '.', '!']).to_lowercase();
        // "Hi team", "Hi all": people addressed together.
        let group = name_words == 0 && GROUP_NAMES.contains(&bare.as_str());
        // "Dear Mr. Sharma": the title's period doesn't end the name.
        if name_words < 2 && TITLES.contains(&w.to_lowercase().as_str()) {
            end += part.len();
            continue;
        }
        let starts_upper = group || w.chars().next().is_some_and(char::is_uppercase);
        // "Hi Karol I looked…": the pronoun starts the sentence, it's no name.
        let pronoun = matches!(
            w.trim_end_matches([',', '.', '!']),
            "I" | "I'm" | "I've" | "I'll" | "I'd" | "I’m" | "I’ve" | "I’ll" | "I’d"
        );
        if !starts_upper || pronoun || name_words == 2 {
            break;
        }
        end += part.len();
        name_words += 1;
        if w.ends_with([',', '.', '!']) {
            break;
        }
    }
    let head = text[..word.len() + end].trim_end();
    let head = head.trim_end_matches([',', '.', '!']).to_string();
    let rest = text[word.len() + end..].trim_start_matches([',', ' ', '.', '!']);
    Some((head, rest))
}

const SIGN_OFFS: &[&str] = &[
    "best regards",
    "kind regards",
    "warm regards",
    "best wishes",
    "many thanks",
    "thank you",
    "regards",
    "cheers",
    "thanks",
    "best",
];

/// The closing ("Best regards Abhash") split from the body: the phrase and
/// the name after it.
fn sign_off(text: &str) -> Option<(&str, String, String)> {
    let lower = text.to_lowercase();
    for phrase in SIGN_OFFS {
        let Some(at) = lower.rfind(phrase) else {
            continue;
        };
        // At the start of a sentence or after a comma.
        let before = text[..at].trim_end();
        if !(before.is_empty() || before.ends_with(['.', '!', '?', ','])) {
            continue;
        }
        let tail = &text[at + phrase.len()..];
        let name =
            tail.trim_matches(|c: char| c == ',' || c == '.' || c == '!' || c.is_whitespace());
        let name_ok = name.split_whitespace().count() <= 3
            && name
                .split_whitespace()
                .all(|w| w.chars().next().is_some_and(char::is_uppercase));
        // "thanks" alone often ends a sentence; it's a sign-off with a name.
        let needs_name = matches!(*phrase, "thanks" | "thank you" | "many thanks" | "best");
        if !name_ok || (needs_name && name.is_empty()) {
            continue;
        }
        let phrase_text = capitalize_first(&text[at..at + phrase.len()]);
        return Some((before.trim_end_matches(','), phrase_text, name.to_string()));
    }
    None
}

fn capitalize_first(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn lowercase_first(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        // "I" stays.
        Some(c) if text.starts_with("I ") || text.starts_with("I'") => {
            c.to_string() + chars.as_str()
        }
        Some(c) => c.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// "Hey Karol, can we" → "Hey Karol can we": messages drop the comma after
/// the greeting.
fn drop_greeting_comma(text: &str) -> String {
    match greeting(text) {
        Some((head, rest)) if !rest.is_empty() => format!("{head} {rest}"),
        _ => text.to_string(),
    }
}

const WARM: &[&str] = &[
    "thank",
    "thanks",
    "great",
    "awesome",
    "amazing",
    "love",
    "excited",
    "congrat",
    "glad",
    "happy",
    "wonderful",
    "fantastic",
];

/// Sentences that thank or praise end with "!".
fn excite(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut sentence = String::new();
    for c in text.chars() {
        sentence.push(c);
        if c == '.' {
            let lower = sentence.to_lowercase();
            if WARM.iter().any(|w| lower.contains(w)) {
                sentence.pop();
                sentence.push('!');
            }
            out.push_str(&sentence);
            sentence.clear();
        }
    }
    out.push_str(&sentence);
    out
}

fn email(text: &str, tone: Tone) -> String {
    if tone == Tone::Formal {
        return formal_email(text);
    }
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let (greet, body) = match greeting(&flat) {
        Some((g, rest)) => (Some(g), rest.to_string()),
        None => (None, flat.clone()),
    };
    let (body, closing) = match sign_off(&body) {
        Some((b, phrase, name)) => (b.to_string(), Some((phrase, name))),
        None => (body, None),
    };
    let mut body = final_period(&capitalize_sentences(&body));
    if tone == Tone::Excited {
        body = excite(&body);
    }
    let mut out = match greet {
        Some(g) => format!("{}, {}", capitalize_first(&g), lowercase_first(&body)),
        None => body,
    };
    if let Some((_, name)) = closing.filter(|(_, n)| !n.is_empty()) {
        out.push(' ');
        out.push_str(&name);
    }
    out
}

/// Greeting and sign-off on their own lines; the paragraphs cleanup made
/// (or you said) stay between them.
fn formal_email(text: &str) -> String {
    let mut paras: Vec<String> = text
        .split("\n\n")
        .map(|p| {
            p.split('\n')
                .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
                .filter(|l| !l.is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|p| !p.is_empty())
        .collect();
    let mut greet = None;
    if let Some(first) = paras.first() {
        if let Some((g, rest)) = greeting(first) {
            let rest = rest.trim().to_string();
            greet = Some(g);
            if rest.is_empty() {
                paras.remove(0);
            } else {
                paras[0] = rest;
            }
        }
    }
    let mut closing = None;
    if let Some(last) = paras.last() {
        if let Some((body, phrase, name)) = sign_off(last) {
            let body = body.trim().to_string();
            closing = Some((phrase.to_string(), name));
            if body.is_empty() {
                paras.pop();
            } else if let Some(last) = paras.last_mut() {
                *last = body;
            }
        }
    }
    let mut out = String::new();
    if let Some(g) = greet {
        out.push_str(&capitalize_first(&g));
        out.push_str(",\n\n");
    }
    out.push_str(
        &paras
            .iter()
            .map(|p| final_period(&capitalize_sentences(p)))
            .collect::<Vec<_>>()
            .join("\n\n"),
    );
    if let Some((phrase, name)) = closing {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(&phrase);
        out.push(',');
        if !name.is_empty() {
            out.push('\n');
            out.push_str(&name);
        }
    }
    out
}

/// Every paragraph a bullet, without final periods: the Notes tone.
fn notes(text: &str) -> String {
    let text = capitalize_sentences(text);
    let mut lines: Vec<String> = Vec::new();
    for line in text.split('\n').filter(|l| !l.trim().is_empty()) {
        if is_list_line(line) || line.trim_end().ends_with(':') {
            lines.push(line.trim_end_matches('.').to_string());
            continue;
        }
        let sentences: Vec<&str> = line
            .split_inclusive(['.', '!', '?'])
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        if sentences.len() < 2 && lines.is_empty() && !text.contains('\n') {
            return drop_final_period(line);
        }
        lines.extend(
            sentences
                .into_iter()
                .map(|s| format!("- {}", s.trim_end_matches('.'))),
        );
    }
    lines.join("\n")
}

fn lowercase(text: &str) -> String {
    text.chars().flat_map(char::to_lowercase).collect()
}

/// Applies a tone to cleaned-up text.
pub fn apply(text: &str, category: &str, tone: Tone) -> String {
    if text.trim().is_empty() {
        return text.to_string();
    }
    match (category, tone) {
        (_, Tone::Raw) => text.to_string(),
        ("email", Tone::Formal | Tone::Casual | Tone::Excited) => email(text, tone),
        (_, Tone::Formal) | (_, Tone::Light) | (_, Tone::Structured) => {
            final_period(&capitalize_sentences(text))
        }
        (_, Tone::Sentence) => final_period(&capitalize_sentences(text)),
        (_, Tone::Casual) => drop_greeting_comma(&drop_final_period(&capitalize_sentences(text))),
        (_, Tone::Lower) => lowercase(&drop_greeting_comma(&drop_final_period(text))),
        (_, Tone::Notes) => notes(text),
        (_, Tone::Excited) => excite(&final_period(&capitalize_sentences(text))),
    }
}

/// The spoken commands that press Enter at the end of a dictation.
const ENTER_COMMANDS: [&str; 2] = ["press enter", "hit enter"];

/// "…, press enter." at the end of a dictation: the text without the
/// command, and whether it was there. Cleanup may have capitalised it or
/// added a period, so case and punctuation around it don't matter.
pub fn spoken_enter(text: &str) -> (String, bool) {
    let body = text.trim_end_matches(|c: char| c.is_whitespace() || ".!?,;:".contains(c));
    let lower = body.to_lowercase();
    for command in ENTER_COMMANDS {
        if !lower.ends_with(command) {
            continue;
        }
        let cut = body.len() - command.len();
        // A whole word: "repress enter" isn't the command.
        if !body.is_char_boundary(cut)
            || body[..cut]
                .chars()
                .last()
                .is_some_and(char::is_alphanumeric)
        {
            continue;
        }
        let kept = body[..cut].trim_end_matches(|c: char| c.is_whitespace() || ",;:-".contains(c));
        return (kept.to_string(), true);
    }
    (text.to_string(), false)
}

/// Stacked messages: a dictation for a chat app, one message per
/// paragraph. A single line break (a list) stays inside its message.
pub fn stack_messages(text: &str) -> Vec<String> {
    text.replace("\r\n", "\n")
        .split("\n\n")
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

/// The kinds of app whose text boxes send messages.
pub fn sends_messages(category: &str) -> bool {
    matches!(category, "personal" | "work")
}

#[cfg(test)]
mod tests {
    use super::*;

    const KAROL: &str =
        "hey Karol, can we move the Datev review to Thursday? i'll bring the open questions";

    #[test]
    fn message_tones() {
        assert_eq!(
            apply(KAROL, "work", Tone::Formal),
            "Hey Karol, can we move the Datev review to Thursday? I'll bring the open questions."
        );
        assert_eq!(
            apply(KAROL, "personal", Tone::Casual),
            "Hey Karol can we move the Datev review to Thursday? I'll bring the open questions"
        );
        assert_eq!(
            apply(KAROL, "personal", Tone::Lower),
            "hey karol can we move the datev review to thursday? i'll bring the open questions"
        );
    }

    #[test]
    fn formal_email_keeps_paragraphs() {
        let text = "hi Karol I looked at the Datev export and the numbers don't match.\n\nAlso, can you send the workflow doc before Thursday? Best regards Abhash";
        assert_eq!(
            apply(text, "email", Tone::Formal),
            "Hi Karol,\n\nI looked at the Datev export and the numbers don't match.\n\nAlso, can you send the workflow doc before Thursday?\n\nBest regards,\nAbhash"
        );
        // Casual mail is still one paragraph.
        assert!(!apply(text, "email", Tone::Casual).contains("\n\n"));
    }

    #[test]
    fn greetings_to_groups_and_titles_stay_whole() {
        assert_eq!(
            apply(
                "Hi team, quick update, the release is Friday.",
                "email",
                Tone::Formal
            ),
            "Hi team,\n\nQuick update, the release is Friday."
        );
        assert_eq!(
            apply(
                "Dear Mr. Sharma, thank you for your email.",
                "email",
                Tone::Formal
            ),
            "Dear Mr. Sharma,\n\nThank you for your email."
        );
        assert_eq!(
            apply("Best regards, Abhash.", "email", Tone::Formal),
            "Best regards,\nAbhash"
        );
    }

    #[test]
    fn numbers_and_addresses_keep_their_case() {
        assert_eq!(
            apply("10 out of 10 would recommend", "work", Tone::Formal),
            "10 out of 10 would recommend."
        );
        assert_eq!(
            apply("github.com/abhash has it", "work", Tone::Formal),
            "github.com/abhash has it."
        );
    }

    #[test]
    fn code_never_gets_paragraphs() {
        let rules = CleanupRules::default();
        assert!(!cleanup_rules_for("code", rules).paragraphs);
        assert!(cleanup_rules_for("email", rules).paragraphs);
    }

    #[test]
    fn email_tones() {
        let said = "hi Robert thanks for the feedback on the ALV draft. the German copy is updated. best regards Abhash";
        assert_eq!(
            apply(said, "email", Tone::Formal),
            "Hi Robert,\n\nThanks for the feedback on the ALV draft. The German copy is updated.\n\nBest regards,\nAbhash"
        );
        assert_eq!(
            apply(said, "email", Tone::Casual),
            "Hi Robert, thanks for the feedback on the ALV draft. The German copy is updated. Abhash"
        );
        assert_eq!(
            apply(
                "hi Robert, thanks so much for the feedback. the copy is updated.",
                "email",
                Tone::Excited
            ),
            "Hi Robert, thanks so much for the feedback! The copy is updated."
        );
        // A plain "thanks" at the end is part of the body.
        assert_eq!(
            apply("I'll send it tomorrow, thanks", "email", Tone::Formal),
            "I'll send it tomorrow, thanks."
        );
    }

    #[test]
    fn document_tones() {
        let said = "the Dokploy migration is blocked on the Netbird routes. the scraper moves to a browser extension.";
        assert_eq!(
            apply(said, "documents", Tone::Notes),
            "- The Dokploy migration is blocked on the Netbird routes\n- The scraper moves to a browser extension"
        );
        assert_eq!(
            apply("dokploy migration is blocked.", "documents", Tone::Casual),
            "Dokploy migration is blocked"
        );
        assert_eq!(
            apply("I need:\n- Milk\n- Eggs", "documents", Tone::Formal),
            "I need:\n- Milk\n- Eggs"
        );
    }

    #[test]
    fn ai_and_code_tones() {
        let said = "look at the overlay code and figure out why the bar stays on top";
        assert_eq!(apply(said, "ai", Tone::Raw), said);
        assert_eq!(
            apply(said, "ai", Tone::Light),
            "Look at the overlay code and figure out why the bar stays on top."
        );
        assert_eq!(
            apply(
                "Add a retry around the Dokploy webhook call.",
                "code",
                Tone::Lower
            ),
            "add a retry around the dokploy webhook call"
        );
        assert_eq!(
            apply("add a retry. see e.g. the docs", "code", Tone::Sentence),
            "Add a retry. See e.g. the docs."
        );
    }

    #[test]
    fn snippets_get_no_period() {
        let marked = crate::snippet_rules::mark(
            "here is my LinkedIn",
            &[crate::snippet_rules::SnippetRule {
                id: 1,
                trigger: "my LinkedIn".into(),
                alone: false,
            }],
        );
        let styled = apply(&marked.text, "work", Tone::Formal);
        assert!(crate::snippet_rules::ends_with_placeholder(&styled));
        assert!(styled.starts_with("Here is "));
    }

    #[test]
    fn press_enter_at_the_end_is_a_command() {
        assert_eq!(
            spoken_enter("Sounds good, see you at six, press enter."),
            ("Sounds good, see you at six".to_string(), true)
        );
        assert_eq!(
            spoken_enter("On my way. Press Enter."),
            ("On my way.".to_string(), true)
        );
        assert_eq!(spoken_enter("hit enter"), (String::new(), true));
        // In the middle, or part of a word, it's just words.
        assert!(!spoken_enter("Press enter to send it, then wait.").1);
        assert!(!spoken_enter("They repress enter").1);
        assert_eq!(
            spoken_enter("No command here."),
            ("No command here.".to_string(), false)
        );
    }

    #[test]
    fn paragraphs_become_separate_messages() {
        assert_eq!(
            stack_messages("Hey Samir!\n\nAre we still on for lunch?\n\n"),
            vec!["Hey Samir!", "Are we still on for lunch?"]
        );
        // A list stays one message.
        assert_eq!(
            stack_messages("Get:\n- milk\n- eggs"),
            vec!["Get:\n- milk\n- eggs"]
        );
        assert!(sends_messages("work") && !sends_messages("email"));
    }
}
