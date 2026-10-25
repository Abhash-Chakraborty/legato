//! Settings › Connections: calendars, Inyon, and AI apps reading Legato
//! over MCP.

use crate::calendar::{self, CalendarStatus, RemovedCalendar};
use crate::calendar_rules::Meeting;
use crate::settings::{get_settings, write_settings};
use serde::Serialize;
use serde_json::{json, Value};
use specta::Type;
use std::path::PathBuf;
use tauri::AppHandle;

#[tauri::command]
#[specta::specta]
pub fn inyon_status(app: AppHandle) -> crate::inyon::InyonStatus {
    crate::inyon::status(&app)
}

/// Pairs with an Inyon hub from the code its Settings › Paired devices
/// shows. Errors are keys under `inyon.reason`.
#[tauri::command]
#[specta::specta]
pub async fn inyon_pair(
    app: AppHandle,
    hub: String,
    code: String,
) -> Result<crate::inyon::InyonStatus, String> {
    crate::inyon::pair(&app, &hub, &code).await
}

#[tauri::command]
#[specta::specta]
pub fn inyon_unpair(app: AppHandle) {
    crate::inyon::unpair(&app);
}

/// Agents waiting for you on the bar or the island, or not.
#[tauri::command]
#[specta::specta]
pub fn inyon_set_alerts(app: AppHandle, on: bool) {
    crate::inyon::set_alerts(&app, on);
}

/// Answers the agent on the card with one of its options. Errors are keys
/// under `inyon.reason`.
#[tauri::command]
#[specta::specta]
pub async fn inyon_answer(
    app: AppHandle,
    host: String,
    pane: String,
    option: u32,
    key: Option<String>,
) -> Result<(), String> {
    crate::inyon::answer(&app, &host, &pane, option, key, None).await
}

/// Agent sessions on the paired hub, for the Scratchpad's Send to.
#[tauri::command]
#[specta::specta]
pub fn inyon_agents() -> Vec<crate::inyon::InyonAgent> {
    crate::inyon::agents()
}

/// Sends a Scratchpad note to an agent's session; Ok with how far it got.
#[tauri::command]
#[specta::specta]
pub async fn inyon_send(
    app: AppHandle,
    host: String,
    pane: String,
    text: String,
) -> Result<String, String> {
    let pane = crate::inyon_rules::PaneRef { host, pane };
    crate::inyon::send_text(&app, &pane, &text).await
}

/// The mic on an agent's card: answer it by voice.
#[tauri::command]
#[specta::specta]
pub fn inyon_dictate(app: AppHandle, host: String, pane: String) {
    crate::inyon::dictate(&app, &host, &pane);
}

/// Reply on the companion's card: speak to Inyon's companion agent.
#[tauri::command]
#[specta::specta]
pub fn inyon_companion_dictate(app: AppHandle) {
    crate::inyon::talk_to_companion(&app);
}

#[tauri::command]
#[specta::specta]
pub fn calendar_list(app: AppHandle) -> Vec<CalendarStatus> {
    calendar::statuses(&app)
}

/// Adds a calendar by its secret iCal address, after fetching it once.
#[tauri::command]
#[specta::specta]
pub async fn calendar_add(
    app: AppHandle,
    name: String,
    url: String,
) -> Result<CalendarStatus, String> {
    calendar::add(&app, &name, &url).await
}

/// Removes a calendar; the address comes back for Undo.
#[tauri::command]
#[specta::specta]
pub fn calendar_remove(app: AppHandle, id: String) -> Result<RemovedCalendar, String> {
    calendar::remove(&app, &id)
}

#[tauri::command]
#[specta::specta]
pub fn calendar_refresh() {
    calendar::refresh_soon();
}

/// Meetings from a day back to two weeks ahead, by start.
#[tauri::command]
#[specta::specta]
pub fn calendar_meetings(app: AppHandle) -> Vec<Meeting> {
    calendar::meetings(&app)
}

/// An AI app Legato can add itself to.
#[derive(Clone, Debug, Serialize, Type)]
pub struct McpClient {
    pub id: String,
    pub name: String,
    /// Legato is already in its configuration.
    pub added: bool,
}

#[derive(Clone, Debug, Serialize, Type)]
pub struct McpInfo {
    pub enabled: bool,
    /// The configuration any MCP app takes, to copy.
    pub config: String,
    /// AI apps found on this PC.
    pub clients: Vec<McpClient>,
}

fn command() -> String {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "legato.exe".into())
}

fn server() -> Value {
    json!({ "command": command(), "args": ["--mcp"] })
}

/// Where each AI app keeps its MCP servers, when the app is installed.
fn client_files() -> Vec<(&'static str, &'static str, PathBuf)> {
    let mut out = Vec::new();
    let appdata = std::env::var_os("APPDATA").map(PathBuf::from);
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let home = std::env::var_os("USERPROFILE").map(PathBuf::from);
    // Claude from the installer, or from the Microsoft Store.
    let claude = appdata
        .as_ref()
        .map(|a| a.join("Claude"))
        .filter(|d| d.is_dir())
        .or_else(|| {
            let packages = local.as_ref()?.join("Packages");
            std::fs::read_dir(packages)
                .ok()?
                .flatten()
                .find(|e| e.file_name().to_string_lossy().starts_with("Claude_"))
                .map(|e| e.path().join("LocalCache").join("Roaming").join("Claude"))
                .filter(|d| d.is_dir())
        });
    if let Some(dir) = claude {
        out.push(("claude", "Claude", dir.join("claude_desktop_config.json")));
    }
    if let Some(dir) = home.map(|h| h.join(".cursor")).filter(|d| d.is_dir()) {
        out.push(("cursor", "Cursor", dir.join("mcp.json")));
    }
    out
}

fn read_json(path: &PathBuf) -> Result<Value, String> {
    match std::fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => Ok(json!({})),
        Ok(text) => serde_json::from_str(&text)
            .map_err(|_| "Its settings file isn't valid JSON; fix it there first".to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
#[specta::specta]
pub fn mcp_info(app: AppHandle) -> McpInfo {
    let clients = client_files()
        .into_iter()
        .map(|(id, name, path)| McpClient {
            id: id.into(),
            name: name.into(),
            added: read_json(&path)
                .ok()
                .is_some_and(|v| v["mcpServers"]["legato"].is_object()),
        })
        .collect();
    McpInfo {
        enabled: get_settings(&app).mcp_enabled,
        config: serde_json::to_string_pretty(&json!({ "mcpServers": { "legato": server() } }))
            .unwrap_or_default(),
        clients,
    }
}

#[tauri::command]
#[specta::specta]
pub fn change_mcp_enabled_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.mcp_enabled = enabled;
    write_settings(&app, settings);
    Ok(())
}

/// Puts Legato in an AI app's MCP servers, keeping everything else there.
/// The app picks it up when it restarts.
#[tauri::command]
#[specta::specta]
pub fn mcp_add_to(id: String) -> Result<(), String> {
    let (_, _, path) = client_files()
        .into_iter()
        .find(|(c, _, _)| *c == id)
        .ok_or("That app isn't installed")?;
    let mut config = read_json(&path)?;
    if !config.is_object() {
        return Err("Its settings file isn't valid JSON; fix it there first".into());
    }
    if !config["mcpServers"].is_object() {
        config["mcpServers"] = json!({});
    }
    config["mcpServers"]["legato"] = server();
    let text = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("Couldn't write {}: {e}", path.display()))
}
