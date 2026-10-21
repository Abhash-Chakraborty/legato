use natural::phonetics::soundex;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use strsim::levenshtein;

/// Builds an n-gram string by cleaning and concatenating words
///
/// Strips punctuation from each word, lowercases, and joins without spaces.
/// This allows matching "Charge B" against "ChargeBee".
fn build_ngram(words: &[&str]) -> String {
    words
        .iter()
        .map(|w| build_match_key(w))
        .collect::<Vec<_>>()
        .concat()
}

fn build_match_key(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

struct CustomWordMatchKey {
    word_index: usize,
    key: String,
}

fn build_custom_word_match_keys(word: &str, word_index: usize) -> Vec<CustomWordMatchKey> {
    let primary_key = build_match_key(word);
    let mut keys = Vec::with_capacity(2);

    // The fallback matcher is intentionally limited to ASCII terms. Its
    // whitespace tokenization and Soundex scoring are not suitable for CJK
    // scripts. Unicode custom words remain available to models that accept
    // them as native decode prompts; they are simply skipped by this fallback.
    if is_supported_fuzzy_key(&primary_key) {
        keys.push(CustomWordMatchKey {
            word_index,
            key: primary_key.clone(),
        });
    }

    if word.contains('&') {
        let expanded_key = build_match_key(&word.replace('&', " and "));
        if is_supported_fuzzy_key(&expanded_key) && expanded_key != primary_key {
            keys.push(CustomWordMatchKey {
                word_index,
                key: expanded_key,
            });
        }
    }

    keys
}

fn is_supported_fuzzy_key(key: &str) -> bool {
    !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric())
}

fn supports_soundex(key: &str) -> bool {
    !key.is_empty() && key.chars().all(|c| c.is_ascii_alphabetic())
}

/// How far apart two match keys are: Levenshtein distance over the longer
/// length, with a boost when they sound alike. 0.0 is the same.
/// The largest share of letters that may differ for a sound-alike to count
/// as closer than its spelling.
const PHONETIC_BOOST_LIMIT: f64 = 0.34;

/// Words so common that a run made only of them is what the person said, not
/// a name the model misheard: "in one" stays, though it sounds like "Inyon".
const EVERYDAY: &[&str] = &[
    "a", "about", "after", "again", "all", "also", "am", "an", "and", "any", "are", "as", "at",
    "back", "be", "been", "but", "by", "can", "could", "day", "did", "do", "does", "done", "down",
    "even", "for", "from", "get", "go", "going", "good", "got", "had", "has", "have", "he", "her",
    "here", "him", "his", "how", "i", "if", "in", "into", "is", "it", "its", "just", "know", "let",
    "like", "make", "may", "me", "might", "more", "most", "much", "must", "my", "need", "new",
    "no", "not", "now", "of", "off", "oh", "ok", "okay", "on", "one", "only", "or", "our", "out",
    "over", "right", "said", "say", "saying", "see", "she", "should", "so", "some", "than", "that",
    "the", "their", "them", "then", "there", "these", "they", "think", "this", "those", "time",
    "to", "too", "two", "up", "us", "very", "want", "was", "way", "we", "well", "were", "what",
    "when", "where", "which", "who", "why", "will", "with", "would", "yeah", "yes", "you", "your",
];

/// Whether every word of a phrase is an everyday word, so the Dictionary
/// shouldn't take it for one of its own.
pub fn everyday_phrase(phrase: &str) -> bool {
    let mut words = phrase.split_whitespace().map(build_match_key).peekable();
    words.peek().is_some() && words.all(|w| w.is_empty() || EVERYDAY.contains(&w.as_str()))
}

fn match_score(a: &str, b: &str) -> f64 {
    let max_len = a.chars().count().max(b.chars().count()) as f64;
    // Calculate Levenshtein distance (normalized by length)
    let levenshtein_score = if max_len > 0.0 {
        levenshtein(a, b) as f64 / max_len
    } else {
        1.0
    };

    // Soundex is an English/ASCII phonetic algorithm. Numeric terms can
    // still use edit distance, but must not receive a phonetic boost.
    let phonetic_match = supports_soundex(a) && supports_soundex(b) && soundex(a, b);

    // Combine scores: favor phonetic matches, but also consider string
    // similarity. Soundex keeps only a letter and three consonants, so short
    // words that share them can still be far apart ("in one" and "Inyon");
    // the boost goes only to near misses, about one edit in three letters.
    if phonetic_match && levenshtein_score <= PHONETIC_BOOST_LIMIT {
        levenshtein_score * 0.3 // Give significant boost to phonetic matches
    } else {
        levenshtein_score
    }
}

/// The matcher's score for a heard form against a word, ignoring case,
/// spaces and punctuation ("doc ploy" against "Dokploy"). Lower is closer.
/// None when either isn't something the matcher compares.
pub fn word_distance(heard: &str, word: &str) -> Option<f64> {
    let a = build_match_key(heard);
    let b = build_match_key(word);
    let comparable = |k: &str| is_supported_fuzzy_key(k) && k.chars().count() <= 50;
    (comparable(&a) && comparable(&b)).then(|| match_score(&a, &b))
}

