//! Commands behind the Style page: cleanup, tones, paste settings per kind of
//! app, and Edit apps.

use crate::audio_toolkit::text::{
    normalize_transcription_output, remove_filler_words, OutputLanguageEvidence,
};
use crate::cleanup_rules::{self, CleanupLevel, CleanupRules};
use crate::context::{self, AppCategory, AppRule, AppRuleKind, ForegroundWindow};
use crate::managers::history::HistoryManager;
use crate::managers::snippet_store;
use crate::managers::snippets::SnippetManager;
use crate::settings::{get_settings, write_settings, CategoryStyle, Tone};
use crate::snippet_rules;
use crate::style_rules;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, State};

/// The fillers a custom list starts from when the built-in one is edited.
const DEFAULT_FILLERS: &[&str] = &["um", "uh", "uhm", "umm", "ah", "eh", "hmm"];

/// An app or site on Edit apps.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct StyleApp {
    pub name: String,
    /// Empty for a website added by name.
    pub exe_path: String,
    pub exe: String,
    pub site: Option<String>,
    pub category: AppCategory,
    /// Words dictated into it.
    pub words: i64,
    pub last_at: Option<i64>,
    /// The rule that moves it, and whether one is saved.
    pub rule: AppRule,
    pub moved: bool,
}

fn style_app(
    info: &context::AppInfo,
    rules: &[AppRule],
    words: i64,
    last_at: Option<i64>,
) -> StyleApp {
    let rule = context::rule_for(info, info.category);
    let moved = rules
        .iter()
        .any(|r| r.kind == rule.kind && r.pattern == rule.pattern);
    StyleApp {
        name: info.site.clone().unwrap_or_else(|| info.name.clone()),
        exe_path: info.exe_path.clone(),
        exe: info.exe.clone(),
        site: info.site.clone(),
        category: info.category,
        words,
        last_at,
        rule,
        moved,
    }
}

/// Apps and sites dictated into, plus websites added by name, each with the
/// kind of app it counts as now.
#[tauri::command]
#[specta::specta]
pub fn get_style_apps(
    app: AppHandle,
    history: State<'_, Arc<HistoryManager>>,
) -> Result<Vec<StyleApp>, String> {
    let rules = get_settings(&app).app_rules;
    let seen = history.seen_apps(80).map_err(|e| e.to_string())?;
    let mut out: Vec<StyleApp> = Vec::new();
    for row in seen {
        let info = context::resolve(&ForegroundWindow {
            exe_path: row.exe_path,
            title: row.title,
        });
        let item = style_app(&info, &rules, row.words, Some(row.last_at));
        if let Some(existing) = out
            .iter_mut()
            .find(|a| a.rule.kind == item.rule.kind && a.rule.pattern == item.rule.pattern)
        {
            existing.words += item.words;
            continue;
        }
        out.push(item);
    }
    for rule in &rules {
        if out
            .iter()
            .any(|a| a.rule.kind == rule.kind && a.rule.pattern == rule.pattern)
        {
            continue;
        }
        out.push(StyleApp {
            name: rule.name.clone(),
            exe_path: String::new(),
            exe: if rule.kind == AppRuleKind::App {
                rule.pattern.clone()
            } else {
                String::new()
            },
            site: (rule.kind == AppRuleKind::Site).then(|| rule.name.clone()),
            category: rule.category,
            words: 0,
            last_at: None,
            rule: rule.clone(),
            moved: true,
        });
    }
    Ok(out)
}

fn save_rules(app: &AppHandle, rules: Vec<AppRule>) -> Vec<AppRule> {
    let mut settings = get_settings(app);
    settings.app_rules = rules.clone();
    write_settings(app, settings);
    context::set_rules(rules.clone());
    rules
}

/// Moves an app or site to a kind of app. Moving it back to where it is
/// built in removes the rule.
#[tauri::command]
#[specta::specta]
pub fn move_app(app: AppHandle, rule: AppRule, category: AppCategory) -> Vec<AppRule> {
    let mut rules = get_settings(&app).app_rules;
    rules.retain(|r| !(r.kind == rule.kind && r.pattern == rule.pattern));
    let moved = AppRule { category, ..rule };
    if context::builtin_category(&moved) != category {
        rules.push(moved);
    }
    save_rules(&app, rules)
}

/// Adds a website by text in its tab title ("Linear", "Jira").
#[tauri::command]
#[specta::specta]
pub fn add_site(
    app: AppHandle,
    name: String,
    category: AppCategory,
) -> Result<Vec<AppRule>, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("Type the site's name as its tab title shows it.".into());
    }
    Ok(move_app(
        app,
        AppRule {
            kind: AppRuleKind::Site,
            pattern: name.to_lowercase(),
            name,
            category,
        },
        category,
    ))
}

