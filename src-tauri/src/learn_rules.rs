//! Learn from my edits: the words someone corrected in a field right after
//! Legato pasted into it.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::audio_toolkit::text::word_distance;

/// A correction is deliberate, so it passes at Loose matching even when the
/// Dictionary matches more strictly.
pub const MIN_THRESHOLD: f64 = 0.28;
/// Characters of the field kept on each side of a proposed word.
const CONTEXT_SIDE: usize = 30;
/// Share of the pasted words the field must still hold, or the paste is
/// taken as gone (deleted, or the field is another one).
const MIN_KEPT: f64 = 0.6;
/// A paste this short is judged by how many words changed, not what share.
const SHORT_PASTE: usize = 3;
/// Larger pastes or fields aren't aligned word by word.
const MAX_PASTED_WORDS: usize = 400;
const MAX_FIELD_WORDS: usize = 5000;

/// The words people correct most are ordinary ones the model got right in
/// another form ("their" for "there"); they aren't Dictionary words.
const COMMON: &[&str] = &[
    "a",
    "about",
    "above",
    "after",
    "again",
    "against",
    "all",
    "also",
    "am",
    "an",
    "and",
    "any",
    "are",
    "around",
    "as",
    "at",
    "be",
    "because",
    "been",
    "before",
    "being",
    "below",
    "between",
    "both",
    "but",
    "by",
    "can",
    "could",
    "did",
    "do",
    "does",
    "doing",
    "done",
    "down",
    "during",
    "each",
    "even",
    "every",
    "few",
    "for",
    "from",
    "further",
    "get",
    "gets",
    "go",
    "goes",
    "going",
    "gone",
    "good",
    "got",
    "great",
    "had",
    "has",
    "have",
    "having",
    "he",
    "hello",
    "her",
    "here",
    "hers",
    "herself",
    "hey",
    "hi",
    "him",
    "himself",
    "his",
    "how",
    "i",
    "if",
    "in",
    "into",
    "is",
    "it",
    "its",
    "itself",
    "just",
    "know",
    "let",
    "like",
    "made",
    "make",
    "many",
    "may",
    "me",
    "might",
    "more",
    "most",
    "much",
    "must",
    "my",
    "myself",
    "need",
    "new",
    "no",
    "nor",
    "not",
    "now",
    "of",
    "off",
    "ok",
    "okay",
    "old",
    "on",
    "once",
    "one",
    "only",
    "or",
    "other",
    "our",
    "ours",
    "ourselves",
    "out",
    "over",
    "own",
    "please",
    "put",
    "really",
    "right",
    "said",
    "same",
    "say",
    "see",
    "she",
    "should",
    "so",
    "some",
    "still",
    "such",
    "sure",
    "take",
    "than",
    "thank",
    "thanks",
    "that",
    "the",
    "their",
    "theirs",
    "them",
    "themselves",
    "then",
    "there",
    "these",
    "they",
    "thing",
    "things",
    "think",
    "this",
    "those",
    "though",
    "through",
    "time",
    "to",
    "today",
    "tomorrow",
    "too",
    "two",
    "under",
    "until",
    "up",
    "us",
    "use",
    "very",
    "want",
    "was",
    "way",
    "we",
    "well",
    "were",
    "what",
    "when",
    "where",
    "whether",
    "which",
    "while",
    "who",
    "whom",
    "why",
    "will",
    "with",
    "without",
    "would",
    "yeah",
    "yes",
    "yesterday",
    "yet",
    "you",
    "your",
    "yours",
    "yourself",
    "yourselves",
];

/// A word someone corrected: what Legato pasted, what they made it, and a
/// little of the text around it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    pub heard: String,
    pub word: String,
    pub context: String,
}

struct Token<'a> {
    /// The word without the punctuation around it.
    core: &'a str,
    start: usize,
    end: usize,
}

