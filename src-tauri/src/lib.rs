mod actions;
mod ai;
mod ai_rules;
mod audio_feedback;
pub mod audio_toolkit;
mod autostart;
mod background_audio;
mod bubble;
mod bubble_rules;
mod calendar;
mod calendar_rules;
mod call_people;
mod calls;
mod calls_rules;
mod catalog;
mod chinese_script;
mod cleanup_rules;
pub mod cli;
mod clipboard;
mod commands;
mod context;
mod desktop;
mod document;
mod edit_reader;
mod elevation;
pub mod engine_supervisor;
mod google;
mod handy_migration;
mod input;
mod inyon;
mod inyon_rules;
mod learn_rules;
mod llm_client;
mod loopback;
mod managers;
pub mod mcp;
mod mcp_rules;
mod mic_rules;
mod note_copies;
mod notes;
mod notes_panel;
mod notes_rules;
mod overlay;
mod paste_tx;
pub mod portable;
mod scratchpad;
mod secrets;
mod selection;
mod settings;
mod setup;
mod shortcut;
mod signal_handle;
mod snippet_rules;
mod speakers;
mod spelling;
mod style_rules;
mod sync;
mod transcription_coordinator;
mod transform_rules;
mod transforms;
mod trash;
mod tray;
mod tray_i18n;
mod utils;

pub use cli::CliArgs;
#[cfg(debug_assertions)]
use specta_typescript::{BigIntExportBehavior, Typescript};
use tauri_specta::{collect_commands, collect_events, Builder};
pub use utils::env_flag_enabled;

use env_filter::Builder as EnvFilterBuilder;
use managers::audio::AudioRecordingManager;
use managers::dictionary::DictionaryManager;
use managers::history::HistoryManager;
use managers::model::ModelManager;
use managers::transcription::TranscriptionManager;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use tauri::image::Image;
pub use transcription_coordinator::TranscriptionCoordinator;

use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Listener, Manager};
use tauri_plugin_log::{Builder as LogBuilder, RotationStrategy, Target, TargetKind};

use crate::settings::get_settings;

// Global atomic to store the file log level filter
// We use u8 to store the log::LevelFilter as a number
pub static FILE_LOG_LEVEL: AtomicU8 = AtomicU8::new(log::LevelFilter::Debug as u8);

/// When `true`, log records are also forwarded to the webview via the
/// `log://log` event for the debug panel's live log viewer. Gated on debug
/// mode — the live log viewer is its only consumer and only exists in debug
/// mode — so normal runs never broadcast log records (which can include file
/// paths or transcribed text) onto the frontend event bus. Synced at startup
/// and whenever debug mode is toggled (see `shortcut::change_debug_mode_setting`).
pub static WEBVIEW_LOG_STREAMING: AtomicBool = AtomicBool::new(false);

fn level_filter_from_u8(value: u8) -> log::LevelFilter {
    match value {
        0 => log::LevelFilter::Off,
        1 => log::LevelFilter::Error,
        2 => log::LevelFilter::Warn,
        3 => log::LevelFilter::Info,
        4 => log::LevelFilter::Debug,
        5 => log::LevelFilter::Trace,
        _ => log::LevelFilter::Trace,
    }
}

fn build_console_filter() -> env_filter::Filter {
    let mut builder = EnvFilterBuilder::new();

    match std::env::var("RUST_LOG") {
        Ok(spec) if !spec.trim().is_empty() => {
            if let Err(err) = builder.try_parse(&spec) {
                log::warn!(
                    "Ignoring invalid RUST_LOG value '{}': {}. Falling back to info-level console logging",
                    spec,
                    err
                );
                builder.filter_level(log::LevelFilter::Info);
            }
        }
        _ => {
            builder.filter_level(log::LevelFilter::Info);
        }
    }

    builder.build()
}

/// The jump list's Scratchpad and New note, or the same flags on the command
/// line.
fn open_scratchpad_from_cli(app: &AppHandle, new_note: bool) {
    let result = if new_note {
        scratchpad::show_new(app)
    } else {
        scratchpad::show(app, None)
    };
    if let Err(e) = result {
        log::error!("Couldn't open the Scratchpad: {e}");
    }
}

/// What a second Legato process asked for: a remote-control flag, or just
/// the window.
fn handle_second_instance(app: &AppHandle, args: &[String]) {
    if args.iter().any(|a| a == "--toggle-transcription") {
        signal_handle::send_transcription_input(app, "transcribe", "CLI");
    } else if args.iter().any(|a| a == "--toggle-post-process") {
        signal_handle::send_transcription_input(app, "transcribe_with_post_process", "CLI");
    } else if args.iter().any(|a| a == "--cancel") {
        crate::utils::cancel_current_operation(app);
    } else if args.iter().any(|a| a == desktop::SCRATCHPAD_ARG) {
        open_scratchpad_from_cli(app, false);
    } else if args.iter().any(|a| a == desktop::NEW_NOTE_ARG) {
        open_scratchpad_from_cli(app, true);
    } else {
        // A second process was launched without remote-control flags:
        // raise the window.
        show_main_window(app);
    }
}

pub(crate) fn show_main_window(app: &AppHandle) {
    if let Some(main_window) = app.get_webview_window("main") {
        if let Err(e) = main_window.unminimize() {
            log::error!("Failed to unminimize webview window: {}", e);
        }
        if let Err(e) = main_window.show() {
            log::error!("Failed to show webview window: {}", e);
        }
        if let Err(e) = main_window.set_focus() {
            log::error!("Failed to focus webview window: {}", e);
        }
        return;
    }

    let webview_labels = app.webview_windows().keys().cloned().collect::<Vec<_>>();
    log::error!(
        "Main window not found. Webview labels: {:?}",
        webview_labels
    );
}

/// Asks the main window to show a page, such as Needs review in the
/// Dictionary.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type, tauri_specta::Event)]
pub struct OpenPage {
    pub page: String,
    pub section: Option<String>,
}

/// Brings the main window up on a page.
pub(crate) fn open_page(app: &AppHandle, page: &str, section: Option<&str>) {
    use tauri_specta::Event;
    show_main_window(app);
    let _ = OpenPage {
        page: page.to_string(),
        section: section.map(str::to_string),
    }
    .emit_to(app, "main");
}

/// The main window's first size and its minimum, in logical pixels.
const MAIN_WINDOW_SIZE: (f64, f64) = (1240.0, 800.0);
const MAIN_WINDOW_MIN_SIZE: (f64, f64) = (960.0, 640.0);

/// Remembers the main window's size, position and maximised state between
/// launches. Visibility stays with the start-hidden logic and decorations with
/// the title bar setting, so neither is saved. Portable installs keep the file
/// in their Data folder.
fn window_state_plugin<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    use tauri_plugin_window_state::StateFlags;

    let mut builder = tauri_plugin_window_state::Builder::default()
        .with_state_flags(StateFlags::SIZE | StateFlags::POSITION | StateFlags::MAXIMIZED)
        .with_denylist(&["recording_overlay"]);
    if let Some(data_dir) = portable::data_dir() {
        // The plugin joins this onto the config folder; an absolute path wins.
        builder = builder.with_filename(data_dir.join("window-state.json").to_string_lossy());
    }
    builder.build()
}

