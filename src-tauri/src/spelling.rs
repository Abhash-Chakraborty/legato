//! Whether a word is an ordinary word, by Windows' own spell checker, in
//! English and the language Windows is set to. The Dictionary uses it so a
//! heard form that's a real word ("Neon" for "Inyon") only changes in a
//! sentence about the Dictionary word, and so close misses never swallow a
//! real word.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::audio_toolkit::text::IsWord;

/// Answers so far, by lower-case word; the same few words come up often.
static ANSWERS: Mutex<Option<HashMap<String, bool>>> = Mutex::new(None);
const MAX_ANSWERS: usize = 5000;

/// The check, for the corrector.
pub fn checker() -> IsWord {
    Arc::new(is_word)
}

/// Whether Windows' spell checker knows `word` in any of its languages. An
/// unknown answer (no spell checker) counts as not a word, which keeps the
/// Dictionary working as it did before.
pub fn is_word(word: &str) -> bool {
    let word = word.trim();
    if word.is_empty() || word.contains(char::is_whitespace) {
        return false;
    }
    let key = word.to_lowercase();
    if let Some(known) = ANSWERS
        .lock()
        .ok()
        .and_then(|a| a.as_ref().and_then(|m| m.get(&key).copied()))
    {
        return known;
    }
    // Lower case, so a name-like capital doesn't decide it.
    let known = win::check(&key);
    if let Ok(mut answers) = ANSWERS.lock() {
        let map = answers.get_or_insert_with(HashMap::new);
        if map.len() >= MAX_ANSWERS {
            map.clear();
        }
        map.insert(key, known);
    }
    known
}

mod win {
    use super::RefCell;
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::Globalization::{
        GetUserDefaultLocaleName, ISpellChecker, ISpellCheckerFactory, SpellCheckerFactory,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
    };

    thread_local! {
        /// This thread's spell checkers, made on first use.
        static CHECKERS: RefCell<Option<Vec<ISpellChecker>>> = const { RefCell::new(None) };
    }

    fn languages() -> Vec<String> {
        let mut out = vec!["en-US".to_string()];
        let mut buf = [0u16; 85];
        let len = unsafe { GetUserDefaultLocaleName(&mut buf) };
        if len > 1 {
            let tag = String::from_utf16_lossy(&buf[..len as usize - 1]);
            if !out.contains(&tag) {
                out.push(tag);
            }
        }
        out
    }

    fn make() -> Vec<ISpellChecker> {
        unsafe {
            // Already set up on this thread is fine either way.
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let Ok(factory) = CoCreateInstance::<_, ISpellCheckerFactory>(
                &SpellCheckerFactory,
                None,
                CLSCTX_INPROC_SERVER,
            ) else {
                log::debug!("No Windows spell checker; every word counts as unknown");
                return Vec::new();
            };
            languages()
                .into_iter()
                .filter_map(|tag| {
                    let tag = HSTRING::from(tag);
                    let supported = factory.IsSupported(PCWSTR(tag.as_ptr())).ok()?;
                    if !supported.as_bool() {
                        return None;
                    }
                    factory.CreateSpellChecker(PCWSTR(tag.as_ptr())).ok()
                })
                .collect()
        }
    }

    pub fn check(word: &str) -> bool {
        CHECKERS.with(|cell| {
            let mut cell = cell.borrow_mut();
            let checkers = cell.get_or_insert_with(make);
            let text = HSTRING::from(word);
            checkers.iter().any(|checker| unsafe {
                let Ok(errors) = checker.Check(PCWSTR(text.as_ptr())) else {
                    return false;
                };
                let mut error = None;
                // S_FALSE with nothing: no spelling errors.
                let _ = errors.Next(&mut error);
                error.is_none()
            })
        })
    }
}