fn tokens(text: &str) -> Vec<Token<'_>> {
    let mut out = Vec::new();
    let mut from = 0;
    for raw in text.split_whitespace() {
        // split_whitespace yields slices of `text`, in order.
        let at = from + text[from..].find(raw).unwrap_or(0);
        from = at + raw.len();
        let core = raw.trim_matches(|c: char| !c.is_alphanumeric());
        if core.is_empty() {
            continue;
        }
        let start = at + raw.find(core).unwrap_or(0);
        out.push(Token {
            core,
            start,
            end: start + core.len(),
        });
    }
    out
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Op {
    Same,
    /// A pasted word replaced by a field word.
    Swap,
    /// A pasted word that's gone.
    Gone,
    /// A field word that wasn't pasted.
    Added,
}

/// Aligns the pasted words with the stretch of the field that best matches
/// them (any field text before or after is free). Returns the operations
/// and the index of the first field word in the stretch.
fn align(pasted: &[Token<'_>], field: &[Token<'_>]) -> (Vec<Op>, usize) {
    let (m, n) = (pasted.len(), field.len());
    let w = n + 1;
    let mut cost = vec![0u32; (m + 1) * w];
    for i in 1..=m {
        cost[i * w] = i as u32;
        for j in 1..=n {
            let same = pasted[i - 1].core == field[j - 1].core;
            let swap = cost[(i - 1) * w + j - 1] + u32::from(!same);
            let gone = cost[(i - 1) * w + j] + 1;
            let added = cost[i * w + j - 1] + 1;
            cost[i * w + j] = swap.min(gone).min(added);
        }
    }
    // Of two ends that cost the same, the later one: a last word that was
    // changed reads as swapped for the field's word, not as deleted.
    let mut j = (0..=n).rev().min_by_key(|&j| cost[m * w + j]).unwrap_or(0);
    let mut i = m;
    let mut ops = Vec::new();
    while i > 0 {
        let here = cost[i * w + j];
        if j > 0 {
            let same = pasted[i - 1].core == field[j - 1].core;
            if here == cost[(i - 1) * w + j - 1] + u32::from(!same) {
                ops.push(if same { Op::Same } else { Op::Swap });
                i -= 1;
                j -= 1;
                continue;
            }
            if here == cost[i * w + j - 1] + 1 {
                ops.push(Op::Added);
                j -= 1;
                continue;
            }
        }
        ops.push(Op::Gone);
        i -= 1;
    }
    ops.reverse();
    (ops, j)
}

fn is_common(word: &str) -> bool {
    word.split(' ')
        .all(|w| COMMON.contains(&w.to_lowercase().as_str()))
}

/// URLs, email addresses, paths, numbers and code identifiers aren't words
/// to learn.
fn looks_like_code(word: &str) -> bool {
    let chars: Vec<char> = word.chars().collect();
    !chars.iter().any(|c| c.is_alphabetic())
        || chars
            .iter()
            .any(|c| "@/\\_=<>{}[]()#$%^&*|~`+:;\"".contains(*c))
        || chars
            .windows(3)
            .any(|w| w[1] == '.' && w[0].is_alphanumeric() && w[2].is_alphanumeric())
}

