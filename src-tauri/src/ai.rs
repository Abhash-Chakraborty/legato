//! Your own AI: the provider and model from Settings › AI, whether it is
//! answering, and the correction pass that lets the model fix misheard
//! Dictionary words.

pub use crate::ai_rules::AiStatusKind;
use crate::ai_rules::{self, PromptWord};
use crate::llm_client;
use crate::managers::dictionary::DictionaryManager;
use crate::notes_rules;
use crate::secrets;
use crate::settings::{
    self, AiLastTest, AiTask, AppSettings, PostProcessApiFormat, PostProcessProvider,
};
use log::{debug, warn};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

/// Providers that work without a key.
const KEYLESS: [&str; 2] = ["ollama", "custom"];

pub fn needs_key(provider_id: &str) -> bool {
    !KEYLESS.contains(&provider_id)
}

/// Whether text sent to this provider stays on this PC.
pub fn is_local(provider: &PostProcessProvider) -> bool {
    provider.id == "ollama" || ai_rules::is_local_url(&provider.base_url)
}

/// What the status line shows, on every screen that uses the AI.
#[derive(Serialize, Clone, Debug, Type)]
pub struct AiStatus {
    pub kind: AiStatusKind,
    pub provider_id: String,
    pub provider_label: String,
    pub model: String,
    pub base_url: String,
    pub local: bool,
    pub needs_key: bool,
    pub has_key: bool,
    /// The last successful Test connection for this provider and model.
    pub tested_at: Option<i64>,
    pub latency_ms: Option<u32>,
    /// When the problem in `kind` was last seen.
    pub problem_at: Option<i64>,
}

/// Sent when the provider, model, key or the provider's answers change what
/// the status line says.
#[derive(Clone, Debug, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct AiStatusChanged;

/// The latest answer, or lack of one, from the provider in use.
struct Live {
    fingerprint: String,
    kind: AiStatusKind,
    at: i64,
}

static LIVE: Mutex<Option<Live>> = Mutex::new(None);

