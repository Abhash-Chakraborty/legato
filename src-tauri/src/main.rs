// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clap::Parser;
use legato_app_lib::CliArgs;

fn main() {
    // Avoid overlay/capture layer crashes (#2049). Set before backend
    // initialization, preserving user overrides.
    if std::env::var_os("VK_LOADER_LAYERS_DISABLE").is_none()
        && !legato_app_lib::env_flag_enabled("LEGATO_KEEP_VULKAN_IMPLICIT_LAYERS")
    {
        std::env::set_var("VK_LOADER_LAYERS_DISABLE", "~implicit~");
    }

    // Transcription worker: runs transcribe.cpp in isolation and never touches
    // Tauri, the CLI, or single-instance handling. Checked after the env setup
    // above so the worker inherits it (e.g. the Vulkan layer opt-out).
    if legato_app_lib::engine_supervisor::is_worker_invocation() {
        std::process::exit(legato_app_lib::engine_supervisor::run_worker());
    }

    // The uninstaller tidying up before it deletes the exe.
    if let Some(code) = legato_app_lib::uninstall_cleanup_invocation() {
        std::process::exit(code);
    }

    // `legato --mcp`: an AI app on this PC asking Legato's data over stdio.
    // Its own process too, so it never starts or reaches the running app.
    if legato_app_lib::mcp::is_invocation() {
        std::process::exit(legato_app_lib::mcp::run());
    }

    let cli_args = CliArgs::parse();
    legato_app_lib::run(cli_args)
}