/// A little of the field around a word, cut between words.
pub fn context(field: &str, start: usize, end: usize) -> String {
    let head: Vec<char> = field[..start].chars().collect();
    let from = head.len().saturating_sub(CONTEXT_SIDE);
    let mut before: String = head[from..].iter().collect();
    if from > 0 {
        // Starts mid-word: drop the rest of that word.
        if !head[from - 1].is_whitespace() {
            before = before
                .split_once(char::is_whitespace)
                .map(|(_, rest)| rest.to_string())
                .unwrap_or_default();
        }
        before = format!("…{}", before.trim_start());
    }
    let tail: Vec<char> = field[end..].chars().collect();
    let mut after: String = tail.iter().take(CONTEXT_SIDE).collect();
    if tail.len() > CONTEXT_SIDE {
        if !tail[CONTEXT_SIDE].is_whitespace() {
            after = after
                .rsplit_once(char::is_whitespace)
                .map(|(kept, _)| kept.to_string())
                .unwrap_or_default();
        }
        after = format!("{}…", after.trim_end());
    }
    format!("{}{}{}", before, &field[start..end], after)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Whether an edit is everyday editing, not a word to learn: the new word
/// is an ordinary lowercase word ("its" made "it's", "deploy" made
/// "deployed", "affect" made "effect"), or only a number in it changed
/// ("10am" made "11am"). `is_word` is the spell checker's answer. A
/// capitalised word is kept: a name can be an ordinary word too.
pub fn everyday_edit(edit: &Edit, is_word: &dyn Fn(&str) -> bool) -> bool {
    let letters = |s: &str| -> String { s.chars().filter(|c| !c.is_ascii_digit()).collect() };
    if edit.word.chars().any(|c| c.is_ascii_digit())
        && letters(&edit.word).eq_ignore_ascii_case(&letters(&edit.heard))
    {
        return true;
    }
    edit.word == edit.word.to_lowercase()
        && edit
            .word
            .split_whitespace()
            .all(|w| is_word(w.trim_matches(|c: char| !c.is_alphanumeric())))
}

/// The words someone changed in `field` after Legato pasted `pasted` there:
/// one or two words swapped for one or two others that look or sound like
/// them, at `threshold` or Loose, whichever is looser. Ordinary words, case
/// changed to lower, and anything that reads as code are left out.
pub fn find_edits(pasted: &str, field: &str, threshold: f64) -> Vec<Edit> {
    let p = tokens(pasted);
    let f = tokens(field);
    if p.is_empty() || p.len() > MAX_PASTED_WORDS || f.len() > MAX_FIELD_WORDS {
        return Vec::new();
    }
    let (ops, first) = align(&p, &f);
    let kept = ops.iter().filter(|op| **op == Op::Same).count();
    // A paste of a word or three can't keep six in ten of its words and
    // still be corrected: "Hi Carol" fixed to "Hi Karol" keeps half, and
    // "doc ploy" fixed to "Dokploy" keeps none. It counts when the field
    // holds about as many words, as it does when the text was corrected on
    // its own, and at most two of them changed; what changed still has to
    // look or sound like what it replaced.
    let enough = if p.len() <= SHORT_PASTE {
        kept + 2 >= p.len() && f.len() <= p.len() + 1
    } else {
        (kept as f64) >= MIN_KEPT * p.len() as f64
    };
    if !enough || kept == p.len() && ops.len() == p.len() {
        return Vec::new();
    }
    let threshold = threshold.max(MIN_THRESHOLD);
    let mut edits: Vec<Edit> = Vec::new();
    let (mut pi, mut fi) = (0usize, first);
    let mut block: (Vec<usize>, Vec<usize>) = (Vec::new(), Vec::new());
    let flush = |block: &mut (Vec<usize>, Vec<usize>), edits: &mut Vec<Edit>| {
        let (heard_ix, word_ix) = std::mem::take(block);
        if heard_ix.is_empty() || word_ix.is_empty() || heard_ix.len() > 2 || word_ix.len() > 2 {
            return;
        }
        let heard = heard_ix
            .iter()
            .map(|&i| p[i].core)
            .collect::<Vec<_>>()
            .join(" ");
        let word = word_ix
            .iter()
            .map(|&i| f[i].core)
            .collect::<Vec<_>>()
            .join(" ");
        let lowered = word.to_lowercase() == word && heard.to_lowercase() == word;
        let close = word_distance(&heard, &word).is_some_and(|d| d <= threshold);
        if heard == word
            || lowered
            || !close
            || word.chars().count() < 2
            || is_common(&word)
            || looks_like_code(&word)
            || edits
                .iter()
                .any(|e| e.word == word && e.heard.eq_ignore_ascii_case(&heard))
        {
            return;
        }
        let (start, end) = (f[word_ix[0]].start, f[*word_ix.last().unwrap_or(&0)].end);
        edits.push(Edit {
            heard,
            word,
            context: context(field, start, end),
        });
    };
    for op in ops {
        match op {
            Op::Same => {
                flush(&mut block, &mut edits);
                pi += 1;
                fi += 1;
            }
            Op::Swap => {
                block.0.push(pi);
                block.1.push(fi);
                pi += 1;
                fi += 1;
            }
            Op::Gone => {
                block.0.push(pi);
                pi += 1;
            }
            Op::Added => {
                block.1.push(fi);
                fi += 1;
            }
        }
    }
    flush(&mut block, &mut edits);
    edits
}

/// Times a term must be written, in all its forms, before it's suggested.
pub const MIN_SPELLING_COUNT: u32 = 3;
/// Most words a term is split into ("cool if I").
const MAX_SPLIT: usize = 3;
/// Shorter terms meet too many ordinary words by chance.
const MIN_KEY_CHARS: usize = 5;
/// Forms listed for a term.
const MAX_FORMS: usize = 4;

/// A term the model wrote more than one way, such as "Coolify" and
/// "cool if I": adding it to the Dictionary fixes every version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Spelling {
    /// The term as one word, the way it was capitalised.
    pub word: String,
    /// The ways it was written, most often first, `word` among them.
    pub forms: Vec<String>,
    pub count: u32,
    pub last_at: i64,
    pub last_app: Option<String>,
    pub last_app_path: Option<String>,
}

