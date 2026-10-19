//! Learn from my edits: the words someone corrected in a field right after
//! Legato pasted into it.

use crate::audio_toolkit::text::word_distance;

/// A correction is deliberate, so it passes at Loose matching even when the
/// Dictionary matches more strictly.
pub const MIN_THRESHOLD: f64 = 0.28;
/// Characters of the field kept on each side of a proposed word.
const CONTEXT_SIDE: usize = 30;
/// Share of the pasted words the field must still hold, or the paste is
/// taken as gone (deleted, or the field is another one).
const MIN_KEPT: f64 = 0.6;
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
    let mut j = (0..=n).min_by_key(|&j| cost[m * w + j]).unwrap_or(0);
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
fn context(field: &str, start: usize, end: usize) -> String {
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
    if (kept as f64) < MIN_KEPT * p.len() as f64 || kept == p.len() && ops.len() == p.len() {
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
}