fn fingerprint(provider: &PostProcessProvider, model: &str) -> String {
    format!(
        "{}|{}|{}",
        provider.id,
        model,
        provider.base_url.trim_end_matches('/')
    )
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// How long an AI step in a dictation waits before the text is pasted
/// without it.
pub fn dictation_timeout(settings: &AppSettings) -> Duration {
    Duration::from_secs(u64::from(settings.ai_timeout_secs.clamp(
        crate::settings::AI_TIMEOUT_RANGE.0,
        crate::settings::AI_TIMEOUT_RANGE.1,
    )))
}

/// How long a Notes request waits; a long meeting takes the model longer
/// than a dictation.
pub fn notes_timeout(settings: &AppSettings) -> Duration {
    Duration::from_secs(u64::from(settings.notes_ai_timeout_secs.clamp(
        crate::settings::NOTES_AI_TIMEOUT_RANGE.0,
        crate::settings::NOTES_AI_TIMEOUT_RANGE.1,
    )))
}

/// Everything a request needs, when the AI is set up.
pub struct AiConfig {
    pub provider: PostProcessProvider,
    pub model: String,
    pub api_key: String,
    pub timeout: Duration,
    /// How long a Notes request (summary, question, fixes, follow-up) waits.
    pub notes_timeout: Duration,
}

impl AiConfig {
    /// The active provider and model, or None when no model is chosen or a
    /// needed key is missing.
    pub fn from_settings(settings: &AppSettings) -> Option<Self> {
        let provider = settings.active_post_process_provider()?.clone();
        let model = settings
            .post_process_models
            .get(&provider.id)
            .map(|m| m.trim().to_string())
            .unwrap_or_default();
        if model.is_empty() {
            return None;
        }
        let api_key = secrets::api_key(settings, &provider.id);
        if needs_key(&provider.id) && api_key.trim().is_empty() {
            return None;
        }
        Some(Self {
            provider,
            model,
            api_key,
            timeout: dictation_timeout(settings),
            notes_timeout: notes_timeout(settings),
        })
    }

    /// The provider and model picked for `task` in Settings › AI, otherwise
    /// the active ones. A pick that can't be used any more (its provider
    /// gone, or its key removed) falls back to the active ones too.
    pub fn for_task(settings: &AppSettings, task: AiTask) -> Option<Self> {
        let Some(choice) = settings.ai_task_models.get(task) else {
            return Self::from_settings(settings);
        };
        Self::with_model(settings, &choice.provider_id, &choice.model).or_else(|| {
            warn!("The model picked for {task:?} can't be used; using the main one");
            Self::from_settings(settings)
        })
    }

    /// A transform's own provider and model when it has them, otherwise the
    /// model for transforms.
    pub fn for_transform(
        settings: &AppSettings,
        transform: &crate::settings::Transform,
    ) -> Option<Self> {
        let (Some(provider_id), Some(model)) = (&transform.provider_id, &transform.model) else {
            return Self::for_task(settings, AiTask::Transforms);
        };
        Self::with_model(settings, provider_id, model)
    }

    /// This provider and model, or None when either is missing or the
    /// provider needs a key it hasn't got.
    fn with_model(settings: &AppSettings, provider_id: &str, model: &str) -> Option<Self> {
        let provider = settings
            .post_process_providers
            .iter()
            .find(|p| p.id == provider_id)?
            .clone();
        let model = model.trim().to_string();
        let api_key = secrets::api_key(settings, &provider.id);
        if model.is_empty() || (needs_key(&provider.id) && api_key.trim().is_empty()) {
            return None;
        }
        Some(Self {
            provider,
            model,
            api_key,
            timeout: dictation_timeout(settings),
            notes_timeout: notes_timeout(settings),
        })
    }

    fn disable_reasoning(&self) -> bool {
        matches!(
            self.provider.id.as_str(),
            "ollama" | "custom" | "openrouter" | "openai"
        )
    }
}

pub fn status(settings: &AppSettings) -> AiStatus {
    let Some(provider) = settings.active_post_process_provider() else {
        return AiStatus {
            kind: AiStatusKind::None,
            provider_id: settings.post_process_provider_id.clone(),
            provider_label: String::new(),
            model: String::new(),
            base_url: String::new(),
            local: false,
            needs_key: false,
            has_key: false,
            tested_at: None,
            latency_ms: None,
            problem_at: None,
        };
    };
    let model = settings
        .post_process_models
        .get(&provider.id)
        .map(|m| m.trim().to_string())
        .unwrap_or_default();
    let has_key = !secrets::api_key(settings, &provider.id).trim().is_empty();
    let needs = needs_key(&provider.id);
    let print = fingerprint(provider, &model);
    let test = settings
        .ai_last_test
        .as_ref()
        .filter(|t| fingerprint_of_test(t) == print);
    let live = LIVE.lock().ok().and_then(|l| {
        l.as_ref()
            .filter(|l| l.fingerprint == print)
            .map(|l| (l.kind, l.at))
    });

    let (kind, problem_at) = if model.is_empty() || (needs && !has_key) {
        (AiStatusKind::None, None)
    } else if let Some((kind, at)) = live {
        (kind, (kind != AiStatusKind::Ok).then_some(at))
    } else if test.is_some() {
        (AiStatusKind::Ok, None)
    } else {
        (AiStatusKind::Untested, None)
    };

    AiStatus {
        kind,
        provider_id: provider.id.clone(),
        provider_label: provider.label.clone(),
        model,
        base_url: provider.base_url.clone(),
        local: is_local(provider),
        needs_key: needs,
        has_key,
        tested_at: test.map(|t| t.at),
        latency_ms: test.map(|t| t.latency_ms),
        problem_at,
    }
}

fn fingerprint_of_test(test: &AiLastTest) -> String {
    format!(
        "{}|{}|{}",
        test.provider_id,
        test.model,
        test.base_url.trim_end_matches('/')
    )
}

/// Tells every screen to read the status again.
pub fn notify(app: &AppHandle) {
    let _ = AiStatusChanged.emit(app);
}

/// Remembers how the provider answered and tells the screens when that
/// changes what they show.
fn report(app: &AppHandle, config: &AiConfig, kind: AiStatusKind) {
    let print = fingerprint(&config.provider, &config.model);
    let changed = match LIVE.lock() {
        Ok(mut live) => {
            let changed = live
                .as_ref()
                .is_none_or(|l| l.fingerprint != print || l.kind != kind);
            *live = Some(Live {
                fingerprint: print,
                kind,
                at: now(),
            });
            changed
        }
        Err(_) => false,
    };
    if changed {
        notify(app);
    }
}

/// Sends one request and waits at most `timeout` for the answer.
async fn ask(
    config: &AiConfig,
    system: &str,
    user: String,
    schema: Option<serde_json::Value>,
    timeout: Duration,
) -> Result<String, AiStatusKind> {
    let call = async {
        if config.provider.api_format == PostProcessApiFormat::Anthropic {
            llm_client::send_chat_completion(
                &config.provider,
                config.api_key.clone(),
                &config.model,
                format!("{system}\n\n{user}"),
                false,
            )
            .await
        } else {
            llm_client::send_chat_completion_with_schema(
                &config.provider,
                config.api_key.clone(),
                &config.model,
                user,
                Some(system.to_string()),
                schema.filter(|_| config.provider.supports_structured_output),
                config.disable_reasoning(),
            )
            .await
        }
    };
    match tokio::time::timeout(timeout, call).await {
        Err(_) => Err(AiStatusKind::Slow),
        Ok(Err(e)) => Err(ai_rules::classify_error(&e)),
        Ok(Ok(None)) => Err(AiStatusKind::Failed),
        Ok(Ok(Some(text))) => Ok(text),
    }
}

/// A step that was skipped because the provider didn't answer properly.
#[derive(Clone, Debug)]
pub struct AiProblem {
    pub kind: AiStatusKind,
    pub provider_id: String,
    pub provider_label: String,
}

impl AiProblem {
    fn new(config: &AiConfig, kind: AiStatusKind) -> Self {
        Self {
            kind,
            provider_id: config.provider.id.clone(),
            provider_label: config.provider.label.clone(),
        }
    }

    /// No model is set up for the provider a step needs.
    pub fn not_set_up(settings: &AppSettings, provider_id: Option<&str>) -> Self {
        let id = provider_id.unwrap_or(&settings.post_process_provider_id);
        let label = settings
            .post_process_providers
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.label.clone())
            .unwrap_or_default();
        Self {
            kind: AiStatusKind::None,
            provider_id: id.to_string(),
            provider_label: label,
        }
    }
}

