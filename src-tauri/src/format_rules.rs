//! How the app a text goes into writes bold, lists and headings, what the
//! AI is told about it, and the conversion that makes an answer fit: a
//! `**bold**` meant for a document is `*bold*` in WhatsApp and plain words
//! in an email. Kept free of app state so it can be tested alone.

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use specta::Type;

/// The markup a destination shows as formatting.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum Markup {
    /// Markdown as it is: Mattermost, Discord, GitHub, Obsidian, Notion,
    /// AI chats and Markdown files.
    Markdown,
    /// One symbol each side, `*bold*` `_italic_` `~strike~`: WhatsApp,
    /// Slack and Signal. No headings or tables.
    Chat,
    /// `**bold**` `__italic__` `~~strike~~`: Telegram. No headings or
    /// tables.
    Telegram,
    /// No markup at all: email, documents and any ordinary text field,
    /// where the symbols would show as typed.
    #[default]
    Plain,
    /// Code editors and terminals: nothing is converted.
    Verbatim,
}

/// Apps and sites that show one symbol each side as formatting.
const CHAT: &[&str] = &["whatsapp", "slack", "signal"];

/// Apps and sites that render Markdown as typed or pasted.
const MARKDOWN: &[&str] = &[
    "discord",
    "mattermost",
    "element",
    "zulip",
    "rocket.chat",
    "github",
    "gitlab",
    "notion",
    "obsidian",
    "logseq",
    "typora",
    "reddit",
    "linear",
];

/// Whether an app's or site's name is `known`, alone or followed by more
/// ("Telegram Desktop").
fn named(name: &str, known: &str) -> bool {
    name == known
        || name
            .strip_prefix(known)
            .is_some_and(|rest| rest.starts_with(' '))
}

/// Whether a window title names a Markdown file, as editors put the file's
/// name in it ("notes.md - Visual Studio Code").
fn markdown_file(title: &str) -> bool {
    title
        .split(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '(' | ')' | '[' | ']'))
        .map(|word| word.trim_end_matches(['*', '●', '•']).to_lowercase())
        .any(|word| {
            [".md", ".mdx", ".markdown"]
                .iter()
                .any(|ext| word.len() > ext.len() && word.ends_with(ext))
        })
}

/// The markup of the place a text goes: by the site in a browser tab or the
/// app's name where it's a known one, else by the kind of app.
pub fn markup_for(category: &str, app: &str, site: Option<&str>, title: &str) -> Markup {
    let name = site.unwrap_or(app).trim().to_lowercase();
    if named(&name, "telegram") {
        return Markup::Telegram;
    }
    if CHAT.iter().any(|k| named(&name, k)) {
        return Markup::Chat;
    }
    if MARKDOWN.iter().any(|k| named(&name, k)) {
        return Markup::Markdown;
    }
    let file = markdown_file(title);
    match category {
        "code" if file => Markup::Markdown,
        "code" => Markup::Verbatim,
        "ai" => Markup::Markdown,
        _ if file => Markup::Markdown,
        _ => Markup::Plain,
    }
}

/// Android apps whose formatting is known, by package.
const PACKAGES: &[(&str, Markup)] = &[
    ("com.whatsapp", Markup::Chat),
    ("com.whatsapp.w4b", Markup::Chat),
    ("com.Slack", Markup::Chat),
    ("org.thoughtcrime.securesms", Markup::Chat),
    ("org.telegram.messenger", Markup::Telegram),
    ("org.telegram.messenger.web", Markup::Telegram),
    ("org.thunderdog.challegram", Markup::Telegram),
    ("com.discord", Markup::Markdown),
    ("com.mattermost.rn", Markup::Markdown),
    ("com.mattermost.rnbeta", Markup::Markdown),
    ("im.vector.app", Markup::Markdown),
    ("com.zulipmobile", Markup::Markdown),
    ("chat.rocket.android", Markup::Markdown),
    ("md.obsidian", Markup::Markdown),
    ("notion.id", Markup::Markdown),
    ("com.github.android", Markup::Markdown),
];

/// The markup of an Android app: by its package where it's a known one,
/// else by its kind. None for an app of no known kind, where nothing is
/// said or converted.
pub fn markup_for_package(package: &str, category: &str) -> Option<Markup> {
    if let Some((_, markup)) = PACKAGES.iter().find(|(p, _)| *p == package) {
        return Some(*markup);
    }
    match category {
        "ai" => Some(Markup::Markdown),
        "code" => Some(Markup::Verbatim),
        "personal" | "work" | "email" | "documents" => Some(Markup::Plain),
        _ => None,
    }
}