/// Keeps a window inside the screen it's on. A small or highly scaled
/// display (1366 × 768 at 125% is 1093 × 614) can't fit the default size or
/// even the minimum, so both shrink to the work area there.
pub(crate) fn fit_window_to_screen(window: &tauri::WebviewWindow, min: (f64, f64)) {
    let Ok(Some(monitor)) = window.current_monitor() else {
        return;
    };
    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let area_w = f64::from(area.size.width) / scale;
    let area_h = f64::from(area.size.height) / scale;

    let min_w = min.0.min(area_w);
    let min_h = min.1.min(area_h);
    if let Err(e) = window.set_min_size(Some(tauri::LogicalSize::new(min_w, min_h))) {
        log::warn!(
            "Failed to set the {} window's minimum size: {e}",
            window.label()
        );
    }

    // A restored maximised window already fills its screen.
    if window.is_maximized().unwrap_or(false) {
        return;
    }
    let Ok(size) = window.outer_size() else {
        return;
    };
    let width = f64::from(size.width) / scale;
    let height = f64::from(size.height) / scale;
    if width <= area_w && height <= area_h {
        keep_on_screen(window, area);
        return;
    }
    let fitted = tauri::LogicalSize::new(
        width.min(area_w * 0.92).max(min_w),
        height.min(area_h * 0.92).max(min_h),
    );
    if let Err(e) = window.set_size(fitted) {
        log::warn!(
            "Failed to fit the {} window to the screen: {e}",
            window.label()
        );
    }
    let _ = window.center();
}

/// Moves a window that fits its screen but hangs off an edge (a remembered
/// spot from a taller screen, or a first launch below the top) back inside
/// the work area. The visible frame counts, not Windows' invisible resize
/// border around it.
fn keep_on_screen(window: &tauri::WebviewWindow, area: &tauri::PhysicalRect<i32, u32>) {
    let (Ok(outer), Ok(inner), Ok(size)) = (
        window.outer_position(),
        window.inner_position(),
        window.inner_size(),
    ) else {
        return;
    };
    let (w, h) = (size.width as i32, size.height as i32);
    let (ax, ay) = (area.position.x, area.position.y);
    let (aw, ah) = (area.size.width as i32, area.size.height as i32);
    if w > aw || h > ah {
        return;
    }
    let x = inner.x.clamp(ax, ax + aw - w);
    let y = inner.y.clamp(ay, ay + ah - h);
    if (x, y) == (inner.x, inner.y) {
        return;
    }
    let moved = tauri::PhysicalPosition::new(outer.x + x - inner.x, outer.y + y - inner.y);
    if let Err(e) = window.set_position(moved) {
        log::warn!(
            "Failed to move the {} window on screen: {e}",
            window.label()
        );
    }
}

fn should_force_show_permissions_window(app: &AppHandle) -> bool {
    let model_manager = app.state::<Arc<ModelManager>>();
    let has_downloaded_models = model_manager
        .get_available_models()
        .iter()
        .any(|model| model.is_downloaded);

    if !has_downloaded_models {
        return false;
    }

    let status = commands::audio::get_windows_microphone_permission_status();
    if status.overall_access == commands::audio::PermissionAccess::Denied {
        log::info!(
            "Windows microphone permissions are denied; forcing main window visible for onboarding"
        );
        return true;
    }

    false
}

fn initialize_core_logic(app_handle: &AppHandle) {
    // API keys an older version kept in the settings file go to Credential
    // Manager before anything reads them.
    secrets::move_keys_out_of_settings(app_handle);

    // The pages may play recordings through the asset protocol, and nothing
    // else: its scope is empty in tauri.conf.json, so a page can't read
    // other files that way. Added here since portable data lives anywhere.
    if let Ok(dir) = portable::app_data_dir(app_handle) {
        if let Err(e) = app_handle
            .asset_protocol_scope()
            .allow_directory(dir.join("recordings"), true)
        {
            log::warn!("Couldn't let the pages play recordings: {e}");
        }
    }

    // Note: Enigo (keyboard/mouse simulation) is NOT initialized here.
    // The frontend is responsible for calling the `initialize_enigo` command
    // after onboarding completes.

    // Initialize the managers. The audio recorder receives the streaming router
    // explicitly, so always-on microphone startup can wire live-preview frames
    // even before Tauri state is populated.
    let model_manager =
        Arc::new(ModelManager::new(app_handle).expect("Failed to initialize model manager"));
    let transcription_manager = Arc::new(
        TranscriptionManager::new(app_handle, model_manager.clone())
            .expect("Failed to initialize transcription manager"),
    );
    let recording_manager = Arc::new(
        AudioRecordingManager::new(app_handle, transcription_manager.stream_router())
            .expect("Failed to initialize recording manager"),
    );
    let history_manager =
        Arc::new(HistoryManager::new(app_handle).expect("Failed to initialize history manager"));
    // After history: it migrates the database the Dictionary lives in.
    let dictionary_manager =
        Arc::new(DictionaryManager::new(app_handle).expect("Failed to initialize the Dictionary"));
    let snippet_manager = Arc::new(
        managers::snippets::SnippetManager::new(app_handle).expect("Failed to initialize snippets"),
    );
    let scratchpad_manager = Arc::new(
        scratchpad::ScratchpadManager::new(app_handle)
            .expect("Failed to initialize the Scratchpad"),
    );
    let notes_manager =
        Arc::new(notes::NotesManager::new(app_handle).expect("Failed to initialize Notes"));
    context::set_rules(settings::get_settings(app_handle).app_rules);

    // Add managers to Tauri's managed state
    app_handle.manage(recording_manager.clone());
    app_handle.manage(model_manager.clone());
    app_handle.manage(transcription_manager.clone());
    app_handle.manage(history_manager.clone());
    // Retention otherwise runs only when a dictation is saved, so old
    // recordings stayed for someone who stopped dictating.
    {
        let history = history_manager.clone();
        std::thread::spawn(move || {
            if let Err(e) = history.cleanup_old_entries() {
                log::warn!("Couldn't apply recording retention at startup: {e}");
            }
        });
    }
    app_handle.manage(dictionary_manager);
    app_handle.manage(snippet_manager);
    app_handle.manage(scratchpad_manager);
    app_handle.manage(notes_manager);
    // Items deleted more than 30 days ago leave Recently deleted.
    {
        let app = app_handle.clone();
        std::thread::spawn(move || trash::purge_expired(&app));
    }
    app_handle.manage(context::DictationContext::default());
    app_handle.manage(tray::TrayState::new());
    // Watches for calls, to offer notes.
    calls::init(app_handle);
    // Meetings from the calendars in Settings › Connections.
    calendar::init(app_handle);
    // After history: the ledger lives in its database.
    sync::init(app_handle);

    // Note: Shortcuts are NOT initialized here.
    // The frontend is responsible for calling the `initialize_shortcuts` command
    // after onboarding completes. This matches the pattern used for Enigo
    // initialization.

    // Get the current theme to set the appropriate initial icon
    let initial_theme = tray::get_current_theme(app_handle);

    // Choose the appropriate initial icon based on theme
    let initial_icon_path = tray::get_icon_path(initial_theme, tray::TrayIconState::Idle);

    let tray = TrayIconBuilder::new()
        .icon(
            Image::from_path(
                app_handle
                    .path()
                    .resolve(initial_icon_path, tauri::path::BaseDirectory::Resource)
                    .unwrap(),
            )
            .unwrap(),
        )
        .tooltip(tray::tray_tooltip())
        // Notification-area convention: left click opens the app, right click
        // shows the menu.
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
            let opens_window = matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } | TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                }
            );
            if opens_window {
                show_main_window(tray.app_handle());
            }
        })
        .on_menu_event(|app, event| match event.id.as_ref() {
            "settings" => {
                show_main_window(app);
            }
            "check_updates" => {
                let settings = settings::get_settings(app);
                if settings::update_checks_effectively_enabled(&settings) {
                    show_main_window(app);
                    let _ = app.emit("check-for-updates", ());
                }
            }
            "copy_last_transcript" => {
                tray::copy_last_transcript(app);
            }
            "pause_clipboard_sync" | "resume_clipboard_sync" => {
                tray::toggle_clipboard_sync(app);
            }
            "send_clipboard_to_phone" => {
                if let Some(sync) = app.try_state::<Arc<sync::SyncManager>>() {
                    if let Err(e) = sync.send_clipboard_now() {
                        log::info!("Didn't send the clipboard: {e:#}");
                    }
                }
            }
            "unload_model" => {
                let transcription_manager = app.state::<Arc<TranscriptionManager>>();
                if !transcription_manager.is_model_loaded() {
                    log::warn!("No model is currently loaded.");
                    return;
                }
                transcription_manager.request_unload();
                log::info!("Model unloaded via tray.");
            }
            "cancel" => {
                use crate::utils::cancel_current_operation;

                // Use centralized cancellation that handles all operations
                cancel_current_operation(app);
            }
            "quit" => {
                log::info!("Quit from the tray menu.");
                app.exit(0);
            }
            id if id.starts_with("model_select:") => {
                let model_id = id.strip_prefix("model_select:").unwrap().to_string();
                let current_model = settings::get_settings(app).selected_model;
                if model_id == current_model {
                    return;
                }
                let app_clone = app.clone();
                std::thread::spawn(move || {
                    match commands::models::switch_active_model(&app_clone, &model_id) {
                        Ok(()) => {
                            log::info!("Model switched to {} via tray.", model_id);
                        }
                        Err(e) => {
                            log::error!("Failed to switch model via tray: {}", e);
                        }
                    }
                    tray::update_tray_menu(&app_clone);
                });
            }
            _ => {}
        })
        .build(app_handle)
        .unwrap();
    app_handle.manage(tray);

    // Initialize tray menu with idle state
    tray::update_tray_menu(app_handle);

    // Apply show_tray_icon setting
    let settings = settings::get_settings(app_handle);
    if !settings.show_tray_icon {
        tray::set_tray_visibility(app_handle, false);
    }

    // Refresh tray menu when model state changes
    let app_handle_for_listener = app_handle.clone();
    app_handle.listen("model-state-changed", move |_| {
        tray::update_tray_menu(&app_handle_for_listener);
    });

    // Apply the autostart preference
    autostart::apply_autostart(app_handle, settings.autostart_enabled);

    // Create the recording overlay window (hidden by default)
    utils::create_recording_overlay(app_handle);

    // The desktop bubble, when it's on (its window is created when it's
    // first wanted). The Notes panel's window waits until it first opens.
    bubble::init(app_handle);
    // Agents waiting on Inyon, while paired.
    inyon::init(app_handle);
}