/// Finds the best matching custom word for a candidate string
///
/// Uses Levenshtein distance and Soundex phonetic matching to find
/// the best match above the given threshold.
///
/// # Arguments
/// * `candidate` - The cleaned/lowercased candidate string to match
/// * `custom_word_match_keys` - Normalized custom-word keys for comparison
/// * `threshold` - Maximum similarity score to accept
///
/// # Returns
/// The index of the best matching custom word and its score, if any match
/// was found
fn find_best_match(
    candidate: &str,
    custom_word_match_keys: &[CustomWordMatchKey],
    threshold: f64,
) -> Option<(usize, f64)> {
    if !is_supported_fuzzy_key(candidate) || candidate.chars().count() > 50 {
        return None;
    }

    let mut best_match: Option<usize> = None;
    let mut best_score = f64::MAX;

    for custom_word_key in custom_word_match_keys {
        // Skip if lengths are too different (optimization + prevents over-matching)
        // Use percentage-based check: max 25% length difference (prevents n-grams from
        // matching significantly shorter custom words, e.g., "openaigpt" vs "openai")
        let candidate_len = candidate.chars().count();
        let custom_word_len = custom_word_key.key.chars().count();
        let len_diff = candidate_len.abs_diff(custom_word_len) as f64;
        let max_len = candidate_len.max(custom_word_len) as f64;
        let max_allowed_diff = (max_len * 0.25).max(2.0); // At least 2 chars difference allowed
        if len_diff > max_allowed_diff {
            continue;
        }

        let combined_score = match_score(candidate, &custom_word_key.key);

        // Accept if the score is good enough (configurable threshold)
        if combined_score < threshold && combined_score < best_score {
            best_match = Some(custom_word_key.word_index);
            best_score = combined_score;
        }
    }

    best_match.map(|m| (m, best_score))
}

/// Applies custom word corrections to transcribed text using fuzzy matching
///
/// This function corrects words in the input text by finding the best matches
/// from a list of custom words using a combination of:
/// - Levenshtein distance for string similarity
/// - Soundex phonetic matching for pronunciation similarity
/// - N-gram matching for multi-word speech artifacts (e.g., "Charge B" -> "ChargeBee")
///
/// # Arguments
/// * `text` - The input text to correct
/// * `custom_words` - List of custom words to match against
/// * `threshold` - Maximum similarity score to accept (0.0 = exact match, 1.0 = any match)
///
/// # Returns
/// The corrected text with custom words applied
pub fn apply_custom_words(text: &str, custom_words: &[String], threshold: f64) -> String {
    let exact_case = vec![false; custom_words.len()];
    correct_close_words(text, custom_words, &exact_case, &HashSet::new(), threshold).0
}

/// The fuzzy matcher behind [`apply_custom_words`], reporting what it changed
/// as `(heard, word)` pairs. Words at the same index in `exact_case` are
/// pasted exactly as spelled instead of taking the heard word's case, and
/// a word already spelled exactly as one in `protected` is left alone.
fn correct_close_words(
    text: &str,
    custom_words: &[String],
    exact_case: &[bool],
    protected: &HashSet<String>,
    threshold: f64,
) -> (String, Vec<(String, String)>) {
    if custom_words.is_empty() {
        return (text.to_string(), Vec::new());
    }

    // Pre-compute normalized comparison keys to avoid repeated allocations.
    let custom_word_match_keys: Vec<CustomWordMatchKey> = custom_words
        .iter()
        .enumerate()
        .flat_map(|(index, word)| build_custom_word_match_keys(word, index))
        .collect();

    let words: Vec<&str> = text.split_whitespace().collect();
    let mut result = Vec::new();
    let mut changes = Vec::new();
    let mut i = 0;

    while i < words.len() {
        // A word already spelled as one of the user's own stays as it is.
        let (prefix, suffix) = extract_punctuation(words[i]);
        let core = words[i]
            .get(prefix.len()..words[i].len() - suffix.len())
            .unwrap_or(words[i]);
        if protected.contains(core) {
            result.push(words[i].to_string());
            i += 1;
            continue;
        }

        let mut best_match: Option<(usize, usize, f64)> = None;

        // Consider n-grams up to three words and choose the closest match. A
        // longest-first match can consume a following ordinary word when both
        // candidates happen to share a Soundex code (for example,
        // "Charge B, che" matching "ChargeBee").
        for n in (1..=3).rev() {
            if i + n > words.len() {
                continue;
            }

            let ngram_words = &words[i..i + n];
            // Do not consume across a punctuation boundary. In
            // "Charge B, che", the comma closes the candidate at "B,".
            if ngram_words[..n.saturating_sub(1)]
                .iter()
                .any(|word| !extract_punctuation(word).1.is_empty())
            {
                continue;
            }
            if everyday_phrase(&ngram_words.join(" ")) {
                continue;
            }
            let ngram = build_ngram(ngram_words);

            if let Some((index, score)) =
                find_best_match(&ngram, &custom_word_match_keys, threshold)
            {
                let is_better = best_match
                    .as_ref()
                    .is_none_or(|(_, _, best_score)| score < *best_score);
                if is_better {
                    best_match = Some((n, index, score));
                }
            }
        }

        if let Some((n, index, _)) = best_match {
            let ngram_words = &words[i..i + n];
            let replacement = &custom_words[index];
            // Extract punctuation from first and last words of the n-gram.
            let (prefix, _) = extract_punctuation(ngram_words[0]);
            let (_, suffix) = extract_punctuation(ngram_words[n - 1]);

            // Preserve case from first word, unless the word is case sensitive.
            let corrected = if exact_case.get(index).copied().unwrap_or(false) {
                replacement.clone()
            } else {
                preserve_case_pattern(ngram_words[0], replacement)
            };

            let joined = ngram_words.join(" ");
            let heard = joined
                .get(prefix.len()..joined.len() - suffix.len())
                .unwrap_or(&joined);
            if heard != corrected {
                changes.push((heard.to_string(), corrected.clone()));
            }
            result.push(format!("{}{}{}", prefix, corrected, suffix));
            i += n;
        } else {
            result.push(words[i].to_string());
            i += 1;
        }
    }

    (result.join(" "), changes)
}