/// What the AI is told about formatting in the destination.
pub fn prompt_note(markup: Markup) -> &'static str {
    match markup {
        Markup::Markdown => {
            "It renders Markdown, so use Markdown where formatting helps: a list for \
             several items, **bold** for the one thing that must stand out, `code` for code. \
             Don't decorate."
        }
        Markup::Chat => {
            "It is a chat that shows *bold*, _italic_, ~strikethrough~ and `code` written \
             exactly like that, with one symbol each side. It has no headings and no tables: \
             use short lines, and put each item of a list on a line of its own, starting \
             with \"- \". Most messages need no formatting at all."
        }
        Markup::Telegram => {
            "It is a chat that shows **bold**, __italic__, ~~strikethrough~~ and `code` \
             written exactly like that. It has no headings and no tables: use short lines, \
             and put each item of a list on a line of its own, starting with \"- \". Most \
             messages need no formatting at all."
        }
        Markup::Plain => {
            "It shows plain text only: no Markdown, no asterisks, backticks or # headings, \
             because they would appear as typed. Use line breaks and paragraphs, and put each \
             item of a list on a line of its own, starting with \"- \" or \"1.\"."
        }
        Markup::Verbatim => {
            "It is code or a terminal: keep code, file names, paths and commands exactly, and \
             add no formatting."
        }
    }
}

static FENCE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\s*```").expect("valid regex"));
static HEADING: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s{0,3}#{1,6}\s+(.+?)\s*#*\s*$").expect("valid regex"));
static STAR_BULLET: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(\s*)[*+]\s+(\S.*)$").expect("valid regex"));
static RULE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*(?:-{3,}|\*{3,}|_{3,})\s*$").expect("valid regex"));
static TABLE_RULE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*\|?\s*:?-{2,}:?\s*(?:\|\s*:?-{2,}:?\s*)*\|?\s*$").expect("valid regex")
});
static LINK: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\[([^\]\n]+)\]\((https?://[^)\s]+)\)").expect("valid regex"));
static BOLD: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\*\*([^\s*](?:[^*\n]*[^\s*])?)\*\*").expect("valid regex"));
// `__bold__` is left alone everywhere: models write `**`, and two
// underscores each side is far more often a name such as `__init__`.
static STRIKE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"~~([^\s~](?:[^~\n]*[^\s~])?)~~").expect("valid regex"));
/// One symbol each side, with nothing word-like outside, so `2 * 3 * 4`
/// and `snake_case_name` are left alone.
static SINGLE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(^|[\s(\[{>])([*_~])([^\s*_~](?:[^*_~\n]*[^\s*_~])?)([*_~])($|[\s.,;:!?)\]}])")
        .expect("valid regex")
});

/// Links and emphasis in text that holds no code span.
fn emphasis(text: &str, markup: Markup) -> String {
    let text = LINK.replace_all(text, |c: &regex::Captures| {
        if c[1].trim() == &c[2] {
            c[2].to_string()
        } else {
            format!("{} ({})", &c[1], &c[2])
        }
    });
    match markup {
        Markup::Chat => {
            let text = BOLD.replace_all(&text, "*$1*");
            STRIKE.replace_all(&text, "~$1~").to_string()
        }
        Markup::Telegram => text.to_string(),
        _ => {
            let text = BOLD.replace_all(&text, "$1");
            let mut text = STRIKE.replace_all(&text, "$1").to_string();
            // A pair is taken out per pass; nested ones need another.
            for _ in 0..3 {
                let next = SINGLE
                    .replace_all(&text, |c: &regex::Captures| {
                        if c[2] == c[4] {
                            format!("{}{}{}", &c[1], &c[3], &c[5])
                        } else {
                            c[0].to_string()
                        }
                    })
                    .to_string();
                if next == text {
                    break;
                }
                text = next;
            }
            text
        }
    }
}

/// Emphasis outside code spans, for one line. Code spans keep what's inside
/// them; only plain text loses the ticks.
fn inline(line: &str, markup: Markup) -> String {
    let ticks = line.matches('`').count();
    if ticks == 0 || ticks % 2 == 1 {
        return emphasis(line, markup);
    }
    let mut out = String::new();
    for (i, part) in line.split('`').enumerate() {
        if i % 2 == 0 {
            out.push_str(&emphasis(part, markup));
        } else if markup == Markup::Plain {
            out.push_str(part);
        } else {
            out.push('`');
            out.push_str(part);
            out.push('`');
        }
    }
    out
}