#[tauri::command]
#[specta::specta]
fn trigger_update_check(app: AppHandle) -> Result<(), String> {
    let settings = settings::get_settings(&app);
    if !settings::update_checks_effectively_enabled(&settings) {
        return Ok(());
    }
    app.emit("check-for-updates", ())
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
fn show_main_window_command(app: AppHandle) -> Result<(), String> {
    show_main_window(&app);
    Ok(())
}

/// Convert an unexpected panic on the headless worker into a normal CLI
/// failure. Without this guard the Tauri event loop remains alive after the
/// worker exits, leaving `--transcribe-file` hung indefinitely.
fn run_headless_guarded<F>(operation: F) -> i32
where
    F: FnOnce() -> i32,
{
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation)) {
        Ok(code) => code,
        Err(payload) => {
            let message = if let Some(message) = payload.downcast_ref::<&str>() {
                (*message).to_string()
            } else if let Some(message) = payload.downcast_ref::<String>() {
                message.clone()
            } else {
                "unknown panic".to_string()
            };
            eprintln!("error: headless transcription panicked: {message}");
            1
        }
    }
}

/// Headless one-shot transcription for the `--transcribe-file` / `--list-devices`
/// path. Drives the same `TranscriptionManager::transcribe` the app uses; no
/// mic, no VAD, no download. Returns a process exit code (0 ok, 1 runtime
/// failure, 2 bad input/usage).
fn run_headless_transcription(app: &AppHandle, args: &CliArgs) -> i32 {
    use std::time::Instant;

    // --list-devices: print registered compute devices (with indices) and exit.
    // Useful on multi-GPU machines to discover the index for --device-index.
    if args.list_devices {
        let tm = app.state::<Arc<TranscriptionManager>>();
        let devices = match crate::managers::transcription::describe_compute_devices(&tm) {
            Ok(devices) => devices,
            Err(e) => {
                eprintln!("error: {}", e);
                return 1;
            }
        };
        if devices.is_empty() {
            println!("No transcribe-cpp compute devices registered.");
        } else {
            println!("transcribe-cpp compute devices:");
            for d in &devices {
                println!("  {}", d);
            }
        }
        if args.transcribe_file.is_none() {
            return 0;
        }
    }

    // --list-models: print the model registry (catalog + on-disk + custom) with
    // their ids — the same ids `--model` accepts — then exit. `--json` emits the
    // full ModelInfo array for scripting.
    if args.list_models {
        let model_manager = app.state::<Arc<ModelManager>>();
        let models = model_manager.get_available_models();
        if args.json {
            match serde_json::to_string_pretty(&models) {
                Ok(s) => println!("{}", s),
                Err(e) => {
                    eprintln!("error: failed to serialize models: {}", e);
                    return 1;
                }
            }
        } else if models.is_empty() {
            println!("No models available.");
        } else {
            println!("Available models (✓ = installed):");
            let width = models.iter().map(|m| m.id.len()).max().unwrap_or(0);
            for m in &models {
                let mark = if m.is_downloaded { "✓" } else { " " };
                let rec = if m.is_recommended {
                    "  [recommended]"
                } else {
                    ""
                };
                println!(
                    "  {}  {:<width$}  {}{}",
                    mark,
                    m.id,
                    m.name,
                    rec,
                    width = width
                );
            }
        }
        if args.transcribe_file.is_none() {
            return 0;
        }
    }

    let Some(wav) = args.transcribe_file.clone() else {
        return 0;
    };

    // read_wav_samples reads 16-bit int samples and does no validation; the app
    // only ever saves 16 kHz mono 16-bit PCM, so reject anything else rather than
    // transcribe garbage / mis-time / mis-decode.
    match hound::WavReader::open(&wav) {
        Ok(reader) => {
            let spec = reader.spec();
            if spec.sample_rate != 16_000
                || spec.channels != 1
                || spec.bits_per_sample != 16
                || spec.sample_format != hound::SampleFormat::Int
            {
                eprintln!(
                    "error: expected 16 kHz mono 16-bit PCM WAV, got {} Hz / {} ch / {}-bit {:?}",
                    spec.sample_rate, spec.channels, spec.bits_per_sample, spec.sample_format
                );
                return 2;
            }
        }
        Err(e) => {
            eprintln!("error: cannot open {}: {}", wav.display(), e);
            return 2;
        }
    }

    let samples = match crate::audio_toolkit::read_wav_samples(&wav) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: failed to read {}: {}", wav.display(), e);
            return 2;
        }
    };
    let audio_secs = samples.len() as f64 / 16_000.0;

    let tm = app.state::<Arc<TranscriptionManager>>();

    let model_id = args
        .model
        .clone()
        .unwrap_or_else(|| get_settings(app).selected_model);
    if model_id.is_empty() {
        eprintln!("error: no model selected (pass --model or pick one in the app)");
        return 2;
    }

    // --device-index hard-selects a compute device by its --list-devices registry
    // index (transcribe-cpp / whisper-family models only; not persisted). Omit it
    // to use the persisted accelerator setting.
    let device_index = args.device_index;
    let requested_device = match device_index {
        Some(idx) => format!("index {}", idx),
        None => "settings".to_string(),
    };

    // Cold load (timed).
    let load_start = Instant::now();
    if let Err(e) = tm.load_model_with_device(&model_id, device_index) {
        eprintln!("error: load_model('{}') failed: {}", model_id, e);
        return 1;
    }
    let load_ms = load_start.elapsed().as_millis() as u64;
    let bound_backend = tm.current_backend();

    let runs = args.repeat.unwrap_or(1).max(1);
    let mut times_ms: Vec<u64> = Vec::new();
    let mut text = String::new();
    for i in 0..runs {
        // If the model's unload-timeout is "Immediately", transcribe() unloads
        // the engine after each run; reload (untimed) so repeats keep working
        // and the inference timing below stays clean.
        if !tm.is_model_loaded() {
            if let Err(e) = tm.load_model_with_device(&model_id, device_index) {
                eprintln!("error: reload before run {} failed: {}", i + 1, e);
                return 1;
            }
        }
        let t = Instant::now();
        match tm.transcribe(samples.clone()) {
            Ok(out) => text = out.text,
            Err(e) => {
                eprintln!("error: transcribe failed: {}", e);
                return 1;
            }
        }
        times_ms.push(t.elapsed().as_millis() as u64);
    }
    let best_ms = times_ms.iter().copied().min().unwrap_or(0);
    let rtf = if best_ms > 0 {
        audio_secs / (best_ms as f64 / 1000.0)
    } else {
        0.0
    };

    if args.json {
        println!(
            "{}",
            serde_json::json!({
                "model": model_id,
                "requested_device": requested_device,
                "bound_backend": bound_backend,
                "audio_secs": audio_secs,
                "load_ms": load_ms,
                "transcribe_ms": times_ms,
                "best_ms": best_ms,
                "rtf": rtf,
                "text": text,
            })
        );
    } else {
        println!(
            "model={} device={} backend={} audio={:.2}s load={}ms best={}ms rtf={:.2}x",
            model_id,
            requested_device,
            bound_backend.as_deref().unwrap_or("?"),
            audio_secs,
            load_ms,
            best_ms,
            rtf,
        );
        println!("text: {}", text);
    }
    0
}