/// One word in the user's Dictionary, as the corrector sees it.
#[derive(Clone, Debug, Default)]
pub struct DictionaryWord {
    pub text: String,
    /// Ways the model mishears the word, replaced whole-word in any case.
    pub heard: Vec<String>,
    /// Pasted with exactly this capitalisation.
    pub case_sensitive: bool,
    /// Sent to the model in its vocabulary hint.
    pub in_hint: bool,
}

/// A change the Dictionary made to a transcript.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DictionaryCorrection {
    pub from: String,
    pub to: String,
    /// A heard-as form matched, rather than a close miss.
    pub heard: bool,
}

/// The Dictionary prepared for correcting transcripts, with every heard-as
/// form compiled into one pattern, longest first.
#[derive(Default)]
pub struct CompiledDictionary {
    words: Vec<DictionaryWord>,
    heard: Option<Regex>,
    /// Normalised heard form to an index into `words`.
    heard_index: HashMap<String, usize>,
    spelled: HashSet<String>,
}

/// Heard forms compare in lower case with single spaces.
fn heard_key(form: &str) -> String {
    form.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// A form can only be matched as whole words if it starts and ends with a
/// letter or digit.
fn matchable_form(form: &str) -> bool {
    form.chars().next().is_some_and(char::is_alphanumeric)
        && form.chars().last().is_some_and(char::is_alphanumeric)
}

/// An all-lowercase word replacing a capitalised one (at the start of a
/// sentence) takes the capital. A word with capitals of its own, like
/// "iPhone", is always pasted as spelled.
fn capitalise_like(found: &str, word: &str) -> String {
    let opens_upper = found.chars().next().is_some_and(char::is_uppercase);
    if !opens_upper || word.chars().any(char::is_uppercase) {
        return word.to_string();
    }
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

impl CompiledDictionary {
    pub fn new(words: Vec<DictionaryWord>) -> Self {
        let mut heard_index = HashMap::new();
        for (index, word) in words.iter().enumerate() {
            for form in word.heard.iter().filter(|f| matchable_form(f)) {
                heard_index.entry(heard_key(form)).or_insert(index);
            }
            // A case-sensitive word also puts its own capitals right.
            if word.case_sensitive && matchable_form(&word.text) {
                heard_index.entry(heard_key(&word.text)).or_insert(index);
            }
        }
        let mut forms: Vec<&String> = heard_index.keys().collect();
        forms.sort_by(|a, b| {
            b.chars()
                .count()
                .cmp(&a.chars().count())
                .then_with(|| a.cmp(b))
        });
        let heard = if forms.is_empty() {
            None
        } else {
            let alternation = forms
                .iter()
                .map(|form| {
                    form.split(' ')
                        .map(regex::escape)
                        .collect::<Vec<_>>()
                        .join(r"\s+")
                })
                .collect::<Vec<_>>()
                .join("|");
            Regex::new(&format!(r"(?i)\b(?:{alternation})\b")).ok()
        };
        let spelled = words.iter().map(|w| w.text.clone()).collect();
        Self {
            words,
            heard,
            heard_index,
            spelled,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    /// Heard-as forms first, then close misses of the words the model
    /// didn't get in its hint (all of them when `prompted` is false).
    pub fn apply(
        &self,
        text: &str,
        prompted: bool,
        threshold: f64,
    ) -> (String, Vec<DictionaryCorrection>) {
        let mut corrections = Vec::new();
        let text = match &self.heard {
            Some(pattern) => pattern
                .replace_all(text, |caps: &regex::Captures| {
                    let found = &caps[0];
                    let Some(word) = self.heard_index.get(&heard_key(found)) else {
                        return found.to_string();
                    };
                    let word = &self.words[*word];
                    let out = if word.case_sensitive {
                        word.text.clone()
                    } else {
                        capitalise_like(found, &word.text)
                    };
                    if out != found {
                        corrections.push(DictionaryCorrection {
                            from: found.to_string(),
                            to: out.clone(),
                            heard: true,
                        });
                    }
                    out
                })
                .into_owned(),
            None => text.to_string(),
        };

        let candidates: Vec<&DictionaryWord> = self
            .words
            .iter()
            .filter(|w| !(prompted && w.in_hint))
            .collect();
        if candidates.is_empty() {
            return (text, corrections);
        }
        let list: Vec<String> = candidates.iter().map(|w| w.text.clone()).collect();
        let exact: Vec<bool> = candidates.iter().map(|w| w.case_sensitive).collect();
        let (text, changes) = correct_close_words(&text, &list, &exact, &self.spelled, threshold);
        corrections.extend(changes.into_iter().map(|(from, to)| DictionaryCorrection {
            from,
            to,
            heard: false,
        }));
        (text, corrections)
    }
}

/// Preserves the case pattern of the original word when applying a replacement
fn preserve_case_pattern(original: &str, replacement: &str) -> String {
    if original.chars().all(|c| c.is_uppercase()) {
        replacement.to_uppercase()
    } else if original.chars().next().is_some_and(|c| c.is_uppercase()) {
        let mut chars: Vec<char> = replacement.chars().collect();
        if let Some(first_char) = chars.get_mut(0) {
            *first_char = first_char.to_uppercase().next().unwrap_or(*first_char);
        }
        chars.into_iter().collect()
    } else {
        replacement.to_string()
    }
}

/// Extracts punctuation prefix and suffix from a word
fn extract_punctuation(word: &str) -> (&str, &str) {
    // String slices use byte offsets. Derive both boundaries from char_indices
    // so multibyte punctuation such as `。` and `「」` can never be split.
    let prefix_end = word
        .char_indices()
        .find(|(_, c)| c.is_alphanumeric())
        .map(|(index, _)| index)
        .unwrap_or(word.len());
    let suffix_start = word
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_alphanumeric())
        .map(|(index, c)| index + c.len_utf8())
        .unwrap_or(0);

    let prefix = if prefix_end > 0 {
        &word[..prefix_end]
    } else {
        ""
    };

    let suffix = if suffix_start < word.len() {
        &word[suffix_start..]
    } else {
        ""
    };

    (prefix, suffix)
}

/// Evidence for the language of the text being cleaned.
///
/// This intentionally describes the transcription output, not Legato's UI
/// language. Unknown output languages fail closed: built-in filler removal is
/// skipped rather than applying a language profile speculatively.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutputLanguageEvidence {
    UserSelected(String),
    ModelConstrained(String),
    /// The transcription model itself identified the language (audio-based
    /// LID, e.g. Whisper in auto mode).
    ModelDetected(String),
    /// Detected from the transcribed text with high confidence, constrained to
    /// the model's supported languages. Weakest accepted evidence.
    TextDetected(String),
    TranslatedToEnglish,
    Unknown,
}

impl OutputLanguageEvidence {
    pub(crate) fn language(&self) -> Option<&str> {
        match self {
            Self::UserSelected(language)
            | Self::ModelConstrained(language)
            | Self::ModelDetected(language)
            | Self::TextDetected(language) => Some(language),
            Self::TranslatedToEnglish => Some("en"),
            Self::Unknown => None,
        }
    }
}

/// Filler tokens that are not lexical words in any language Legato's models can
/// output, so removing them cannot corrupt text regardless of the (possibly
/// unknown) output language. Kept deliberately conservative: anything that is a
/// real word somewhere ("um" pt/de, "ha" es, "ah"/"eh" interjections, "mm"
/// millimetres) belongs in the language-gated lists instead.
const UNIVERSAL_FILLER_WORDS: &[&str] = &[
    "uh", "uhm", "umm", "uhh", "uhhh", "ehh", "ehm", "ahm", "hmm", "hm", "mmm", "хм", "ммм",
];

/// Filler words that are only safe to remove with evidence for the output
/// language, because the same token is a real word elsewhere (e.g. Portuguese
/// "um" = "a/an", German "um" = "at/around", Spanish "ha" = "has").
fn gated_filler_words_for_language(lang: &str) -> &'static [&'static str] {
    let base_lang = lang.split(&['-', '_'][..]).next().unwrap_or(lang);

    match base_lang {
        "en" => &["um", "ah", "eh"],
        "de" => &["äh", "ähm"],
        "fr" => &["euh"],
        _ => &[],
    }
}

static MULTI_SPACE_PATTERN: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s{2,}").unwrap());

/// Collapses repeated words (3+ repetitions) to a single instance.
/// E.g., "wh wh wh wh" -> "wh", "I I I I" -> "I"
fn collapse_stutters(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return text.to_string();
    }

    let mut result: Vec<&str> = Vec::new();
    let mut i = 0;

    while i < words.len() {
        let word = words[i];
        let word_lower = word.to_lowercase();

        if word_lower.chars().all(|c| c.is_alphabetic()) {
            // Count consecutive repetitions (case-insensitive)
            let mut count = 1;
            while i + count < words.len() && words[i + count].to_lowercase() == word_lower {
                count += 1;
            }

            // If 3+ repetitions, collapse to single instance
            if count >= 3 {
                result.push(word);
                i += count;
            } else {
                result.push(word);
                i += 1;
            }
        } else {
            result.push(word);
            i += 1;
        }
    }

    result.join(" ")
}

