# AGENTS.md

How this codebase is laid out, how to build and check it, and the conventions to follow.

## Development Commands

**Prerequisites:**

- [Rust](https://rustup.rs/) (latest stable)
- [Bun](https://bun.sh/) package manager

**Core Development:**

```bash
# Install dependencies
bun install

# Run in development mode
bun run tauri dev

# Build for production
bun run tauri build

# Frontend only development
bun run dev        # Start Vite dev server
bun run build      # Build frontend (TypeScript + Vite)
bun run preview    # Preview built frontend
```

**Linting and Formatting (run before committing):**

```bash
bun run lint              # ESLint for frontend
bun run lint:fix          # ESLint with auto-fix
bun run format            # Prettier + cargo fmt
bun run format:check      # Check formatting without changes
bun run format:frontend   # Prettier only
bun run format:backend    # cargo fmt only
```

For the Windows build setup (Visual Studio C++ tools, CMake, Vulkan SDK), see [BUILD.md](BUILD.md).

## Architecture Overview

Legato is a Windows desktop speech-to-text application built with Tauri 2.x (Rust backend + React/TypeScript frontend).

### Backend Structure (src-tauri/src/)

- `lib.rs` - Main entry point, Tauri setup, manager initialization
- `managers/` - Core business logic:
  - `audio.rs` - Audio recording and device management
  - `model.rs` - Model downloading and management
  - `transcription.rs` - Speech-to-text processing pipeline
  - `history.rs` - Transcription history storage
- `audio_toolkit/` - Low-level audio processing:
  - `audio/` - Recording sink (VAD, levels, live streaming feed), output devices, WAV I/O
  - `vad/` - Voice Activity Detection (Silero VAD)
- `commands/` - Tauri command handlers for frontend communication
- `cli.rs` - CLI argument definitions (clap derive)
- `shortcut/` - Global keyboard shortcut handling
- `settings.rs` - Application settings management
- `overlay.rs` - Recording overlay window
- `signal_handle.rs` - `send_transcription_input()` reusable function
- `utils.rs` - Shared helpers (operation cancellation, env flags, log redaction, x64-on-ARM64 detection)

### Frontend Structure (src/)

- `App.tsx` - Main component with onboarding flow
- `components/` - React UI components:
  - `settings/` - Settings UI
  - `model-selector/` - Model management interface
  - `onboarding/` - First-run experience
  - `update-checker/` - App update notifications
  - `shared/`, `ui/`, `icons/`, `footer/` - Shared components
- `hooks/useSettings.ts` - Settings state management hook
- `stores/settingsStore.ts` - Zustand store for settings
- `bindings.ts` - Auto-generated Tauri type bindings (via tauri-specta)
- `overlay/` - Recording overlay window entry point
- `lib/types/` - Shared TypeScript type definitions

### Key Architecture Patterns

**Manager Pattern:** Core functionality organized into managers (Audio, Model, Transcription) initialized at startup and managed via Tauri state.

**Command-Event Architecture:** Frontend → Backend via Tauri commands; Backend → Frontend via events.

**Pipeline Processing:** Audio → VAD → Whisper/Parakeet → Text output → Clipboard/Paste

**State Flow:** Zustand → Tauri Command → Rust State → Persistence (tauri-plugin-store)

### Technology Stack

**Core Libraries:**

- `transcribe-cpp` - Local speech recognition (Whisper, Parakeet, Moonshine, SenseVoice, etc.; GGML/GGUF) with GPU acceleration
- `handy-recorder` - Microphone capture: device selection, resampling to 16 kHz, failure detection
- `transcribe-cpp` (Silero VAD v6, CPU-only, in-process) / `earshot` - Voice Activity Detection
- `handy-keys` / `tauri-plugin-global-shortcut` - Global keyboard shortcuts
- `rodio` - Audio playback for feedback sounds

### Application Flow

1. **Initialization:** App starts minimized to tray, loads settings, initializes managers
2. **Model Setup:** First-run downloads preferred Whisper model (Small/Medium/Turbo/Large)
3. **Recording:** Global shortcut triggers audio recording with VAD filtering
4. **Processing:** Audio sent to Whisper model for transcription
5. **Output:** Text pasted to active application via system clipboard

### Settings System

Settings are stored using Tauri's store plugin with reactive updates:

- Keyboard shortcuts (configurable, supports push-to-talk)
- Audio devices (microphone/output selection)
- Model preferences (Small/Medium/Turbo/Large Whisper variants)
- Audio feedback and translation options

### Single Instance Architecture

The app enforces single instance behavior — launching when already running brings the settings window to front rather than creating a new process. Remote control flags (`--toggle-transcription`, etc.) work by launching a second instance that sends args to the running instance via `tauri_plugin_single_instance`, then exits.

## UI Strings

The UI is English only. All user-facing strings still go through i18next, and ESLint enforces this (no hardcoded strings in JSX).

**Adding new text:**

1. Add key to `src/i18n/locales/en/translation.json`
2. Use in component: `const { t } = useTranslation(); t('key.path')`

## Code Style

**Rust:**

- Run `cargo fmt` and `cargo clippy` before committing
- Handle errors explicitly (avoid unwrap in production)
- Use descriptive names, add doc comments for public APIs

**TypeScript/React:**

- Strict TypeScript, avoid `any` types
- Functional components with hooks
- Tailwind CSS for styling
- Path aliases: `@/` → `./src/`

## CLI Parameters

Legato supports command-line parameters for integration with scripts, hotkey tools, and autostart configurations.

**Implementation:** `cli.rs` (definitions), `main.rs` (parsing), `lib.rs` (applying), `signal_handle.rs` (shared logic)

| Flag                     | Description                                                |
| ------------------------ | ---------------------------------------------------------- |
| `--toggle-transcription` | Toggle recording on/off on a running instance              |
| `--toggle-post-process`  | Toggle recording with post-processing on/off               |
| `--cancel`               | Cancel the current operation on a running instance         |
| `--start-hidden`         | Launch without showing the main window (tray icon visible) |
| `--no-tray`              | Launch without system tray (closing window quits the app)  |
| `--debug`                | Enable debug mode with verbose (Trace) logging             |

**Key design decisions:**

- CLI flags are runtime-only overrides — they do NOT modify persisted settings
- Remote control flags work via `tauri_plugin_single_instance`: second instance sends args, then exits
- `send_transcription_input()` in `signal_handle.rs` is the shared entry point for CLI flags and other external triggers

## Debug Mode

Access debug features with `Ctrl+Shift+D`.

## Platform Notes

Legato targets Windows only, on x64 and ARM64.

- x64 builds use Vulkan GPU acceleration; ARM64 builds are CPU-only (see the per-architecture dependency tables in `src-tauri/Cargo.toml`)
- Release installers are code-signed through the `signCommand` in `src-tauri/tauri.conf.json`
- Implicit Vulkan layers (overlays, capture hooks) are disabled for the Legato process via `VK_LOADER_LAYERS_DISABLE=~implicit~` set in `main.rs`; opt out with `LEGATO_KEEP_VULKAN_IMPLICIT_LAYERS=1` or by setting `VK_LOADER_LAYERS_DISABLE` yourself

## Environment Variables

Legato reads these `LEGATO_*` variables. Handy's `HANDY_*` names are not read any more.

| Variable                             | Read by              | Purpose                                                                                    |
| ------------------------------------ | -------------------- | ------------------------------------------------------------------------------------------ |
| `LEGATO_KEEP_VULKAN_IMPLICIT_LAYERS` | app                  | Keep implicit Vulkan layers enabled (see Platform Notes)                                   |
| `LEGATO_DISABLE_UPDATER`             | app                  | Force update checks off without touching the persisted setting; the toggle shows as locked |
| `LEGATO_VC_REDIST_DIRS`              | `build.rs`           | VC++ redist dirs (`;`-separated) whose runtime DLLs are staged app-local; set by CI        |
| `LEGATO_TRANSCRIBE_WORKER_LOG`       | transcription worker | Log level the app hands to its worker process; set internally                              |
| `LEGATO_DEBUG_MIC_READY_DELAY_MS`    | debug builds         | Delay the microphone-ready signal to preview the arming animation                          |
| `LEGATO_FORCE_TRANSCRIPTION_FAILURE` | debug builds         | Make every transcription fail, to test the error path                                      |
| `LEGATO_WORKER_FAULT`                | debug builds         | Inject a crash or hang into the worker at a given stage (`<abort\|segv\|hang>@<stage>`)    |
| `LEGATO_WORKER_FAULT_ONCE`           | debug builds         | Marker file path so the injected fault fires only once                                     |
| `LEGATO_WORKER_FAULT_GPU_ONLY`       | debug builds         | Fire the injected fault only on GPU work                                                   |
| `LEGATO_TRANSCRIBE_WORKER_EXE`       | ignored worker tests | Built app to launch as the worker under `cargo test`                                       |
| `LEGATO_TEST_MODEL`                  | ignored worker tests | GGUF model for the end-to-end worker tests                                                 |
| `LEGATO_TEST_WAV`                    | ignored worker tests | 16 kHz mono WAV for the end-to-end worker tests                                            |

## Troubleshooting

See the [Troubleshooting](README.md#troubleshooting) section in README.md.

## Commits

Use conventional commit prefixes (`feat:`, `fix:`, `docs:`, `refactor:`, `chore:`). Focus the message on _why_, not _what_.