/// A table row as a line: its cells joined with " · ".
fn table_row(line: &str) -> Option<String> {
    let t = line.trim();
    if !t.starts_with('|') || t.matches('|').count() < 2 {
        return None;
    }
    let cells: Vec<&str> = t
        .trim_matches('|')
        .split('|')
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .collect();
    Some(cells.join(" · "))
}

/// Makes text written with Markdown fit the destination: its own symbols
/// for bold and strikethrough, headings as a bold line or a plain one,
/// tables as lines, links as text and address. Markdown and code
/// destinations get the text as it is.
pub fn fit(text: &str, markup: Markup) -> String {
    if matches!(markup, Markup::Markdown | Markup::Verbatim) {
        return text.to_string();
    }
    let mut out: Vec<String> = Vec::new();
    let mut fenced = false;
    for line in text.lines() {
        if FENCE.is_match(line) {
            fenced = !fenced;
            // Chats show a fenced block as code; plain text has no use for
            // the fence.
            if markup != Markup::Plain {
                out.push(line.to_string());
            }
            continue;
        }
        if fenced {
            out.push(line.to_string());
            continue;
        }
        if RULE.is_match(line) || (line.contains('|') && TABLE_RULE.is_match(line)) {
            continue;
        }
        if let Some(c) = HEADING.captures(line) {
            let words = inline(&c[1], Markup::Plain);
            out.push(match markup {
                Markup::Chat => format!("*{words}*"),
                Markup::Telegram => format!("**{words}**"),
                _ => words,
            });
            continue;
        }
        if let Some(row) = table_row(line) {
            out.push(inline(&row, markup));
            continue;
        }
        let line = match STAR_BULLET.captures(line) {
            Some(c) => format!("{}- {}", &c[1], &c[2]),
            None => line.to_string(),
        };
        out.push(inline(&line, markup));
    }
    let mut fitted = out.join("\n");
    // A dropped rule or table line leaves its blank lines side by side.
    while fitted.contains("\n\n\n") {
        fitted = fitted.replace("\n\n\n", "\n\n");
    }
    if text.ends_with('\n') {
        fitted.push('\n');
    }
    fitted
}