/// `legato --uninstall-cleanup [--with-secrets]`, run by the uninstaller
/// while the exe still exists: removes what Legato keeps outside its
/// folders. The run-as-administrator task always goes (it would start a
/// missing exe); Credential Manager entries go only with "Delete app data",
/// since a reinstall over kept data needs them. Best effort: the uninstall
/// carries on either way.
pub fn uninstall_cleanup_invocation() -> Option<i32> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.iter().any(|a| a == "--uninstall-cleanup") {
        return None;
    }
    #[cfg(windows)]
    elevation::delete_task();
    if args.iter().any(|a| a == "--with-secrets") {
        secrets::delete_all();
    }
    Some(0)
}

/// Where the logs are, without an AppHandle: the portable folder's, or
/// Tauri's log folder for this app's identifier.
fn log_dir_for_errors() -> std::path::PathBuf {
    if let Some(dir) = portable::data_dir() {
        return dir.join("logs");
    }
    std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_default()
        .join("io.github.abhash-chakraborty.legato")
        .join("logs")
}

/// Set once `RunEvent::Exit` has run. When Windows shuts down, tao's event
/// loop can still get a message after it was destroyed and panics with
/// "cannot move state from Destroyed"; by then Legato has finished, so that
/// panic must not put an error box in front of the shutdown.
static EXITING: AtomicBool = AtomicBool::new(false);

/// A panic on the main thread ends Legato, and with no console a person
/// would see it vanish. This logs every panic and, for that one, says what
/// happened and where the log is before the process goes.
fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let name = thread.name().unwrap_or("unnamed");
        if EXITING.load(Ordering::SeqCst) {
            log::warn!("Panic on thread {name} after exit: {info}");
            return;
        }
        log::error!("Panic on thread {name}: {info}");
        if name == "main" {
            #[cfg(windows)]
            {
                use windows::core::HSTRING;
                use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
                let text = format!(
                    "Legato hit a problem and has to close.\n\n{info}\n\nThe log is in {}",
                    log_dir_for_errors().display()
                );
                // SAFETY: both strings live until the box closes.
                unsafe {
                    MessageBoxW(
                        None,
                        &HSTRING::from(text),
                        &HSTRING::from("Legato"),
                        MB_OK | MB_ICONERROR,
                    );
                }
            }
        }
        default(info);
    }));
}