/// One dictation, as the model wrote it.
pub struct Said<'a> {
    pub text: &'a str,
    pub at: i64,
    pub app: Option<&'a str>,
    pub app_path: Option<&'a str>,
}

/// Letters only, folded so near-spellings meet: case, c and k, y and i,
/// ph and f, and doubled letters.
fn spelling_key(form: &str) -> String {
    let letters: String = form
        .chars()
        .filter(|c| c.is_alphabetic())
        .flat_map(char::to_lowercase)
        .collect();
    let mut key = String::new();
    for c in letters.replace("ph", "f").chars() {
        let c = match c {
            'c' => 'k',
            'y' => 'i',
            c => c,
        };
        if !key.ends_with(c) {
            key.push(c);
        }
    }
    key
}

/// Whether the token at `i` is inside a sentence, where a capital marks a
/// name rather than the start.
fn mid_sentence(text: &str, toks: &[Token<'_>], i: usize) -> bool {
    i > 0 && !text[toks[i - 1].end..toks[i].start].contains(['.', '!', '?', '\n', '"', '“', ':'])
}

/// The words in a dictation as forms of up to three words, kept within a
/// phrase: (form, words in it, token index).
fn forms_in<'a>(text: &'a str, toks: &[Token<'a>]) -> Vec<(String, usize, usize)> {
    let mut out = Vec::new();
    for i in 0..toks.len() {
        for n in 1..=MAX_SPLIT.min(toks.len() - i) {
            if n > 1
                && !text[toks[i + n - 2].end..toks[i + n - 1].start]
                    .chars()
                    .all(char::is_whitespace)
            {
                break;
            }
            let form = toks[i..i + n]
                .iter()
                .map(|t| t.core)
                .collect::<Vec<_>>()
                .join(" ");
            out.push((form, n, i));
        }
    }
    out
}

/// Terms the model wrote more than one way in these dictations, most often
/// first. A term counts when it's written as one capitalised word inside a
/// sentence at least once, which keeps out ordinary pairs like "every day"
/// and "everyday", and as something else the rest of the time.
pub fn spelled_differently(said: &[Said<'_>]) -> Vec<Spelling> {
    // Names first: one capitalised word inside a sentence.
    let mut names: HashSet<String> = HashSet::new();
    for s in said {
        let toks = tokens(s.text);
        for (i, t) in toks.iter().enumerate() {
            if t.core.chars().next().is_some_and(char::is_uppercase)
                && mid_sentence(s.text, &toks, i)
                && !is_common(t.core)
                && !looks_like_code(t.core)
            {
                let key = spelling_key(t.core);
                if key.chars().count() >= MIN_KEY_CHARS {
                    names.insert(key);
                }
            }
        }
    }
    if names.is_empty() {
        return Vec::new();
    }

    #[derive(Default)]
    struct Group {
        forms: HashMap<String, (u32, bool)>,
        count: u32,
        last_at: i64,
        last_app: Option<String>,
        last_app_path: Option<String>,
    }
    let mut groups: HashMap<String, Group> = HashMap::new();
    for s in said {
        let toks = tokens(s.text);
        for (form, n, i) in forms_in(s.text, &toks) {
            let key = spelling_key(&form);
            if !names.contains(&key) {
                continue;
            }
            let named = n == 1
                && form.chars().next().is_some_and(char::is_uppercase)
                && mid_sentence(s.text, &toks, i);
            let g = groups.entry(key).or_default();
            let f = g.forms.entry(form).or_default();
            f.0 += 1;
            f.1 |= named;
            g.count += 1;
            if s.at >= g.last_at {
                g.last_at = s.at;
                g.last_app = s.app.map(str::to_string);
                g.last_app_path = s.app_path.map(str::to_string);
            }
        }
    }

    let mut out: Vec<Spelling> = groups
        .into_values()
        .filter_map(|g| {
            let distinct: HashSet<String> = g.forms.keys().map(|f| f.to_lowercase()).collect();
            if g.count < MIN_SPELLING_COUNT || distinct.len() < 2 {
                return None;
            }
            let mut forms: Vec<(String, u32, bool)> = g
                .forms
                .into_iter()
                .map(|(f, (n, named))| (f, n, named))
                .collect();
            forms.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            let word = forms.iter().find(|f| f.2)?.0.clone();
            Some(Spelling {
                word,
                forms: forms.into_iter().take(MAX_FORMS).map(|f| f.0).collect(),
                count: g.count,
                last_at: g.last_at,
                last_app: g.last_app,
                last_app_path: g.last_app_path,
            })
        })
        .collect();
    out.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then(b.last_at.cmp(&a.last_at))
            .then_with(|| a.word.cmp(&b.word))
    });
    out
}

