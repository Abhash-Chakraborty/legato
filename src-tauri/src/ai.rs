//! Your own AI: the provider and model from Settings › AI, whether it is
//! answering, and the correction pass that lets the model fix misheard
//! Dictionary words.

pub use crate::ai_rules::AiStatusKind;
use crate::ai_rules::{self, PromptWord};
use crate::llm_client;
use crate::managers::dictionary::DictionaryManager;
use crate::secrets;
use crate::settings::{self, AiLastTest, AppSettings, PostProcessApiFormat, PostProcessProvider};
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

/// Everything a request needs, when the AI is set up.
pub struct AiConfig {
    pub provider: PostProcessProvider,
    pub model: String,
    pub api_key: String,
    pub timeout: Duration,
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
            timeout: Duration::from_secs(u64::from(settings.ai_timeout_secs.clamp(1, 60))),
        })
    }

    /// A transform's own provider and model when it has them, otherwise the
    /// active ones.
    pub fn for_transform(
        settings: &AppSettings,
        transform: &crate::settings::Transform,
    ) -> Option<Self> {
        let (Some(provider_id), Some(model)) = (&transform.provider_id, &transform.model) else {
            return Self::from_settings(settings);
        };
        let provider = settings
            .post_process_providers
            .iter()
            .find(|p| &p.id == provider_id)?
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
            timeout: Duration::from_secs(u64::from(settings.ai_timeout_secs.clamp(1, 60))),
        })
    }

    fn disable_reasoning(&self) -> bool {
        matches!(
            self.provider.id.as_str(),
            "ollama" | "custom" | "openrouter"
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
    let Some(config) = AiConfig::from_settings(settings) else {
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
    let spellings: Vec<&str> = entries.iter().map(|e| e.text.as_str()).collect();
    let (corrected, changes) = ai_rules::apply_replacements(text, &spellings, &replacements);
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