pub fn run(cli_args: CliArgs) {
    // Detect portable mode before anything else
    portable::init();
    install_panic_hook();

    // Parse console logging directives from RUST_LOG, falling back to info-level logging
    // when the variable is unset
    let console_filter = build_console_filter();

    let specta_builder = Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            shortcut::change_binding,
            shortcut::reset_binding,
            shortcut::shadowed_shortcuts,
            shortcut::change_shortcut_activation_setting,
            shortcut::change_hold_threshold_ms_setting,
            shortcut::change_audio_feedback_setting,
            shortcut::change_audio_feedback_volume_setting,
            shortcut::change_sound_theme_setting,
            shortcut::change_theme_setting,
            shortcut::change_start_hidden_setting,
            shortcut::change_autostart_setting,
            shortcut::change_translate_to_english_setting,
            shortcut::change_selected_language_setting,
            shortcut::change_overlay_position_setting,
            shortcut::change_bar_look_setting,
            shortcut::change_feature_setting,
            shortcut::change_island_idle_setting,
            shortcut::change_bar_opacity_setting,
            shortcut::change_overlay_style_setting,
            shortcut::change_debug_mode_setting,
            shortcut::change_word_correction_threshold_setting,
            shortcut::change_extra_recording_buffer_setting,
            shortcut::change_paste_delay_ms_setting,
            shortcut::change_paste_delay_after_ms_setting,
            shortcut::change_reliable_paste_setting,
            shortcut::change_paste_method_setting,
            shortcut::change_clipboard_handling_setting,
            shortcut::change_auto_submit_setting,
            shortcut::change_auto_submit_key_setting,
            shortcut::change_post_process_enabled_setting,
            shortcut::change_experimental_enabled_setting,
            shortcut::change_post_process_base_url_setting,
            shortcut::change_post_process_api_format_setting,
            shortcut::change_post_process_api_key_setting,
            shortcut::change_post_process_model_setting,
            shortcut::change_ai_task_model_setting,
            shortcut::set_post_process_provider,
            shortcut::fetch_post_process_models,
            shortcut::suspend_all_bindings,
            shortcut::resume_all_bindings,
            shortcut::change_background_audio_setting,
            shortcut::change_append_trailing_space_setting,
            shortcut::change_press_enter_by_voice_setting,
            shortcut::change_stacked_messages_setting,
            shortcut::change_lazy_stream_close_setting,
            shortcut::change_vad_enabled_setting,
            shortcut::change_long_form_chunking_setting,
            shortcut::change_vad_backend_setting,
            shortcut::change_filler_word_removal_enabled_setting,
            shortcut::change_chinese_script_setting,
            shortcut::change_update_checks_setting,
            shortcut::change_show_whats_new_on_update_setting,
            shortcut::change_whats_new_last_seen_version_setting,
            shortcut::change_keyboard_implementation_setting,
            shortcut::get_keyboard_implementation,
            shortcut::change_show_tray_icon_setting,
            shortcut::change_remember_app_setting,
            shortcut::change_transcribe_accelerator_setting,
            shortcut::change_transcribe_gpu_device,
            shortcut::get_available_accelerators,
            shortcut::handy_keys::start_handy_keys_recording,
            shortcut::handy_keys::stop_handy_keys_recording,
            trigger_update_check,
            show_main_window_command,
            commands::cancel_operation,
            commands::finish_dictation,
            commands::bubble::change_bubble_enabled_setting,
            commands::bubble::change_live_caption_setting,
            commands::change_admin_apps_setting,
            commands::is_elevated,
            commands::admin_apps_unavailable,
            commands::bubble::bubble_layout,
            commands::bubble::bubble_expand,
            commands::bubble::bubble_press,
            commands::bubble::bubble_action,
            commands::bubble::island_shape,
            commands::bubble::show_notes_panel,
            commands::bubble::hide_notes_panel,
            commands::connections::inyon_status,
            commands::connections::inyon_pair,
            commands::connections::inyon_unpair,
            commands::connections::inyon_set_alerts,
            commands::connections::inyon_answer,
            commands::connections::inyon_dictate,
            commands::connections::inyon_companion_dictate,
            commands::connections::inyon_agents,
            commands::connections::inyon_send,
            commands::connections::calendar_list,
            commands::connections::calendar_add,
            commands::connections::calendar_remove,
            commands::connections::calendar_refresh,
            commands::connections::google_available,
            commands::connections::calendar_connect_google,
            commands::connections::calendar_cancel_google,
            commands::connections::calendar_add_meeting,
            commands::connections::calendar_meetings,
            commands::connections::mcp_info,
            commands::connections::change_mcp_enabled_setting,
            commands::connections::mcp_add_to,
            commands::is_portable,
            commands::read_clipboard_text,
            commands::has_mica,
            commands::notify_update,
            commands::is_update_checks_locked,
            commands::get_app_dir_path,
            commands::get_app_settings,
            commands::get_default_settings,
            commands::get_log_dir_path,
            commands::set_log_level,
            commands::open_recordings_folder,
            commands::open_log_dir,
            commands::open_app_data_dir,
            commands::initialize_enigo,
            commands::initialize_shortcuts,
            commands::models::get_available_models,
            commands::models::get_model_info,
            commands::models::download_model,
            commands::models::delete_model,
            commands::models::get_retired_models,
            commands::models::delete_retired_model,
            commands::models::cancel_download,
            commands::models::set_active_model,
            commands::models::get_current_model,
            commands::models::get_transcription_model_status,
            commands::models::is_model_loading,
            commands::models::rescan_local_models,
            commands::audio::update_microphone_mode,
            commands::audio::get_microphone_mode,
            commands::audio::get_windows_microphone_permission_status,
            commands::audio::open_microphone_privacy_settings,
            commands::audio::get_available_microphones,
            commands::audio::set_selected_microphone,
            commands::audio::get_selected_microphone,
            commands::audio::set_microphone_order,
            commands::audio::get_available_output_devices,
            commands::audio::set_selected_output_device,
            commands::audio::get_selected_output_device,
            commands::audio::play_test_sound,
            commands::audio::check_custom_sounds,
            commands::audio::is_recording,
            commands::audio::get_microphone_channels,
            commands::audio::set_selected_channel,
            commands::transcription::set_model_unload_timeout,
            commands::transcription::get_model_load_status,
            commands::transcription::unload_model_manually,
            commands::history::get_history_entries,
            commands::history::get_history_summary,
            commands::history::get_insights,
            commands::history::save_history_audio,
            commands::history::get_app_icon,
            commands::audio::watch_mic_level,
            commands::history::toggle_history_entry_saved,
            commands::history::get_audio_file_path,
            commands::history::delete_history_entry,
            commands::history::retry_history_entry_transcription,
            commands::history::update_history_limit,
            commands::history::update_recording_retention_period,
            commands::dictionary::get_dictionary,
            commands::dictionary::add_dictionary_entry,
            commands::dictionary::update_dictionary_entry,
            commands::dictionary::set_dictionary_entry_pinned,
            commands::dictionary::delete_dictionary_entries,
            commands::dictionary::restore_dictionary_entries,
            commands::dictionary::test_dictionary,
            commands::dictionary::import_dictionary,
            commands::dictionary::export_dictionary,
            commands::dictionary::find_fix_candidates,
            commands::dictionary::fix_word,
            commands::dictionary::undo_fix_word,
            commands::dictionary::accept_pending_words,
            commands::dictionary::undo_accept_pending_words,
            commands::dictionary::dismiss_pending_words,
            commands::dictionary::restore_pending_words,
            commands::dictionary::add_spelling,
            commands::dictionary::add_never_learn_word,
            commands::dictionary::remove_never_learn_word,
            commands::dictionary::get_your_name,
            commands::dictionary::set_your_name,
            commands::dictionary::change_learn_from_edits_setting,
            commands::dictionary::change_learn_mode_setting,
            commands::ai::get_ai_status,
            commands::ai::test_ai_connection,
            commands::ai::get_ai_key_hints,
            commands::ai::change_ai_correction_setting,
            commands::ai::change_ai_notes_setting,
            commands::ai::change_ai_min_words_setting,
            commands::ai::change_ai_timeout_setting,
            commands::ai::change_notes_ai_timeout_setting,
            commands::transforms::get_transform_runs,
            commands::transforms::clear_transform_runs,
            commands::transforms::save_transform,
            commands::transforms::delete_transform,
            commands::transforms::restore_transform,
            commands::transforms::set_transform_enabled,
            commands::transforms::move_transform,
            commands::transforms::clear_transform_shortcut,
            commands::transforms::change_auto_transform_setting,
            commands::transforms::change_dictate_transform_setting,
            commands::transforms::change_auto_transform_skip_code_setting,
            commands::transforms::change_auto_transform_skip_ai_setting,
            commands::transforms::change_auto_transform_min_words_setting,
            commands::transforms::change_ask_enabled_setting,
            commands::transforms::change_bar_confirm_paste_setting,
            commands::transforms::change_bar_show_changes_setting,
            commands::transforms::try_transform,
            commands::transforms::new_transform,
            commands::transforms::bar_undo,
            commands::transforms::bar_keep,
            commands::transforms::bar_hold,
            commands::transforms::bar_pick,
            commands::transforms::bar_dismiss,
            commands::transforms::bar_review,
            commands::snippets::get_snippets,
            commands::snippets::add_snippet,
            commands::snippets::update_snippet,
            commands::snippets::delete_snippets,
            commands::snippets::restore_snippets,
            commands::snippets::import_snippets,
            commands::snippets::export_snippets,
            commands::snippets::test_snippets,
            commands::style::get_style_apps,
            commands::style::move_app,
            commands::style::add_site,
            commands::style::restore_app_defaults,
            commands::style::pick_window,
            commands::style::preview_style,
            commands::style::get_default_filler_words,
            commands::style::change_cleanup_level_setting,
            commands::style::change_cleanup_rules_setting,
            commands::style::change_custom_filler_words_setting,
            commands::style::change_style_enabled_setting,
            commands::style::change_category_style_setting,
            commands::scratchpad::get_scratch_notes,
            commands::scratchpad::get_scratchpad_state,
            commands::scratchpad::new_scratch_note,
            commands::scratchpad::save_scratch_note,
            commands::scratchpad::rename_scratch_note,
            commands::scratchpad::set_scratch_note_pinned,
            commands::scratchpad::dismiss_scratch_reason,
            commands::scratchpad::delete_scratch_notes,
            commands::scratchpad::restore_scratch_notes,
            commands::scratchpad::get_scratch_versions,
            commands::scratchpad::restore_scratch_version,
            commands::scratchpad::open_scratch_tab,
            commands::scratchpad::close_scratch_tab,
            commands::scratchpad::move_scratch_tab,
            commands::scratchpad::open_scratchpad,
            commands::scratchpad::get_send_targets,
            commands::scratchpad::set_send_target,
            commands::scratchpad::hide_scratchpad,
            commands::scratchpad::scratchpad_dictate,
            commands::scratchpad::send_scratch_note,
            commands::scratchpad::change_scratchpad_keep_on_top_setting,
            commands::scratchpad::change_scratchpad_open_setting,
            commands::scratchpad::change_scratchpad_failed_pastes_setting,
            commands::notes::get_notes,
            commands::notes::get_note_lines,
            commands::notes::search_notes,
            commands::notes::get_note_tags,
            commands::notes::rename_note,
            commands::notes::get_note_folders,
            commands::notes::add_note_folder,
            commands::notes::rename_note_folder,
            commands::notes::delete_note_folder,
            commands::notes::move_notes_to_folder,
            commands::notes::edit_note_line,
            commands::notes::fix_note_word,
            commands::notes::add_note_word,
            commands::notes::undo_note_word_fix,
            commands::notes::delete_note_line,
            commands::notes::restore_note_line,
            commands::notes::suggest_note_fixes,
            commands::notes::apply_note_fixes,
            commands::notes::draft_note_follow_up,
            commands::notes::find_note_speakers,
            commands::notes::name_note_speaker,
            commands::notes::get_speaker_model,
            commands::notes::download_speaker_model,
            commands::notes::cancel_speaker_model_download,
            commands::notes::delete_speaker_model,
            commands::notes::change_notes_speakers_setting,
            commands::notes::list_voices,
            commands::notes::forget_voice,
            commands::notes::change_notes_remember_voices_setting,
            commands::notes::change_notes_audio_keep_days_setting,
            commands::notes::change_notes_folder_setting,
            commands::notes::note_folder_suggestions,
            commands::notes::open_notes_folder,
            commands::notes::save_note_my_notes,
            commands::notes::add_note_tag,
            commands::notes::remove_note_tag,
            commands::notes::set_note_action_done,
            commands::notes::answer_short_note,
            commands::notes::keep_notes_recording,
            commands::notes::delete_note,
            commands::notes::restore_note,
            commands::trash::list_trash,
            commands::trash::restore_trash,
            commands::trash::delete_trash_forever,
            commands::trash::empty_trash,
            commands::notes::get_note_audio,
            commands::notes::get_note_markdown,
            commands::notes::copy_note_formatted,
            commands::notes::export_note,
            commands::notes::get_notes_audio_sources,
            commands::notes::start_notes,
            commands::notes::stop_notes,
            commands::notes::pause_notes,
            commands::notes::get_notes_session,
            commands::notes::summarise_note,
            commands::notes::summarise_note_as,
            commands::notes::ask_note,
            commands::notes::notes_what_did_i_miss,
            commands::notes::ask_all_notes,
            commands::notes::retry_note_lines,
            commands::sync::get_sync_state,
            commands::sync::start_pairing,
            commands::sync::copy_pairing_link,
            commands::sync::cancel_pairing,
            commands::sync::unpair_phone,
            commands::sync::rename_phone,
            commands::sync::sync_phone_now,
            commands::sync::get_sync_merges,
            commands::sync::change_sync_settings,
            commands::sync::send_clipboard_to_phone,
            commands::sync::get_recent_clips,
            commands::sync::clear_recent_clips,
            commands::sync::copy_recent_clip,
            commands::sync::get_clip_image,
            commands::notes::notes_dictate,
            commands::notes::open_note,
            commands::notes::change_notes_live_transcript_setting,
            commands::notes::dismiss_notes_reminder,
            commands::calls::get_call_state,
            commands::calls::change_call_settings,
            commands::calls::call_card_action,
            commands::calls::test_call_card,
            commands::setup::set_practice_mode,
            commands::setup::practice_toggle,
            commands::setup::start_mic_check,
            commands::setup::stop_mic_check,
            commands::setup::get_input_languages,
            commands::setup::change_setup_step_setting,
        ])
        .events(collect_events![
            managers::history::HistoryUpdatePayload,
            managers::dictionary::DictionaryChanged,
            ai::AiStatusChanged,
            transforms::TransformRunsChanged,
            managers::snippets::SnippetsChanged,
            OpenPage,
            scratchpad::ScratchpadChanged,
            scratchpad::ScratchpadShow,
            scratchpad::ScratchpadInsert,
            scratchpad::ScratchpadDictating,
            notes::NotesChanged,
            trash::TrashChanged,
            notes::NoteLineAdded,
            notes::NotePreview,
            notes::NoteLineChanged,
            notes::NoteLinesChanged,
            speakers::SpeakerModelChanged,
            notes::NoteLineRemoved,
            notes::NotesSessionChanged,
            notes::NotesTick,
            calls::CallsChanged,
            calendar::CalendarChanged,
            sync::SyncChanged,
            calls::CallTest,
            setup::PracticeStatus,
            setup::PracticeResult,
            setup::PracticeHint,
            setup::MicCheckLevel,
            notes::NoteInsert,
            managers::transcription::StreamTextEvent,
            managers::transcription::StreamPhaseEvent,
        ]);

    #[cfg(debug_assertions)] // <- Only export on non-release builds
    specta_builder
        .export(
            Typescript::default().bigint(BigIntExportBehavior::Number),
            "../src/bindings.ts",
        )
        .expect("Failed to export typescript bindings");

    let invoke_handler = specta_builder.invoke_handler();

    // The headless path must run as its own instance (see the single-instance
    // note below), not forward to an already-running app.
    let headless_mode =
        cli_args.transcribe_file.is_some() || cli_args.list_devices || cli_args.list_models;

    let mut builder = tauri::Builder::default()
        .device_event_filter(tauri::DeviceEventFilter::Always)
        // First, so Handy's data is in Legato's folders before the log plugin
        // creates them and before any manager reads settings.
        .plugin(handy_migration::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            LogBuilder::new()
                .level(log::LevelFilter::Trace) // Set to most verbose level globally
                // At trace these print whole frames and handshakes, Inyon's
                // token among them.
                .level_for("tungstenite", log::LevelFilter::Debug)
                .level_for("tokio_tungstenite", log::LevelFilter::Debug)
                .max_file_size(2_000_000)
                .rotation_strategy(RotationStrategy::KeepOne)
                .clear_targets()
                .targets([
                    // Console output respects RUST_LOG environment variable. In
                    // headless mode (--transcribe-file/--list-devices/--list-models)
                    // stdout carries only the result (JSON or plain), so send console
                    // logs to stderr instead to keep stdout clean for CI parsing.
                    Target::new(if headless_mode {
                        TargetKind::Stderr
                    } else {
                        TargetKind::Stdout
                    })
                    .filter({
                        let console_filter = console_filter.clone();
                        move |metadata| console_filter.enabled(metadata)
                    }),
                    // File logs respect the user's settings (stored in FILE_LOG_LEVEL atomic)
                    Target::new(if let Some(data_dir) = portable::data_dir() {
                        TargetKind::Folder {
                            path: data_dir.join("logs"),
                            file_name: Some("legato".into()),
                        }
                    } else {
                        TargetKind::LogDir {
                            file_name: Some("legato".into()),
                        }
                    })
                    .filter(|metadata| {
                        let file_level = FILE_LOG_LEVEL.load(Ordering::Relaxed);
                        metadata.level() <= level_filter_from_u8(file_level)
                    }),
                    // Stream logs to the webview (via the `log://log` event) so the
                    // debug panel's live log viewer can show them in real time. Only
                    // active while debug mode is on (its sole consumer), and shares the
                    // file log level so the "Log Level" setting controls verbosity.
                    Target::new(TargetKind::Webview).filter(|metadata| {
                        WEBVIEW_LOG_STREAMING.load(Ordering::Relaxed)
                            && metadata.level()
                                <= level_filter_from_u8(FILE_LOG_LEVEL.load(Ordering::Relaxed))
                    }),
                ])
                .build(),
        );

    // Single-instance forwards CLI args to an already-running Legato and exits.
    // That would make the headless path
    // (--transcribe-file/--list-devices/--list-models) a silent no-op whenever the
    // app is already open, so skip it in headless mode and run a standalone
    // instance instead.
    if !headless_mode {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            // The plugin calls this while the second process waits in
            // SendMessage, and WebView2 can't start inside that call: the
            // Scratchpad, built on first use, opened blank from the jump list.
            // Act once the call has returned.
            let app = app.clone();
            tauri::async_runtime::spawn(async move { handle_second_instance(&app, &args) });
        }));
    }

    let app = builder
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        // Started with Windows, Legato waits in the tray.
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .args(["--start-hidden"])
                .build(),
        )
        .plugin(window_state_plugin())
        .manage(cli_args.clone())
        .setup(move |app| {
            log::info!(
                "Vulkan layer policy: VK_LOADER_LAYERS_DISABLE={:?}, LEGATO_KEEP_VULKAN_IMPLICIT_LAYERS={}",
                std::env::var_os("VK_LOADER_LAYERS_DISABLE"),
                utils::env_flag_enabled("LEGATO_KEEP_VULKAN_IMPLICIT_LAYERS"),
            );

            specta_builder.mount_events(app);

            // Headless one-shot path (`--transcribe-file` / `--list-devices` /
            // `--list-models`): initialize only what transcription needs — the
            // store/paths plugins, the model + transcription managers, and the
            // transcribe-cpp backend + accelerator settings — then run on a worker
            // thread and exit. Deliberately skips the window, tray, overlay, audio
            // recorder (so it never opens the mic, even with always_on_microphone),
            // and autostart that initialize_core_logic sets up.
            if headless_mode {
                let app_handle = app.handle().clone();
                let model_manager = Arc::new(
                    ModelManager::new(&app_handle).expect("Failed to initialize model manager"),
                );
                let transcription_manager = Arc::new(
                    TranscriptionManager::new(&app_handle, model_manager.clone())
                        .expect("Failed to initialize transcription manager"),
                );
                managers::transcription::report_compute_devices(&transcription_manager);
                app_handle.manage(model_manager);
                app_handle.manage(transcription_manager);

                let handle = app_handle.clone();
                let args = cli_args.clone();
                std::thread::spawn(move || {
                    let code = run_headless_guarded(|| run_headless_transcription(&handle, &args));
                    // Drop the loaded engine before exit so its transcription
                    // worker is told to shut down cleanly.
                    if let Some(tm) = handle.try_state::<Arc<TranscriptionManager>>() {
                        let _ = tm.unload_model();
                    }
                    // process::exit (not app.exit, which exits 0 regardless) so the
                    // exit code propagates to the shell for CI gating. Flush first
                    // since process::exit runs no destructors / buffer flushes.
                    use std::io::Write;
                    let _ = std::io::stdout().flush();
                    let _ = std::io::stderr().flush();
                    std::process::exit(code);
                });
                return Ok(());
            }

            // Working in admin apps: a Legato that isn't elevated hands over
            // to the one the task starts, before it opens anything.
            {
                let settings = settings::get_settings(app.handle());
                let hidden = settings.start_hidden || cli_args.start_hidden;
                if elevation::hand_over_at_startup(app.handle(), hidden) {
                    std::process::exit(0);
                }
            }

            // Before the first window, so the taskbar groups it under the
            // Start menu shortcut and its jump list.
            desktop::set_app_id(app.handle());
            desktop::set_jump_list(app.handle());
            desktop::raise_priority();

            // Create main window programmatically so we can set data_directory
            // for portable mode (redirects WebView2 cache to portable Data dir)
            let mut win_builder =
                tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::App("/".into()))
                    .title("Legato")
                    .inner_size(MAIN_WINDOW_SIZE.0, MAIN_WINDOW_SIZE.1)
                    .min_inner_size(MAIN_WINDOW_MIN_SIZE.0, MAIN_WINDOW_MIN_SIZE.1)
                    .decorations(false)
                    .resizable(true)
                    .maximizable(true)
                    .visible(false);

            if let Some(data_dir) = portable::data_dir() {
                win_builder = win_builder.data_directory(data_dir.join("webview"));
            }

            // Mica behind the sidebar and title bar on Windows 11; the page
            // keeps its content solid. Windows 10 keeps an opaque window.
            if desktop::supports_mica() {
                win_builder = win_builder.transparent(true).effects(
                    tauri::window::EffectsBuilder::new()
                        .effect(tauri::window::Effect::Mica)
                        .build(),
                );
                desktop::set_mica(true);
            }

            let main_window = win_builder.build()?;
            fit_window_to_screen(&main_window, MAIN_WINDOW_MIN_SIZE);

            // Disable WebView2 browser accelerators (F5, F6, Ctrl+F, F12, ...).
            // A settings window has no use for them, and pressing F6 while
            // recording a shortcut was reported to turn the whole window white
            // (cjpais/Handy#1940), likely by triggering WebView2 focus cycling.
            // DevTools stays enabled; only the F12 accelerator is lost.
            let _ = main_window.with_webview(|webview| unsafe {
                use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings3;
                use windows::core::Interface;

                let result = webview
                    .controller()
                    .CoreWebView2()
                    .and_then(|core| core.Settings())
                    .and_then(|settings| settings.cast::<ICoreWebView2Settings3>())
                    .and_then(|settings| settings.SetAreBrowserAcceleratorKeysEnabled(false));

                if let Err(error) = result {
                    log::warn!("Failed to disable WebView2 browser accelerators: {error}");
                }
            });

            let mut settings = get_settings(app.handle());

            // Apply the persisted appearance theme to the native title bar before
            // the window is shown, so it matches the in-app palette without a flash
            // of the wrong theme. See `apply_window_theme`.
            shortcut::apply_window_theme(app.handle(), settings.theme);

            // CLI --debug flag overrides debug_mode and log level (runtime-only, not persisted)
            if cli_args.debug {
                settings.debug_mode = true;
                settings.log_level = settings::LogLevel::Trace;
            }

            let tauri_log_level: tauri_plugin_log::LogLevel = settings.log_level.into();
            let file_log_level: log::Level = tauri_log_level.into();
            // Store the file log level in the atomic for the filter to use
            FILE_LOG_LEVEL.store(file_log_level.to_level_filter() as u8, Ordering::Relaxed);
            // Only forward logs to the webview while debug mode is on (the live log
            // viewer is the sole consumer and only exists in debug mode). This also
            // honors the runtime `--debug` override applied to `settings` above.
            WEBVIEW_LOG_STREAMING.store(settings.debug_mode, Ordering::Relaxed);
            let app_handle = app.handle().clone();
            app.manage(TranscriptionCoordinator::new(app_handle.clone()));

            initialize_core_logic(&app_handle);

            // Populate the overlay-enabled cache from initial settings so the
            // audio path (overlay::emit_levels, called ~24 Hz during recording)
            // can do a single atomic load instead of reading the Tauri store.
            // Kept in sync by shortcut::change_overlay_style_setting.
            overlay::update_overlay_enabled_cache(
                settings.overlay_style != settings::OverlayStyle::None,
            );

            // Pre-warm GPU/accelerator enumeration on a background thread. The first
            // device listing opens the GPU, so it stays off the startup path. Then
            // get_available_accelerators enumerates ORT execution providers and
            // transcribe-cpp compute devices; without this the cost is paid
            // synchronously when the user first opens Advanced settings, freezing
            // the UI. The device list is kept by the transcription engine and
            // refreshed by every worker it starts that may use the GPU (CPU-only
            // workers can't see it).
            let devices_app_handle = app_handle.clone();
            std::thread::spawn(move || {
                let tm = devices_app_handle.state::<Arc<TranscriptionManager>>();
                crate::managers::transcription::report_compute_devices(&tm);
                let _ = crate::managers::transcription::get_available_accelerators(&tm);
            });

            // Hide tray icon if --no-tray was passed
            if cli_args.no_tray {
                tray::set_tray_visibility(&app_handle, false);
            }

            // Show main window only if not starting hidden.
            // CLI --start-hidden flag overrides the setting.
            // But if permission onboarding is required, always show the window.
            // The jump list's Scratchpad and New note open just the
            // Scratchpad when Legato wasn't running.
            let scratchpad_only = cli_args.open_scratchpad || cli_args.new_note;
            let should_hide = settings.start_hidden
                || cli_args.start_hidden
                || scratchpad_only
                || elevation::take_hidden_marker(&app_handle);
            let should_force_show = should_force_show_permissions_window(&app_handle);

            // If start_hidden but tray is disabled, we must show the window
            // anyway. Without a tray icon there would be no way back in.
            let tray_available = settings.show_tray_icon && !cli_args.no_tray;
            if should_force_show || !should_hide || !tray_available {
                show_main_window(&app_handle);
            }
            if scratchpad_only {
                open_scratchpad_from_cli(&app_handle, cli_args.new_note);
            }

            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _res = window.hide();
            }
            // Moved to a screen with another scale, or the scale changed.
            tauri::WindowEvent::ScaleFactorChanged { .. } => match window.label() {
                "recording_overlay" => overlay::update_overlay_position(window.app_handle()),
                "main" => {
                    if let Some(w) = window.app_handle().get_webview_window("main") {
                        fit_window_to_screen(&w, MAIN_WINDOW_MIN_SIZE);
                    }
                    bubble::screens_changed(window.app_handle());
                }
                label if label == scratchpad::WINDOW => {
                    if let Some(w) = window.app_handle().get_webview_window(label) {
                        fit_window_to_screen(&w, scratchpad::WINDOW_MIN);
                    }
                }
                _ => {}
            },
            tauri::WindowEvent::ThemeChanged(theme) => {
                log::info!("Theme changed to: {:?}", theme);
                // Re-apply the current tray state with the new theme's icon set
                utils::refresh_tray_icon(window.app_handle());
            }
            _ => {}
        })
        .invoke_handler(invoke_handler)
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    // No transcription teardown on exit: transcribe.cpp runs only in the
    // worker process, which exits by itself as soon as this process's end
    // of its stdin closes, even if it is hung. Waiting on an unload here
    // could block quitting behind a hung model load.
    // A clean exit leaves this line, so a log that stops without it means
    // the process was ended from outside or crashed.
    app.run(|_, event| {
        if let tauri::RunEvent::Exit = event {
            log::info!("Legato is exiting.");
            EXITING.store(true, Ordering::SeqCst);
        }
    });
}

#[cfg(test)]
mod headless_guard_tests {
    use super::run_headless_guarded;

    #[test]
    fn preserves_normal_exit_codes() {
        assert_eq!(run_headless_guarded(|| 2), 2);
    }

    #[test]
    fn converts_worker_panics_to_runtime_failures() {
        assert_eq!(run_headless_guarded(|| panic!("simulated failure")), 1);
    }
}