/// A name's words, lowercased, split at anything but letters and digits:
/// "A.Chakraborty" and "a.chakraborty@x" both give ["a", "chakraborty"].
fn name_words(name: &str) -> Vec<String> {
    let name = name.split('@').next().unwrap_or(name);
    name.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Two spellings of one word, allowing a slip or two in a long one
/// ("Chkraborty" for "Chakraborty"), none in a short one.
fn close_words(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let shorter = a.len().min(b.len());
    let allowed = match shorter {
        0..=3 => 0,
        4..=5 => 1,
        _ => 2,
    };
    if allowed == 0 || a.len().abs_diff(b.len()) > allowed {
        return false;
    }
    // Levenshtein over two rows.
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut row = vec![i + 1; b.len() + 1];
        for (j, cb) in b.iter().enumerate() {
            row[j + 1] = (prev[j] + usize::from(ca != cb))
                .min(prev[j + 1] + 1)
                .min(row[j] + 1);
        }
        prev = row;
    }
    prev[b.len()] <= allowed
}

/// Whether two names from invites, a Meet window or a summary are one
/// person: "Priya" and "Priya Shah", "A. Chakraborty", "a.chakraborty" and
/// "Abhash Chakraborty", "abhashchakraborty" and "Abhash Chakraborty", with
/// a slip in a long surname. "Priya Shah" and "Priya Patel" aren't.
pub fn same_person(a: &str, b: &str) -> bool {
    let (a, b) = (name_words(a), name_words(b));
    if a.is_empty() || b.is_empty() {
        return false;
    }
    if a.concat() == b.concat() {
        return true;
    }
    let (short, long) = if a.len() <= b.len() {
        (&a, &b)
    } else {
        (&b, &a)
    };
    let (first, last) = (&long[0], &long[long.len() - 1]);
    if short.len() == 1 {
        let one = &short[0];
        if long.len() == 1 {
            return close_words(one, first);
        }
        // A first name, or an address such as "achakraborty".
        let initial_last = format!("{}{last}", first.chars().next().unwrap_or(' '));
        return one == first
            || close_words(one, &long.concat())
            || (one.chars().count() > 4 && close_words(one, &initial_last));
    }
    let (s_first, s_last) = (&short[0], &short[short.len() - 1]);
    let initial = |w: &str, of: &str| w.chars().count() == 1 && of.starts_with(w);
    let firsts = s_first == first || initial(s_first, first) || initial(first, s_first);
    let lasts = close_words(s_last, last) || (initial(s_last, last) && s_first == first);
    firsts && lasts
}

/// A Dictionary word as merge suggestions see it.
pub struct WordForMerge<'a> {
    pub id: i64,
    pub text: &'a str,
    pub hits: i64,
}

/// At most this many merges are suggested at once.
const MERGE_SUGGESTIONS: usize = 8;

