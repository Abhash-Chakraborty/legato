//! What's worth knowing from the words on a screen: the names, products
//! and code identifiers in the window a dictation goes to, so a word the
//! speech model has never seen comes out the way the screen spells it.
//! Kept free of app state so it can be tested alone.

use std::collections::HashMap;

/// The most terms a dictation takes from the screen.
pub const MAX_TERMS: usize = 40;

/// The shortest and longest a term may be, in characters.
const MIN_CHARS: usize = 3;
const MAX_CHARS: usize = 40;

/// A token without the punctuation text recognition leaves around it.
fn core(token: &str) -> &str {
    token.trim_matches(|c: char| !c.is_alphanumeric())
}

/// Whether a token is written like code or a product: capitals inside it
/// ("TypeScript", "useState"), words joined by "_", "-" or "." ("snake_case",
/// "legato-phone", "main.rs"), or letters with digits ("gpt5", "h264").
fn written_like_a_name(token: &str) -> bool {
    let chars: Vec<char> = token.chars().collect();
    let hump = chars
        .windows(2)
        .any(|w| w[0].is_lowercase() && w[1].is_uppercase());
    let joined = chars.windows(3).any(|w| {
        matches!(w[1], '_' | '-' | '.') && w[0].is_alphanumeric() && w[2].is_alphanumeric()
    });
    let letters = chars.iter().filter(|c| c.is_alphabetic()).count();
    let digits = chars.iter().filter(|c| c.is_ascii_digit()).count();
    hump || joined || (letters >= 3 && (1..=4).contains(&digits))
}

/// Whether a token could be said aloud: it has a vowel, or it's a short
/// run of capitals read letter by letter ("MCP").
fn sayable(token: &str) -> bool {
    let letters: Vec<char> = token.chars().filter(|c| c.is_alphabetic()).collect();
    let acronym = (2..=6).contains(&letters.len()) && letters.iter().all(|c| c.is_uppercase());
    acronym
        || letters
            .iter()
            .any(|c| "aeiouyAEIOUY".contains(*c) || !c.is_ascii())
}

/// A token's parts when it's several words joined: "legato-phone" is
/// checked as "legato" and "phone".
fn parts(token: &str) -> impl Iterator<Item = &str> {
    token.split(['_', '-', '.']).filter(|p| !p.is_empty())
}

/// The terms on a screen worth passing on, most seen first: tokens written
/// like code or a product, and words the spell checker (`is_word`) doesn't
/// know. A capitalised or all-capitals unknown word counts on one sighting;
/// a lower-case one has to show twice, since a misread word shows once.
/// Addresses, paths and numbers are left out.
pub fn terms(text: &str, is_word: &dyn Fn(&str) -> bool) -> Vec<String> {
    // By lower-cased term: how often it shows, where first, and each
    // spelling's count.
    let mut seen: HashMap<String, (usize, usize, HashMap<String, usize>)> = HashMap::new();
    for (at, token) in text.split_whitespace().enumerate() {
        if token.contains(['/', '\\', '@']) || token.contains("://") {
            continue;
        }
        let token = core(token);
        let count = token.chars().count();
        if !(MIN_CHARS..=MAX_CHARS).contains(&count)
            || !token.chars().any(char::is_alphabetic)
            || !token
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.'))
            || !sayable(token)
        {
            continue;
        }
        let entry = seen
            .entry(token.to_lowercase())
            .or_insert((0, at, HashMap::new()));
        entry.0 += 1;
        *entry.2.entry(token.to_string()).or_default() += 1;
    }

    let mut found: Vec<(usize, usize, String)> = Vec::new();
    for (count, at, spellings) in seen.into_values() {
        // The spelling seen most; of equals, the one with more capitals,
        // which a title or a heading tends to get right.
        let Some(term) = spellings
            .iter()
            .max_by_key(|(s, n)| (**n, s.chars().filter(|c| c.is_uppercase()).count()))
            .map(|(s, _)| s.clone())
        else {
            continue;
        };
        let known = parts(&term).all(|p| is_word(p) || is_word(&p.to_lowercase()));
        let name_like = written_like_a_name(&term);
        // "well-known" and "re-run" are words joined, not names.
        if known && !(name_like && term.chars().any(char::is_uppercase)) {
            continue;
        }
        let capital = term.chars().next().is_some_and(char::is_uppercase);
        if name_like || capital || count >= 2 {
            found.push((count, at, term));
        }
    }
    found.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    found.truncate(MAX_TERMS);
    found.into_iter().map(|(_, _, term)| term).collect()
}

/// The terms a dictation may use: those on screen that the Dictionary
/// doesn't have already under any spelling (`known`, lower case).
pub fn fresh(terms: Vec<String>, known: &dyn Fn(&str) -> bool) -> Vec<String> {
    terms
        .into_iter()
        .filter(|t| !known(&t.to_lowercase()))
        .collect()
}