/// [`fit`] for a model's answer to `input`. When the text the model was
/// given already holds such symbols (code, or a message written with the
/// app's own formatting), the answer's are taken as meant and left alone.
pub fn fit_answer(input: &str, answer: &str, markup: Markup) -> String {
    if fit(input, markup) != input {
        return answer.to_string();
    }
    fit(answer, markup)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_app_or_site_decides_the_markup() {
        let m = |category, app, site, title| markup_for(category, app, site, title);
        assert_eq!(m("personal", "WhatsApp", None, "WhatsApp"), Markup::Chat);
        assert_eq!(
            m("personal", "Chrome", Some("WhatsApp"), "(2) WhatsApp"),
            Markup::Chat
        );
        assert_eq!(
            m("personal", "Telegram Desktop", None, ""),
            Markup::Telegram
        );
        assert_eq!(m("work", "Slack", None, ""), Markup::Chat);
        assert_eq!(
            m("work", "Chrome", Some("Mattermost"), ""),
            Markup::Markdown
        );
        assert_eq!(m("personal", "Discord", None, ""), Markup::Markdown);
        assert_eq!(m("work", "Microsoft Teams", None, ""), Markup::Plain);
        assert_eq!(
            m("email", "Chrome", Some("Gmail"), "Inbox - Gmail"),
            Markup::Plain
        );
        assert_eq!(
            m("documents", "Word", None, "Report.docx - Word"),
            Markup::Plain
        );
        assert_eq!(m("documents", "Obsidian", None, ""), Markup::Markdown);
        assert_eq!(m("ai", "Claude", None, ""), Markup::Markdown);
        assert_eq!(
            m("code", "Code", None, "main.rs - legato"),
            Markup::Verbatim
        );
        assert_eq!(
            m(
                "code",
                "Code",
                None,
                "● README.md - legato - Visual Studio Code"
            ),
            Markup::Markdown
        );
        assert_eq!(
            m("other", "Notepad", None, "todo.md - Notepad"),
            Markup::Markdown
        );
        assert_eq!(
            m("other", "Notepad", None, "cmd.mdb - Notepad"),
            Markup::Plain
        );
        // A name that only starts like a known one is something else.
        assert_eq!(m("other", "Slackware Updater", None, ""), Markup::Plain);
    }

    #[test]
    fn markdown_becomes_the_chats_own_symbols() {
        let md = "## Plan for **Friday**\n\n* book the *big* room\n* ~~cancel~~ move the call\n\nSee [the doc](https://example.com/doc) and `run.sh`.";
        assert_eq!(
            fit(md, Markup::Chat),
            "*Plan for Friday*\n\n- book the *big* room\n- ~cancel~ move the call\n\nSee the doc (https://example.com/doc) and `run.sh`."
        );
        assert_eq!(
            fit(md, Markup::Telegram),
            "**Plan for Friday**\n\n- book the *big* room\n- ~~cancel~~ move the call\n\nSee the doc (https://example.com/doc) and `run.sh`."
        );
        assert_eq!(fit(md, Markup::Markdown), md);
        assert_eq!(fit(md, Markup::Verbatim), md);
    }

    #[test]
    fn plain_destinations_get_no_symbols() {
        let md = "# Notes\n\n**Budget**: _roughly_ 4 * 3 * 2 for `q3_report_final`.\n- one\n- **two**\n\n---\n\n| Name | Role |\n|------|------|\n| Sam | Lead |\n\n```\nlet a_b = 1 * 2;\n```\nUse snake_case_names and https://example.com/a_b_c.";
        assert_eq!(
            fit(md, Markup::Plain),
            "Notes\n\nBudget: roughly 4 * 3 * 2 for q3_report_final.\n- one\n- two\n\nName · Role\nSam · Lead\n\nlet a_b = 1 * 2;\nUse snake_case_names and https://example.com/a_b_c."
        );
        // A link whose text is its address is written once.
        assert_eq!(
            fit("[https://example.com](https://example.com)", Markup::Plain),
            "https://example.com"
        );
        // Text with nothing to convert comes back as it was, line end kept.
        assert_eq!(fit("See you at 4.\n", Markup::Plain), "See you at 4.\n");
        assert_eq!(
            fit("5 * 6 = 30, *really*", Markup::Plain),
            "5 * 6 = 30, really"
        );
        assert_eq!(
            fit("call __init__ first", Markup::Plain),
            "call __init__ first"
        );
    }

    #[test]
    fn an_answer_to_text_with_symbols_keeps_them() {
        // A selected message already written with symbols: they're meant.
        let answer = "*Team*, the build is **green**.";
        assert_eq!(
            fit_answer("*Team*, the build is green", answer, Markup::Plain),
            answer
        );
        // Code keeps its stars either way.
        let code = "def f(*args, **kwargs):\n    return g(**kwargs)";
        assert_eq!(fit_answer("add kwargs", code, Markup::Plain), code);
        // Dictated words have none, so the answer's are converted.
        assert_eq!(
            fit_answer(
                "the budget is important",
                "The **budget** is important.",
                Markup::Chat
            ),
            "The *budget* is important."
        );
    }

    #[test]
    fn phone_apps_have_their_markup_too() {
        assert_eq!(
            markup_for_package("com.whatsapp", "personal"),
            Some(Markup::Chat)
        );
        assert_eq!(
            markup_for_package("org.telegram.messenger", "personal"),
            Some(Markup::Telegram)
        );
        assert_eq!(
            markup_for_package("com.mattermost.rn", "other"),
            Some(Markup::Markdown)
        );
        assert_eq!(
            markup_for_package("com.google.android.gm", "email"),
            Some(Markup::Plain)
        );
        assert_eq!(markup_for_package("com.android.chrome", "other"), None);
    }

    #[test]
    fn every_markup_has_a_note_for_the_ai() {
        for m in [
            Markup::Markdown,
            Markup::Chat,
            Markup::Telegram,
            Markup::Plain,
            Markup::Verbatim,
        ] {
            assert!(!prompt_note(m).is_empty());
        }
    }
}