/// Whether a word appended to `kept` would open a sentence: nothing but
/// whitespace so far, or the last visible character ends a sentence.
fn opens_sentence(kept: &str) -> bool {
    kept.trim_end()
        .chars()
        .next_back()
        .is_none_or(|c| matches!(c, '.' | '!' | '?' | '…'))
}

/// Appends `segment` to `kept`. While `capital_owed` is set, the first
/// alphanumeric character of `segment` is uppercased and the debt is settled.
fn push_restoring_capital(kept: &mut String, segment: &str, capital_owed: &mut bool) {
    if *capital_owed {
        if let Some((index, first)) = segment.char_indices().find(|(_, c)| c.is_alphanumeric()) {
            *capital_owed = false;
            kept.push_str(&segment[..index]);
            kept.extend(first.to_uppercase());
            kept.push_str(&segment[index + first.len_utf8()..]);
            return;
        }
    }
    kept.push_str(segment);
}

/// Deletes every match of one filler pattern. A capitalized filler that opened
/// a sentence hands its capital to the word that takes its place, so
/// "Um, so I think" becomes "So I think" rather than "so I think".
fn remove_filler_matches(text: &str, pattern: &Regex) -> String {
    let mut kept = String::with_capacity(text.len());
    let mut resume = 0;
    let mut capital_owed = false;

    for filler in pattern.find_iter(text) {
        push_restoring_capital(&mut kept, &text[resume..filler.start()], &mut capital_owed);
        let capitalized = filler.as_str().starts_with(char::is_uppercase);
        capital_owed |= capitalized && opens_sentence(&kept);
        resume = filler.end();
    }
    push_restoring_capital(&mut kept, &text[resume..], &mut capital_owed);

    kept
}