/// Pairs of Dictionary words that look like one word, as (keep, other): a
/// name and the same person's full name or initial ("Abhash", "A.
/// Chakraborty" and "Abhash Chakraborty"), or two spellings that fold to
/// one ("Dokploy", "DocPloy"). The word kept has the most words,
/// then the most fixes. Pairs in `apart` (lowercase, in order) were said
/// not to be the same.
pub fn merge_suggestions(
    words: &[WordForMerge<'_>],
    apart: &[(String, String)],
) -> Vec<(i64, i64)> {
    let named = |w: &str| w.chars().next().is_some_and(char::is_uppercase);
    let mut out = Vec::new();
    for (i, a) in words.iter().enumerate() {
        for b in &words[i + 1..] {
            let (x, y) = (a.text.to_lowercase(), b.text.to_lowercase());
            let pair = if x <= y { (x, y) } else { (y, x) };
            if apart.contains(&pair) {
                continue;
            }
            let person = named(a.text) && named(b.text) && same_person(a.text, b.text);
            let key = spelling_key(a.text);
            let spelled = key.chars().count() >= 5 && key == spelling_key(b.text);
            if !(person || spelled) {
                continue;
            }
            let size = |w: &WordForMerge<'_>| {
                (
                    w.text.split_whitespace().count(),
                    w.hits,
                    w.text.chars().filter(|c| c.is_alphabetic()).count(),
                )
            };
            let (keep, other) = if size(b) > size(a) { (b, a) } else { (a, b) };
            out.push((keep.id, other.id));
            if out.len() == MERGE_SUGGESTIONS {
                return out;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(edits: &[Edit]) -> Vec<(&str, &str)> {
        edits
            .iter()
            .map(|e| (e.heard.as_str(), e.word.as_str()))
            .collect()
    }

    #[test]
    fn finds_a_corrected_name_and_a_split_word() {
        let pasted = "Hey Carol, the doc ploy deploy failed again on the staging box.";
        let field = "Hey Karol, the Dokploy deploy failed again on the staging box.";
        assert_eq!(
            words(&find_edits(pasted, field, 0.18)),
            vec![("Carol", "Karol"), ("doc ploy", "Dokploy")]
        );
    }

    #[test]
    fn finds_the_paste_inside_other_text() {
        let pasted = "can we move the datev review to Thursday";
        let field = "Morning all,\n\ncan we move the Datev review to Thursday? I'll bring the export.\n\nAbhash";
        let edits = find_edits(pasted, field, 0.18);
        assert_eq!(words(&edits), vec![("datev", "Datev")]);
        assert_eq!(
            edits[0].context,
            "Morning all, can we move the Datev review to Thursday? I'll…"
        );
    }

    #[test]
    fn ignores_rewrites_ordinary_words_and_code() {
        // Reworded, not corrected.
        assert!(find_edits("let's meet at noon", "let's meet after lunch", 0.18).is_empty());
        // Ordinary words.
        assert!(find_edits("put it over their", "put it over there", 0.18).is_empty());
        // Lowercased by hand.
        assert!(find_edits("the Staging box", "the staging box", 0.18).is_empty());
        // Code, paths and addresses.
        assert!(find_edits("run the build script", "run the build_script", 0.18).is_empty());
        assert!(find_edits("mail karol at datev", "mail karol@datev.de", 0.18).is_empty());
        // No change at all.
        assert!(find_edits("all good here", "all good here", 0.18).is_empty());
    }

    #[test]
    fn a_short_paste_teaches_and_everyday_edits_do_not() {
        // Corrected on its own, a paste of two words keeps one.
        assert_eq!(
            words(&find_edits("Hi Carol", "Hi Karol", 0.18)),
            vec![("Carol", "Karol")]
        );
        assert_eq!(
            words(&find_edits("doc ploy", "Dokploy", 0.18)),
            vec![("doc ploy", "Dokploy")]
        );
        // In a field with more around it, two words are too few to place.
        assert!(find_edits("Hi Carol", "Morning. Hi Karol, see below for more", 0.18).is_empty());

        let is_word = |w: &str| ["it's", "deployed", "effect", "karol"].contains(&w);
        let edit = |heard: &str, word: &str| Edit {
            heard: heard.into(),
            word: word.into(),
            context: String::new(),
        };
        assert!(everyday_edit(&edit("its", "it's"), &is_word));
        assert!(everyday_edit(&edit("deploy", "deployed"), &is_word));
        assert!(everyday_edit(&edit("affect", "effect"), &is_word));
        assert!(everyday_edit(&edit("10am", "11am"), &is_word));
        // A name is learned, even one that's a word too.
        assert!(!everyday_edit(&edit("Carol", "Karol"), &is_word));
        assert!(!everyday_edit(&edit("doc ploy", "Dokploy"), &is_word));
        assert!(!everyday_edit(&edit("cube cuddle", "kubectl"), &is_word));
    }

    #[test]
    fn gives_up_when_the_paste_is_gone() {
        let pasted = "please review the Netbird config before Friday";
        assert!(find_edits(pasted, "Totally different text in this field now.", 0.18).is_empty());
        assert!(find_edits(pasted, "", 0.18).is_empty());
    }

    #[test]
    fn keeps_one_proposal_per_correction() {
        let pasted = "ask Carol, then tell Carol";
        let field = "ask Karol, then tell Karol";
        assert_eq!(
            words(&find_edits(pasted, field, 0.18)),
            vec![("Carol", "Karol")]
        );
    }

    #[test]
    fn cuts_context_at_character_boundaries() {
        let pasted = "größe of the coolify box";
        let field = "größe of the Coolify box — ünd more text after it that runs on";
        let edits = find_edits(pasted, field, 0.18);
        assert_eq!(words(&edits), vec![("coolify", "Coolify")]);
        assert!(edits[0].context.starts_with("größe of the Coolify"));
        assert!(edits[0].context.ends_with('…'));
    }

    fn said(texts: &[&'static str]) -> Vec<Said<'static>> {
        texts
            .iter()
            .enumerate()
            .map(|(i, text)| Said {
                text,
                at: i as i64,
                app: Some("Claude Code"),
                app_path: None,
            })
            .collect()
    }

    #[test]
    fn finds_a_term_written_three_ways() {
        let found = spelled_differently(&said(&[
            "move the staging app from cool if I to Dokploy",
            "the Coolify dashboard is down again",
            "is coolify still on the old box",
            "Lunch at noon?",
        ]));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].word, "Coolify");
        assert_eq!(found[0].count, 3);
        assert_eq!(found[0].last_at, 2);
        assert!(found[0].forms.contains(&"cool if I".to_string()));
        assert!(found[0].forms.contains(&"coolify".to_string()));
    }

    #[test]
    fn ordinary_pairs_and_one_spelling_are_left_out() {
        let found = spelled_differently(&said(&[
            "I walk every day. Everyday things.",
            "Everyday is fine, every day works",
            "ask Samir about it, Samir knows",
            "and Samir again",
        ]));
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn a_split_form_stays_within_a_phrase() {
        let found = spelled_differently(&said(&[
            "it was cool. If I go, Coolify stays",
            "the Coolify box",
            "Coolify again",
        ]));
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn merges_are_suggested_for_one_person_or_one_spelling() {
        let w = |id: i64, text: &'static str, hits: i64| WordForMerge { id, text, hits };
        let words = [
            w(1, "Abhash", 9),
            w(2, "Abhash Chakraborty", 1),
            w(3, "Dokploy", 2),
            w(4, "DocPloy", 5),
            w(5, "Netbird", 1),
            w(6, "bash", 0),
        ];
        assert_eq!(merge_suggestions(&words, &[]), vec![(2, 1), (4, 3)]);
        let apart = vec![("abhash".to_string(), "abhash chakraborty".to_string())];
        assert_eq!(merge_suggestions(&words, &apart), vec![(4, 3)]);
    }

    #[test]
    fn spelling_keys_fold_near_spellings() {
        assert_eq!(spelling_key("doc ploy"), spelling_key("Dokploy"));
        assert_eq!(spelling_key("cool if I"), spelling_key("Coolify"));
        assert_ne!(spelling_key("Carol"), spelling_key("Coral"));
    }
}