/// A window's title as context for the AI: without the app's own name at
/// the end ("notes.md - Visual Studio Code" is "notes.md"), unread
/// counters, and anything past `max` characters.
pub fn title_for_prompt(title: &str, app: &str, max: usize) -> Option<String> {
    let mut title = title.trim();
    for sep in [" - ", " – ", " — ", " | "] {
        if let Some((head, tail)) = title.rsplit_once(sep) {
            let tail = tail.trim().to_lowercase();
            let app = app.trim().to_lowercase();
            if !app.is_empty() && (tail.contains(&app) || app.contains(&tail)) {
                title = head.trim();
                break;
            }
        }
    }
    let title = match title.strip_prefix('(').and_then(|r| r.split_once(") ")) {
        Some((n, rest)) if n.chars().all(|c| c.is_ascii_digit()) => rest.trim(),
        _ => title,
    };
    if title.is_empty() || title.eq_ignore_ascii_case(app.trim()) {
        return None;
    }
    let cut: String = title.chars().take(max).collect();
    Some(cut)
}

/// The end of what was dictated just before, as context for what comes
/// next: its last `max` characters, from a word's start.
pub fn tail_for_prompt(before: &str, max: usize) -> Option<String> {
    let before = before.split_whitespace().collect::<Vec<_>>().join(" ");
    if before.is_empty() {
        return None;
    }
    let chars: Vec<char> = before.chars().collect();
    if chars.len() <= max {
        return Some(before);
    }
    let tail: String = chars[chars.len() - max..].iter().collect();
    let tail = tail
        .split_once(' ')
        .map_or(tail.as_str(), |(_, rest)| rest)
        .to_string();
    Some(format!("…{tail}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(w: &str) -> bool {
        [
            "the", "build", "is", "green", "and", "we", "ship", "on", "open", "file", "edit",
            "view", "well", "known", "run", "phone", "state", "use", "type", "script", "with",
            "deploy", "failed", "settings", "sara", "meeting", "notes", "for", "in", "to", "re",
        ]
        .contains(&w.to_lowercase().as_str())
    }

    #[test]
    fn names_products_and_code_are_picked_from_a_screen() {
        let screen = "File Edit View  legato-phone  main.rs\n\
            The Dokploy deploy failed on kubectl apply. Dokploy settings\n\
            fn useState() with TypeScript and MCP  well-known  re-run\n\
            kubectl get pods   https://example.com/a  C:\\Users\\sam  sam@example.com  12345\n\
            lnyon   xzqv   Meeting notes for Sara";
        let got = terms(screen, &words);
        // Most seen first.
        assert_eq!(&got[..2], ["Dokploy", "kubectl"]);
        for want in ["legato-phone", "main.rs", "useState", "TypeScript", "MCP"] {
            assert!(got.iter().any(|t| t == want), "{want} in {got:?}");
        }
        // Ordinary words, joined ordinary words, addresses, paths, numbers,
        // and words seen once in lower case or with nothing to say.
        for not in [
            "File",
            "well-known",
            "re-run",
            "12345",
            "lnyon",
            "xzqv",
            "Sara",
            "Meeting",
        ] {
            assert!(!got.iter().any(|t| t == not), "{not} in {got:?}");
        }
        assert!(!got
            .iter()
            .any(|t| t.contains("example") || t.contains("Users")));
    }

    #[test]
    fn one_spelling_per_term_and_no_more_than_the_limit() {
        let got = terms("github GitHub GITHUB GitHub", &words);
        assert_eq!(got, ["GitHub"]);
        let many: String = (0..80).map(|i| format!("Zorblat{i}x ")).collect();
        assert_eq!(terms(&many, &words).len(), MAX_TERMS);
        assert!(terms("", &words).is_empty());
    }

    #[test]
    fn words_the_dictionary_has_are_left_to_it() {
        let got = fresh(vec!["Dokploy".into(), "kubectl".into()], &|w| {
            w == "dokploy"
        });
        assert_eq!(got, ["kubectl"]);
    }

    #[test]
    fn a_title_and_the_last_dictation_are_trimmed_for_a_prompt() {
        assert_eq!(
            title_for_prompt(
                "notes.md - legato - Visual Studio Code",
                "Visual Studio Code",
                80
            )
            .as_deref(),
            Some("notes.md - legato")
        );
        assert_eq!(
            title_for_prompt("(3) Sam Rao | WhatsApp", "WhatsApp", 80).as_deref(),
            Some("Sam Rao")
        );
        assert_eq!(title_for_prompt("WhatsApp", "WhatsApp", 80), None);
        assert_eq!(title_for_prompt("  ", "Code", 80), None);
        assert_eq!(
            title_for_prompt("A very long title indeed", "", 6).as_deref(),
            Some("A very")
        );
        assert_eq!(
            tail_for_prompt("  see you   at four ", 80).as_deref(),
            Some("see you at four")
        );
        assert_eq!(
            tail_for_prompt("the first part and then the end of it", 12).as_deref(),
            Some("…end of it")
        );
        assert_eq!(tail_for_prompt(" ", 80), None);
    }
}