/// Removes filler words from transcription output when enabled.
///
/// Built-in removal is two-tiered: [`UNIVERSAL_FILLER_WORDS`] apply regardless
/// of language evidence, while [`gated_filler_words_for_language`] tokens are
/// only removed when the output language is known. A custom list is an
/// explicit user override and replaces both tiers without requiring language
/// evidence. `Some(empty vec)` disables removal, preserving the legacy
/// power-user setting. The master toggle takes precedence over both built-in
/// and custom lists.
///
/// # Arguments
/// * `text` - The raw transcription text to filter
/// * `language` - Evidence for the language of the transcription output
/// * `custom_filler_words` - Optional user-provided filler word list. `Some(vec)` overrides
///   language defaults; `Some(empty vec)` disables filtering; `None` uses language defaults.
/// * `enabled` - Whether filler-word removal is enabled
///
/// # Returns
/// The text with configured filler words removed
pub fn remove_filler_words(
    text: &str,
    language: &OutputLanguageEvidence,
    custom_filler_words: &Option<Vec<String>>,
    enabled: bool,
) -> String {
    remove_filler_words_counted(text, language, custom_filler_words, enabled).0
}

/// [`remove_filler_words`], also returning each filler it removed, in lower
/// case, with how many times, for Insights.
pub fn remove_filler_words_counted(
    text: &str,
    language: &OutputLanguageEvidence,
    custom_filler_words: &Option<Vec<String>>,
    enabled: bool,
) -> (String, Vec<(String, i64)>) {
    if !enabled {
        return (text.to_string(), Vec::new());
    }

    // Build filler patterns from custom list or the built-in tiers
    let words: Vec<String> = match custom_filler_words {
        Some(words) => words.clone(),
        None => UNIVERSAL_FILLER_WORDS
            .iter()
            .chain(
                language
                    .language()
                    .map(gated_filler_words_for_language)
                    .unwrap_or_default(),
            )
            .map(|word| word.to_string())
            .collect(),
    };

    // Remove filler words
    let mut filtered = text.to_string();
    let mut removed = Vec::new();
    for word in &words {
        let Ok(pattern) = Regex::new(&format!(r"(?i)\b{}\b[,.]?", regex::escape(word))) else {
            continue;
        };
        let count = pattern.find_iter(&filtered).count();
        if count > 0 {
            filtered = remove_filler_matches(&filtered, &pattern);
            removed.push((word.to_lowercase(), count as i64));
        }
    }

    (filtered, removed)
}

/// Applies non-filler transcription cleanup.
///
/// Kept separate from [`remove_filler_words`] so disabling filler deletion
/// does not also disable the existing repeated-word and whitespace cleanup.
pub fn normalize_transcription_output(text: &str) -> String {
    let mut normalized = collapse_stutters(text);

    // Clean up multiple spaces to single space
    normalized = MULTI_SPACE_PATTERN
        .replace_all(&normalized, " ")
        .to_string();

    // Trim leading/trailing whitespace
    normalized.trim().to_string()
}

#[cfg(test)]
mod tests {
    #[test]
    fn removed_fillers_are_counted() {
        let (text, removed) = remove_filler_words_counted(
            "Uh, so um the build, um, is green",
            &OutputLanguageEvidence::Unknown,
            &Some(vec!["um".into(), "uh".into(), "er".into()]),
            true,
        );
        assert!(!text.to_lowercase().contains("um"));
        assert_eq!(removed, vec![("um".to_string(), 2), ("uh".to_string(), 1)]);
        let (_, none) = remove_filler_words_counted(
            "um yes",
            &OutputLanguageEvidence::Unknown,
            &Some(vec!["um".into()]),
            false,
        );
        assert!(none.is_empty());
    }

    use super::*;

    /// Exercise the complete cleanup sequence with an explicitly selected
    /// language. Individual tests below predate the split between filler
    /// removal and non-filler normalization.
    fn filter_transcription_output(
        text: &str,
        language: &str,
        custom_filler_words: &Option<Vec<String>>,
    ) -> String {
        let language = OutputLanguageEvidence::UserSelected(language.to_string());
        let filtered = remove_filler_words(text, &language, custom_filler_words, true);
        normalize_transcription_output(&filtered)
    }

    #[test]
    fn everyday_words_are_not_taken_for_names() {
        let words = vec!["Inyon".to_string(), "Netbird".to_string()];
        for said in [
            "we can do it in one go",
            "let me fill you in on it",
            "I am saying the errors are back",
            "this is not bad",
        ] {
            assert_eq!(apply_custom_words(said, &words, 0.18), said);
        }
        assert_eq!(
            apply_custom_words("the netboard tunnel is down", &words, 0.18),
            "the Netbird tunnel is down"
        );
        assert_eq!(
            apply_custom_words("open in yon now", &words, 0.18),
            "open Inyon now"
        );
    }

    #[test]
    fn test_apply_custom_words_exact_match() {
        let text = "hello world";
        let custom_words = vec!["Hello".to_string(), "World".to_string()];
        let result = apply_custom_words(text, &custom_words, 0.5);
        assert_eq!(result, "Hello World");
    }