/// Sends one request with no answer format, waits at most `timeout` and
/// reports how the provider answered.
pub async fn run(
    app: &AppHandle,
    config: &AiConfig,
    system: &str,
    user: String,
    timeout: Duration,
) -> Result<String, AiProblem> {
    match ask(config, system, user, None, timeout).await {
        Ok(text) => {
            report(app, config, AiStatusKind::Ok);
            Ok(text)
        }
        Err(kind) => {
            report(app, config, kind);
            Err(AiProblem::new(config, kind))
        }
    }
}

/// `run`, with an answer format the provider is asked to follow.
async fn run_json(
    app: &AppHandle,
    config: &AiConfig,
    system: &str,
    user: String,
    schema: serde_json::Value,
    timeout: Duration,
) -> Result<String, AiProblem> {
    match ask(config, system, user, Some(schema), timeout).await {
        Ok(text) => {
            report(app, config, AiStatusKind::Ok);
            Ok(text)
        }
        Err(kind) => {
            report(app, config, kind);
            Err(AiProblem::new(config, kind))
        }
    }
}

/// Summarises a note's transcript: in one request, or part by part and then
/// combined when it's long.
pub async fn summarise_note(
    app: &AppHandle,
    config: &AiConfig,
    lines: &[notes_rules::PromptLine<'_>],
    tags_in_use: &[String],
    people: &[String],
    words: &[String],
    you: Option<&str>,
) -> Result<notes_rules::NotesSummary, AiProblem> {
    let json_hint = !config.provider.supports_structured_output
        || config.provider.api_format == PostProcessApiFormat::Anthropic;
    let one = |request: String| async move {
        let answer = run_json(
            app,
            config,
            notes_rules::SUMMARY_SYSTEM,
            request,
            notes_rules::summary_schema(),
            config.notes_timeout,
        )
        .await?;
        notes_rules::parse_summary(&answer).ok_or_else(|| {
            warn!("Notes summary: the model's answer wasn't the expected JSON");
            AiProblem::new(config, AiStatusKind::Failed)
        })
    };
    let parts = notes_rules::transcript_parts(lines);
    if parts.len() <= 1 {
        let transcript = parts.into_iter().next().unwrap_or_default();
        return one(notes_rules::summary_request(
            &transcript,
            json_hint,
            tags_in_use,
            people,
            words,
            you,
        ))
        .await;
    }
    debug!("Summarising a long note in {} parts", parts.len());
    let mut summaries = Vec::new();
    for part in &parts {
        summaries.push(
            one(notes_rules::summary_request(
                part,
                json_hint,
                tags_in_use,
                people,
                words,
                you,
            ))
            .await?,
        );
    }
    let mut combined = one(notes_rules::combine_request(
        &summaries,
        json_hint,
        tags_in_use,
        words,
        you,
    ))
    .await?;
    // Each part's details keep their own times; the combined answer's
    // topics, decisions and tasks cover the whole.
    combined.outline.details = summaries
        .iter()
        .flat_map(|s| s.outline.details.iter().cloned())
        .collect();
    combined.outline.people = notes_rules::merge_people(
        std::iter::once(combined.outline.people.as_slice())
            .chain(summaries.iter().map(|s| s.outline.people.as_slice())),
    );
    Ok(combined)
}

/// Answers a question from a note's transcript, citing line times.
pub async fn ask_note(
    app: &AppHandle,
    config: &AiConfig,
    lines: &[notes_rules::PromptLine<'_>],
    question: &str,
) -> Result<String, AiProblem> {
    let transcript = notes_rules::ask_transcript(lines, question);
    let answer = run(
        app,
        config,
        notes_rules::ASK_SYSTEM,
        notes_rules::ask_request(&transcript, question),
        config.notes_timeout,
    )
    .await?;
    Ok(answer.trim().to_string())
}

/// Misheard words in a note's transcript, found part by part.
pub async fn find_note_fixes(
    app: &AppHandle,
    config: &AiConfig,
    lines: &[notes_rules::PromptLine<'_>],
    title: &str,
    words: &[String],
) -> Result<Vec<notes_rules::LineFix>, AiProblem> {
    let json_hint = !config.provider.supports_structured_output
        || config.provider.api_format == PostProcessApiFormat::Anthropic;
    let texts: Vec<&str> = lines.iter().map(|l| l.text).collect();
    let mut fixes = Vec::new();
    for part in notes_rules::numbered_parts(lines) {
        let answer = run_json(
            app,
            config,
            notes_rules::FIX_SYSTEM,
            notes_rules::fix_request(&part, title, words, json_hint),
            notes_rules::fix_schema(),
            config.notes_timeout,
        )
        .await?;
        fixes.extend(notes_rules::parse_fixes(&answer, &texts));
        if fixes.len() >= notes_rules::FIX_MAX {
            fixes.truncate(notes_rules::FIX_MAX);
            break;
        }
    }
    Ok(fixes)
}

/// A follow-up email from a note's summary.
pub async fn follow_up(
    app: &AppHandle,
    config: &AiConfig,
    request: String,
) -> Result<String, AiProblem> {
    let answer = run(
        app,
        config,
        notes_rules::FOLLOW_UP_SYSTEM,
        request,
        config.notes_timeout,
    )
    .await?;
    Ok(answer.trim().to_string())
}

/// Shown on the recording pill after a paste that went out without the AI's
/// changes.
#[derive(Serialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AiNotice {
    AiSkipped {
        problem: AiStatusKind,
        provider_id: String,
        provider_label: String,
    },
}

impl From<&AiProblem> for AiNotice {
    fn from(problem: &AiProblem) -> Self {
        AiNotice::AiSkipped {
            problem: problem.kind,
            provider_id: problem.provider_id.clone(),
            provider_label: problem.provider_label.clone(),
        }
    }
}

/// What the correction pass changed.
pub struct Corrected {
    pub text: String,
    /// (heard text, Dictionary word) for each change.
    pub changes: Vec<(String, String)>,
}

/// Cleanup at Medium: one request that fixes grammar and false starts and,
/// when the correction pass is on, Dictionary spellings too. `text` carries
/// snippet placeholders, which the answer must keep. Ok(None) means it
/// didn't run or the answer was thrown away; the rule-based text is pasted.
pub async fn medium_pass(
    app: &AppHandle,
    settings: &AppSettings,
    text: &str,
    placeholders: usize,
) -> Result<Option<String>, AiProblem> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    if (text.split_whitespace().count() as u32) < settings.ai_min_words {
        debug!("Cleanup at Medium skipped: dictation is shorter than the minimum");
        return Ok(None);
    }
    let Some(config) = AiConfig::for_task(settings, AiTask::Dictation) else {
        return Ok(None);
    };
    let entries = if settings.ai_correction_enabled {
        let mut entries = app
            .try_state::<Arc<DictionaryManager>>()
            .and_then(|d| d.list().ok())
            .unwrap_or_default();
        entries.sort_by(|a, b| b.pinned.cmp(&a.pinned).then(b.hits.cmp(&a.hits)));
        entries
    } else {
        Vec::new()
    };
    let words: Vec<PromptWord> = entries
        .iter()
        .map(|e| PromptWord {
            text: &e.text,
            heard: &e.heard,
            ordinary: ordinary_forms(&e.heard),
            kind: e.kind.map(|k| k.noun()),
        })
        .collect();
    let input = crate::snippet_rules::to_ai(text);
    let request = ai_rules::cleanup_request(&words, &input);
    let started = Instant::now();
    let system = ai_rules::cleanup_system_prompt(placeholders);
    let answer = match ask(&config, &system, request, None, config.timeout).await {
        Ok(answer) => answer,
        Err(kind) => {
            warn!(
                "Cleanup at Medium skipped: {} answered {:?} after {:?}",
                config.provider.id,
                kind,
                started.elapsed()
            );
            report(app, &config, kind);
            return Err(AiProblem::new(&config, kind));
        }
    };
    report(app, &config, AiStatusKind::Ok);
    let cleaned = crate::transform_rules::clean_answer(&answer, &input);
    if !ai_rules::plausible_cleanup(&input, &cleaned) {
        warn!("Cleanup at Medium: the answer changed too much, kept the rule-based text");
        return Ok(None);
    }
    if ai_rules::invents_words(&input, &cleaned, &words) {
        warn!(
            "Cleanup at Medium: the answer brought in a Dictionary word, kept the rule-based text"
        );
        return Ok(None);
    }
    let Some(restored) = crate::snippet_rules::from_ai(&cleaned, placeholders) else {
        warn!("Cleanup at Medium: the answer lost a snippet, kept the rule-based text");
        return Ok(None);
    };
    debug!("Cleanup at Medium took {:?}", started.elapsed());
    Ok(Some(restored))
}

/// The single-word heard forms that are real words, for the prompt.
fn ordinary_forms(heard: &[String]) -> Vec<&str> {
    heard
        .iter()
        .map(String::as_str)
        .filter(|h| crate::spelling::is_word(h))
        .collect()
}

/// Lets the model fix misheard Dictionary words, when Settings › AI allows
/// it. Ok(None) means the pass didn't run: it is off, the AI isn't set up,
/// the Dictionary is empty or the dictation is short.
pub async fn correction_pass(
    app: &AppHandle,
    settings: &AppSettings,
    text: &str,
) -> Result<Option<Corrected>, AiProblem> {
    if !settings.ai_correction_enabled || text.trim().is_empty() {
        return Ok(None);
    }
    if (text.split_whitespace().count() as u32) < settings.ai_min_words {
        debug!("Correction pass skipped: dictation is shorter than the minimum");
        return Ok(None);
    }
    let Some(config) = AiConfig::for_task(settings, AiTask::Dictation) else {
        return Ok(None);
    };
    let Some(dictionary) = app.try_state::<Arc<DictionaryManager>>() else {
        return Ok(None);
    };
    let mut entries = match dictionary.list() {
        Ok(entries) if !entries.is_empty() => entries,
        Ok(_) => return Ok(None),
        Err(e) => {
            warn!("Correction pass skipped: couldn't read the Dictionary: {e}");
            return Ok(None);
        }
    };
    // The most used words first, in case the list is cut.
    entries.sort_by(|a, b| b.pinned.cmp(&a.pinned).then(b.hits.cmp(&a.hits)));
    let words: Vec<PromptWord> = entries
        .iter()
        .map(|e| PromptWord {
            text: &e.text,
            heard: &e.heard,
            ordinary: ordinary_forms(&e.heard),
            kind: e.kind.map(|k| k.noun()),
        })
        .collect();

    let json_hint = !config.provider.supports_structured_output
        || config.provider.api_format == PostProcessApiFormat::Anthropic;
    let request = ai_rules::correction_request(&words, text, json_hint);
    let started = Instant::now();
    let answer = ask(
        &config,
        ai_rules::CORRECTION_SYSTEM_PROMPT,
        request,
        Some(ai_rules::correction_schema()),
        config.timeout,
    )
    .await;
    let answer = match answer {
        Ok(answer) => answer,
        Err(kind) => {
            warn!(
                "Correction pass skipped: {} answered {:?} after {:?}",
                config.provider.id,
                kind,
                started.elapsed()
            );
            report(app, &config, kind);
            return Err(AiProblem::new(&config, kind));
        }
    };
    report(app, &config, AiStatusKind::Ok);

    let Some(replacements) = ai_rules::parse_replacements(&answer) else {
        warn!("Correction pass: the model's answer wasn't the expected JSON");
        return Ok(None);
    };
    let (corrected, changes) = ai_rules::apply_replacements(text, &words, &replacements);
    debug!(
        "Correction pass: {} of {} suggestions applied in {:?}",
        changes.len(),
        replacements.len(),
        started.elapsed()
    );
    Ok(Some(Corrected {
        text: corrected,
        changes,
    }))
}

#[derive(Serialize, Clone, Debug, Type)]
pub struct AiTestResult {
    pub status: AiStatus,
    /// The provider's model list, when it sent one.
    pub models: Vec<String>,
}

/// How long Test connection waits; longer than a dictation would, since the
/// first request may have to load the model.
const TEST_TIMEOUT: Duration = Duration::from_secs(20);

/// When a dictation starts that will go to the AI, opens the connection to
/// the provider while you speak, so the request when you stop skips the
/// handshake.
pub fn warm_up(app: &AppHandle) {
    let settings = settings::get_settings(app);
    let uses_ai = settings.cleanup_level == crate::cleanup_rules::CleanupLevel::Medium
        || settings.ai_correction_enabled
        || settings.post_process_enabled;
    if !uses_ai {
        return;
    }
    let Some(config) = AiConfig::for_task(&settings, AiTask::Dictation) else {
        return;
    };
    tauri::async_runtime::spawn(async move {
        llm_client::warm_connection(&config.provider, &config.api_key).await;
    });
}

/// Fetches the model list and sends one short request, then records the
/// round trip for the status line.
pub async fn test_connection(app: &AppHandle) -> AiTestResult {
    let settings = settings::get_settings(app);
    let Some(config) = AiConfig::from_settings(&settings) else {
        return AiTestResult {
            status: status(&settings),
            models: Vec::new(),
        };
    };

    // The list says whether anything is there and whether the key works. A
    // server without a list (404) can still answer requests.
    let models = match tokio::time::timeout(
        TEST_TIMEOUT,
        llm_client::fetch_models(&config.provider, config.api_key.clone()),
    )
    .await
    {
        Ok(Ok(models)) => models,
        Ok(Err(e)) => match ai_rules::classify_error(&e) {
            AiStatusKind::NotFound => Vec::new(),
            kind => return finish_test(app, &config, Err(kind), Vec::new()),
        },
        Err(_) => return finish_test(app, &config, Err(AiStatusKind::Slow), Vec::new()),
    };

    let started = Instant::now();
    let answer = ask(
        &config,
        "Reply with the single word OK.",
        "Are you there?".to_string(),
        None,
        TEST_TIMEOUT,
    )
    .await;
    let latency = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
    finish_test(app, &config, answer.map(|_| latency), models)
}

fn finish_test(
    app: &AppHandle,
    config: &AiConfig,
    result: Result<u32, AiStatusKind>,
    models: Vec<String>,
) -> AiTestResult {
    match result {
        Ok(latency_ms) => {
            let mut settings = settings::get_settings(app);
            settings.ai_last_test = Some(AiLastTest {
                provider_id: config.provider.id.clone(),
                model: config.model.clone(),
                base_url: config.provider.base_url.clone(),
                at: now(),
                latency_ms,
            });
            settings::write_settings(app, settings);
            report(app, config, AiStatusKind::Ok);
        }
        Err(kind) => report(app, config, kind),
    }
    AiTestResult {
        status: status(&settings::get_settings(app)),
        models,
    }
}