/// Removes the moves into and out of one kind of app.
#[tauri::command]
#[specta::specta]
pub fn restore_app_defaults(app: AppHandle, category: AppCategory) -> Vec<AppRule> {
    let mut rules = get_settings(&app).app_rules;
    rules.retain(|r| r.category != category && context::builtin_category(r) != category);
    save_rules(&app, rules)
}

/// Waits, then reads the window in front: Edit apps' Pick a window. None
/// when it's Legato itself or nothing.
#[tauri::command]
#[specta::specta]
pub async fn pick_window(app: AppHandle, delay_ms: u32) -> Option<StyleApp> {
    tokio::time::sleep(Duration::from_millis(u64::from(delay_ms.min(10_000)))).await;
    let window = context::foreground_window()?;
    let own = std::env::current_exe()
        .ok()
        .map(|p| p.to_string_lossy().to_lowercase());
    if own.as_deref() == Some(window.exe_path.to_lowercase().as_str()) {
        return None;
    }
    let info = context::resolve(&window);
    Some(style_app(&info, &get_settings(&app).app_rules, 0, None))
}

/// What Legato would paste for this text in this kind of app, by the rules:
/// the Dictionary, fillers, snippets, cleanup at Light and the tone. Medium's
/// AI step isn't run.
#[tauri::command]
#[specta::specta]
pub fn preview_style(
    app: AppHandle,
    snippets: State<'_, Arc<SnippetManager>>,
    text: String,
    category: String,
    tone: Option<Tone>,
    level: Option<CleanupLevel>,
) -> String {
    let settings = get_settings(&app);
    let level = level.unwrap_or(settings.cleanup_level);
    let tone = tone.unwrap_or_else(|| settings.styles.get(&category).tone);
    let lexicon = crate::managers::dictionary::lexicon();
    let text = if lexicon.corrector.is_empty() {
        text
    } else {
        lexicon
            .corrector
            .apply(&text, false, settings.word_correction_threshold)
            .0
    };
    // The page is in English; a fixed language stands in for the evidence a
    // real dictation would carry.
    let language = match settings.selected_language.as_str() {
        "auto" | "" => OutputLanguageEvidence::UserSelected("en".into()),
        code => OutputLanguageEvidence::UserSelected(code.into()),
    };
    let text = normalize_transcription_output(&remove_filler_words(
        &text,
        &language,
        &settings.custom_filler_words,
        settings.filler_word_removal_enabled && level != CleanupLevel::None,
    ));
    let list = snippets.list().unwrap_or_default();
    let marked = snippet_rules::mark(&text, &snippet_store::rules_of(&list));
    let mut styled = marked.text.clone();
    if !marked.whole {
        if level != CleanupLevel::None {
            let lists = style_rules::list_style(&category, tone, settings.cleanup_rules.lists);
            styled = cleanup_rules::light(&styled, &settings.cleanup_rules, lists);
        }
        styled = style_rules::apply(&styled, &category, tone);
    }
    let vars = snippet_rules::Variables {
        now: chrono::Local::now(),
        clipboard: Some("{{clipboard}}".into()),
        selection: Some("{{selection}}".into()),
    };
    snippet_rules::unmark(&styled, &marked.hits, |id| {
        list.iter()
            .find(|s| s.id == id)
            .map(|s| snippet_rules::fill(&s.text, &vars))
            .unwrap_or_default()
    })
}

#[tauri::command]
#[specta::specta]
pub fn get_default_filler_words() -> Vec<String> {
    DEFAULT_FILLERS.iter().map(|w| w.to_string()).collect()
}

#[tauri::command]
#[specta::specta]
pub fn change_cleanup_level_setting(app: AppHandle, level: CleanupLevel) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.cleanup_level = level;
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_cleanup_rules_setting(app: AppHandle, rules: CleanupRules) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.cleanup_rules = rules;
    write_settings(&app, settings);
    Ok(())
}

/// A custom list of filler words, or None for the built-in ones.
#[tauri::command]
#[specta::specta]
pub fn change_custom_filler_words_setting(
    app: AppHandle,
    words: Option<Vec<String>>,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.custom_filler_words = words.map(|list| {
        let mut out: Vec<String> = Vec::new();
        for w in list {
            let w = w.trim().to_lowercase();
            if !w.is_empty() && !out.contains(&w) {
                out.push(w);
            }
        }
        out
    });
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_style_enabled_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.style_enabled = enabled;
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_category_style_setting(
    app: AppHandle,
    category: String,
    style: CategoryStyle,
) -> Result<(), String> {
    if !style_rules::tones_for(&category).contains(&style.tone) {
        return Err(format!("{:?} isn't a tone for {category}", style.tone));
    }
    let mut settings = get_settings(&app);
    if !settings.styles.set(&category, style) {
        return Err(format!("Unknown kind of app: {category}"));
    }
    write_settings(&app, settings);
    Ok(())
}