    #[test]
    fn test_apply_custom_words_fuzzy_match() {
        let text = "helo wrold";
        let custom_words = vec!["hello".to_string(), "world".to_string()];
        let result = apply_custom_words(text, &custom_words, 0.5);
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_preserve_case_pattern() {
        assert_eq!(preserve_case_pattern("HELLO", "world"), "WORLD");
        assert_eq!(preserve_case_pattern("Hello", "world"), "World");
        assert_eq!(preserve_case_pattern("hello", "WORLD"), "WORLD");
    }

    #[test]
    fn test_extract_punctuation() {
        assert_eq!(extract_punctuation("hello"), ("", ""));
        assert_eq!(extract_punctuation("!hello?"), ("!", "?"));
        assert_eq!(extract_punctuation("...hello..."), ("...", "..."));
    }

    #[test]
    fn test_extract_punctuation_uses_unicode_boundaries() {
        assert_eq!(extract_punctuation("你好。"), ("", "。"));
        assert_eq!(extract_punctuation("「你好」"), ("「", "」"));
        assert_eq!(extract_punctuation("你好！"), ("", "！"));
    }

    #[test]
    fn test_empty_custom_words() {
        let text = "hello world";
        let custom_words = vec![];
        let result = apply_custom_words(text, &custom_words, 0.5);
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_filter_filler_words() {
        let text = "So uhm I was thinking uh about this";
        let result = filter_transcription_output(text, "en", &None);
        assert_eq!(result, "So I was thinking about this");
    }

    #[test]
    fn test_filter_filler_words_case_insensitive() {
        let text = "UHM this is UH a test";
        let result = filter_transcription_output(text, "en", &None);
        assert_eq!(result, "This is a test");
    }

    #[test]
    fn test_filter_filler_words_with_punctuation() {
        let text = "Well, uhm, I think, uh. that's right";
        let result = filter_transcription_output(text, "en", &None);
        assert_eq!(result, "Well, I think, that's right");
    }

    #[test]
    fn test_filter_cleans_whitespace() {
        let text = "Hello    world   test";
        let result = filter_transcription_output(text, "en", &None);
        assert_eq!(result, "Hello world test");
    }

    #[test]
    fn test_filter_trims() {
        let text = "  Hello world  ";
        let result = filter_transcription_output(text, "en", &None);
        assert_eq!(result, "Hello world");
    }

    #[test]
    fn test_filter_combined() {
        let text = "  Uhm, so I was, uh, thinking about this  ";
        let result = filter_transcription_output(text, "en", &None);
        assert_eq!(result, "So I was, thinking about this");
    }

    #[test]
    fn test_filter_leading_filler_keeps_sentence_capital() {
        let result = filter_transcription_output("Um, so I think we should ship it.", "en", &None);
        assert_eq!(result, "So I think we should ship it.");

        let result = filter_transcription_output("That works. Um, let me check.", "en", &None);
        assert_eq!(result, "That works. Let me check.");

        // Mid-sentence there is no capital to hand over.
        let result = filter_transcription_output("He said, Um, not today.", "en", &None);
        assert_eq!(result, "He said, not today.");
    }

    #[test]
    fn test_filter_preserves_valid_text() {
        let text = "This is a completely normal sentence.";
        let result = filter_transcription_output(text, "en", &None);
        assert_eq!(result, "This is a completely normal sentence.");
    }

    #[test]
    fn test_filter_stutter_collapse() {
        let text = "w wh wh wh wh wh wh wh wh wh why";
        let result = filter_transcription_output(text, "en", &None);
        assert_eq!(result, "w wh why");
    }

    #[test]
    fn test_filter_stutter_short_words() {
        let text = "I I I I think so so so so";
        let result = filter_transcription_output(text, "en", &None);
        assert_eq!(result, "I think so");
    }

    #[test]
    fn test_filter_stutter_longer_words() {
        let text = "Check data doc doc doc doc documentation.";
        let result = filter_transcription_output(text, "en", &None);
        assert_eq!(result, "Check data doc documentation.");
    }

    #[test]
    fn test_filter_stutter_mixed_case() {
        let text = "No NO no NO no";
        let result = filter_transcription_output(text, "en", &None);
        assert_eq!(result, "No");
    }

    #[test]
    fn test_filter_stutter_preserves_two_repetitions() {
        let text = "no no is fine";
        let result = filter_transcription_output(text, "en", &None);
        assert_eq!(result, "no no is fine");
    }

    #[test]
    fn test_filter_english_removes_um() {
        let text = "um I think um this is good";
        let result = filter_transcription_output(text, "en", &None);
        assert_eq!(result, "I think this is good");
    }

    #[test]
    fn test_filter_portuguese_preserves_um() {
        // "um" means "a/an" in Portuguese
        let text = "um gato bonito";
        let result = filter_transcription_output(text, "pt", &None);
        assert_eq!(result, "um gato bonito");
    }

    #[test]
    fn test_filter_spanish_preserves_ha() {
        // "ha" means "has" in Spanish
        let text = "ha sido un buen día";
        let result = filter_transcription_output(text, "es", &None);
        assert_eq!(result, "ha sido un buen día");
    }

    #[test]
    fn test_filter_language_code_with_region() {
        // "pt-BR" should normalize to "pt"
        let text = "um gato bonito";
        let result = filter_transcription_output(text, "pt-BR", &None);
        assert_eq!(result, "um gato bonito");
    }

    #[test]
    fn test_filter_custom_filler_words_override() {
        let custom = Some(vec!["okay".to_string(), "right".to_string()]);
        let text = "okay so I think right this works";
        let result = filter_transcription_output(text, "en", &custom);
        assert_eq!(result, "so I think this works");
    }

    #[test]
    fn test_filter_custom_filler_words_empty_disables() {
        let custom = Some(vec![]);
        let text = "So uhm I was thinking uh about this";
        let result = filter_transcription_output(text, "en", &custom);
        // No filler words removed since custom list is empty
        assert_eq!(result, "So uhm I was thinking uh about this");
    }

    #[test]
    fn test_filter_unknown_language_still_removes_universal_fillers() {
        let text = "uh I think uhm this works";
        let result = filter_transcription_output(text, "xx", &None);
        assert_eq!(result, "I think this works");
    }

    #[test]
    fn test_filter_unknown_language_does_not_remove_um() {
        let text = "um I think this works";
        let result = filter_transcription_output(text, "xx", &None);
        assert_eq!(result, "um I think this works");
    }

    #[test]
    fn test_filter_unknown_evidence_removes_universal_keeps_gated() {
        let filtered = remove_filler_words(
            "uhh bueno hmm creo que um ha llegado",
            &OutputLanguageEvidence::Unknown,
            &None,
            true,
        );
        assert_eq!(
            normalize_transcription_output(&filtered),
            "bueno creo que um ha llegado"
        );

        let cyrillic = remove_filler_words(
            "хм я думаю ммм это работает",
            &OutputLanguageEvidence::Unknown,
            &None,
            true,
        );
        assert_eq!(
            normalize_transcription_output(&cyrillic),
            "я думаю это работает"
        );
    }

    #[test]
    fn test_filter_german_gated_fillers_require_evidence() {
        let text = "äh ich glaube ähm das passt";

        let unknown = remove_filler_words(text, &OutputLanguageEvidence::Unknown, &None, true);
        assert_eq!(normalize_transcription_output(&unknown), text);

        let result = filter_transcription_output(text, "de", &None);
        assert_eq!(result, "ich glaube das passt");
    }

    #[test]
    fn test_filter_preserves_millimetre_unit() {
        // "mm" was removed from the filler lists because it eats units.
        let text = "the screw is 5 mm long";
        let result = filter_transcription_output(text, "en", &None);
        assert_eq!(result, "the screw is 5 mm long");
    }

    #[test]
    fn test_filter_detected_evidence_unlocks_gated_fillers() {
        let model = remove_filler_words(
            "um I think this works",
            &OutputLanguageEvidence::ModelDetected("en".to_string()),
            &None,
            true,
        );
        assert_eq!(normalize_transcription_output(&model), "I think this works");

        let text = remove_filler_words(
            "euh je pense que ça marche",
            &OutputLanguageEvidence::TextDetected("fr".to_string()),
            &None,
            true,
        );
        assert_eq!(
            normalize_transcription_output(&text),
            "je pense que ça marche"
        );
    }

    #[test]
    fn test_filter_master_toggle_disables_custom_and_builtin_removal() {
        let text = "um customword I think";
        let language = OutputLanguageEvidence::UserSelected("en".to_string());
        let custom = Some(vec!["customword".to_string()]);

        let result = remove_filler_words(text, &language, &custom, false);

        assert_eq!(result, text);
    }

    #[test]
    fn test_filter_custom_words_apply_without_language_evidence() {
        let custom = Some(vec!["customword".to_string()]);
        let text = "customword should be removed but um should remain";

        let filtered = remove_filler_words(text, &OutputLanguageEvidence::Unknown, &custom, true);
        let result = normalize_transcription_output(&filtered);

        assert_eq!(result, "should be removed but um should remain");
    }

    #[test]
    fn test_apply_custom_words_ngram_two_words() {
        let text = "il cui nome è Charge B, che permette";
        let custom_words = vec!["ChargeBee".to_string()];
        let result = apply_custom_words(text, &custom_words, 0.5);
        assert!(result.contains("ChargeBee,"), "unexpected result: {result}");
        assert!(!result.contains("Charge B"));
    }

    #[test]
    fn test_apply_custom_words_ngram_three_words() {
        let text = "use Chat G P T for this";
        let custom_words = vec!["ChatGPT".to_string()];
        let result = apply_custom_words(text, &custom_words, 0.5);
        assert!(result.contains("ChatGPT"));
    }

    #[test]
    fn test_apply_custom_words_prefers_longer_ngram() {
        let text = "Open AI GPT model";
        let custom_words = vec!["OpenAI".to_string(), "GPT".to_string()];
        let result = apply_custom_words(text, &custom_words, 0.5);
        assert_eq!(result, "OpenAI GPT model");
    }

    #[test]
    fn test_apply_custom_words_ngram_preserves_case() {
        let text = "CHARGE B is great";
        let custom_words = vec!["ChargeBee".to_string()];
        let result = apply_custom_words(text, &custom_words, 0.5);
        assert!(result.contains("CHARGEBEE"));
    }

    #[test]
    fn test_apply_custom_words_ngram_with_spaces_in_custom() {
        // Custom word with space should also match against split words
        let text = "using Mac Book Pro";
        let custom_words = vec!["MacBook Pro".to_string()];
        let result = apply_custom_words(text, &custom_words, 0.5);
        assert_eq!(result, "using MacBook Pro");
    }

    #[test]
    fn test_apply_custom_words_trailing_number_not_doubled() {
        // Verify that trailing non-alpha chars (like numbers) aren't double-counted
        // between build_ngram stripping them and extract_punctuation capturing them
        let text = "use GPT4 for this";
        let custom_words = vec!["GPT-4".to_string()];
        let result = apply_custom_words(text, &custom_words, 0.5);
        // Should NOT produce "GPT-44" (double-counting the trailing 4)
        assert!(
            !result.contains("GPT-44"),
            "got double-counted result: {}",
            result
        );
    }

    #[test]
    fn test_apply_custom_words_matches_ampersand_word() {
        let text = "send it to RD for review";
        let custom_words = vec!["R&D".to_string()];
        let result = apply_custom_words(text, &custom_words, 0.18);
        assert_eq!(result, "send it to R&D for review");
    }

    #[test]
    fn test_apply_custom_words_matches_spoken_ampersand_word() {
        let text = "send it to R and D for review";
        let custom_words = vec!["R&D".to_string()];
        let result = apply_custom_words(text, &custom_words, 0.18);
        assert_eq!(result, "send it to R&D for review");
    }

    #[test]
    fn test_apply_custom_words_preserves_ampersand_word() {
        let text = "send it to R&D for review";
        let custom_words = vec!["R&D".to_string()];
        let result = apply_custom_words(text, &custom_words, 0.18);
        assert_eq!(result, "send it to R&D for review");
    }

    #[test]
    fn test_apply_custom_words_handles_unicode_punctuation() {
        let text = "「Handee。」";
        let custom_words = vec!["Handy".to_string()];
        let result = apply_custom_words(text, &custom_words, 0.5);
        assert_eq!(result, "「Handy。」");
    }

    #[test]
    fn test_apply_custom_words_skips_cjk_fuzzy_matching() {
        let text = "你好。";
        let custom_words = vec!["你号".to_string()];
        let result = apply_custom_words(text, &custom_words, 1.0);
        assert_eq!(result, text);
    }

    fn word(text: &str, heard: &[&str]) -> DictionaryWord {
        DictionaryWord {
            text: text.to_string(),
            heard: heard.iter().map(|h| h.to_string()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn dictionary_replaces_heard_forms_in_any_case() {
        let dict = CompiledDictionary::new(vec![
            word("Dokploy", &["doc ploy"]),
            word("Karol", &["carol"]),
        ]);
        let (text, fixes) = dict.apply("Restart Doc  ploy, then ask carol.", true, 0.18);
        assert_eq!(text, "Restart Dokploy, then ask Karol.");
        assert_eq!(
            fixes,
            vec![
                DictionaryCorrection {
                    from: "Doc  ploy".into(),
                    to: "Dokploy".into(),
                    heard: true
                },
                DictionaryCorrection {
                    from: "carol".into(),
                    to: "Karol".into(),
                    heard: true
                },
            ]
        );
    }

    #[test]
    fn dictionary_heard_forms_match_whole_words_only() {
        let dict = CompiledDictionary::new(vec![word("Karol", &["carol"])]);
        let (text, fixes) = dict.apply("Christmas carols", true, 0.18);
        assert_eq!(text, "Christmas carols");
        assert!(fixes.is_empty());
    }

    #[test]
    fn dictionary_prefers_the_longest_heard_form() {
        let dict =
            CompiledDictionary::new(vec![word("Net", &["net"]), word("Netbird", &["net bird"])]);
        let (text, _) = dict.apply("open net bird now", true, 0.18);
        assert_eq!(text, "open Netbird now");
    }

    #[test]
    fn dictionary_keeps_a_sentence_capital() {
        let dict = CompiledDictionary::new(vec![
            word("iPhone", &["eye phone"]),
            word("legato", &["le gato"]),
        ]);
        assert_eq!(
            dict.apply("Eye phone is here", true, 0.18).0,
            "iPhone is here"
        );
        assert_eq!(dict.apply("Le gato works", true, 0.18).0, "Legato works");
    }

    #[test]
    fn dictionary_case_sensitive_words_fix_their_own_capitals() {
        let mut gpu = word("GPU", &[]);
        gpu.case_sensitive = true;
        let dict = CompiledDictionary::new(vec![gpu]);
        let (text, fixes) = dict.apply("the Gpu is fast", true, 0.18);
        assert_eq!(text, "the GPU is fast");
        assert_eq!(fixes.len(), 1);
    }

    #[test]
    fn dictionary_fuzzy_matches_only_words_outside_the_hint() {
        let mut in_hint = word("Dokploy", &[]);
        in_hint.in_hint = true;
        let dict = CompiledDictionary::new(vec![in_hint, word("Coolify", &[])]);
        // Prompted: Dokploy was in the hint, so the model's spelling stands.
        let (text, fixes) = dict.apply("deploy dokploi with coolifi", true, 0.18);
        assert_eq!(text, "deploy dokploi with Coolify");
        assert_eq!(
            fixes,
            vec![DictionaryCorrection {
                from: "coolifi".into(),
                to: "Coolify".into(),
                heard: false
            }]
        );
        // Not prompted: every word is matched.
        let (text, _) = dict.apply("deploy dokploi with coolifi", false, 0.18);
        assert_eq!(text, "deploy Dokploy with Coolify");
    }

    #[test]
    fn dictionary_leaves_correct_words_and_unrelated_text_alone() {
        let dict = CompiledDictionary::new(vec![word("Karol", &[]), word("Carol", &[])]);
        let (text, fixes) = dict.apply("Carol and Karol met.", false, 0.18);
        assert_eq!(text, "Carol and Karol met.");
        assert!(fixes.is_empty());
        let empty = CompiledDictionary::default();
        assert_eq!(
            empty.apply("keep  this\nas is", false, 0.18).0,
            "keep  this\nas is"
        );
    }
}
