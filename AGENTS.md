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

- `lib.rs` - Main entry point, Tauri setup, manager initialization, window events (DPI refit)
- `managers/` - Core business logic and the SQLite stores:
  - `audio.rs` - Audio recording and device management
  - `model.rs` - Model downloading and management
  - `transcription.rs` - Speech-to-text processing pipeline (loads the model Style picks per kind of app; a recording that ends while the model is still loading onto the GPU is transcribed on the CPU meanwhile)
  - `history.rs` / `history_store.rs` - Transcription history, Home's queries, daily statistics for Insights
  - `dictionary.rs` / `dictionary_store.rs` - The Dictionary: words, heard-as forms, learned and spelled-differently suggestions
  - `snippets.rs` / `snippet_store.rs` - Snippets (trigger phrase → saved text)
  - `transform_store.rs` - Transform runs and usage
  - `notes_store.rs` - Notes recordings, transcript lines, tags, action items
  - `scratch_store.rs` - Scratchpad notes and tabs
- `engine_supervisor/` - Runs transcribe.cpp in a worker process, plus short-lived ones beside it (speaker detection, a CPU transcription during a GPU load)
- `audio_toolkit/` - Low-level audio processing:
  - `audio/` - Recording sink (VAD, levels, live streaming feed), output devices, WAV I/O
  - `vad/` - Voice Activity Detection (Silero VAD)
- `commands/` - Tauri command handlers for frontend communication, one file per page or feature
- `sync/` - Phone sync and the shared clipboard (protocol in [docs/phone-sync.md](docs/phone-sync.md)):
  - `protocol.rs` / `identity.rs` - Frames, messages, the pairing link; this PC's certificate and pinned TLS
  - `ledger.rs` / `kinds.rs` - The change ledger, peers, merges and clips in history.db; export and apply per kind
  - `engine.rs` / `link.rs` / `hub.rs` - Merging, connections and pairing, free of Tauri so the phone side (`phone/`) reuses them
  - `mod.rs` / `clipboard.rs` / `net.rs` - Windows glue: `SyncManager`, the clipboard watcher, this PC's addresses
  - `bluetooth.rs` - The RFCOMM listener and SDP record for a phone that shares no network with the PC
- `actions.rs` - What each shortcut does; the dictation pipeline from recording to paste
- `*_rules.rs` - Pure logic kept free of Tauri so it can be unit tested on any OS: `cleanup_rules`, `style_rules`, `snippet_rules`, `transform_rules`, `learn_rules`, `notes_rules`, `calls_rules`, `ai_rules`, `bubble_rules`, `calendar_rules`, `mcp_rules`
- `transforms.rs` - Transforms on the selection or last dictation, the keyboard picker, Ask, the bar's Keep/Undo
- `clipboard.rs` / `paste_tx/` - Pasting, per-category paste overrides, remembering the last paste
- `selection.rs` - Copying the selection from another app; Add selection to Dictionary
- `edit_reader.rs` - Learn from my edits (UI Automation read after a paste)
- `scratchpad.rs` - The Scratchpad window, failed pastes, Send to
- `notes.rs` / `loopback.rs` - Notes: voice memos and meetings (per-app loopback capture), the live preview of the line being spoken, line corrections with Undo, and the AI's misheard-word fixes and follow-up drafts
- `speakers.rs` - Telling voices apart in Notes: the speaker model (downloaded on request), passes during and after a recording, and voices saved when you name someone, recognised in later meetings
- `notes_panel.rs` - The Notes side panel docked at the right of the screen (live transcript, My notes, Pause, Stop)
- `calls.rs` - Noticing calls and offering notes
- `calendar.rs` - Calendars from secret iCal addresses (Settings › Connections): meetings for the bubble, note titles and MCP, cached in `calendar.json`
- `mcp.rs` - `legato --mcp`: a Model Context Protocol server over stdio that lets AI apps read dictations, notes, the Dictionary, the Scratchpad and meetings
- `context.rs` - Knowing the app in front (name, category, icon) and listing open windows
- `elevation.rs` - Work in apps run as administrator: the scheduled task that starts Legato elevated, and handing over to it at startup
- `desktop.rs` - Windows shell integration: app ID, jump list, notifications (Do not disturb aware), Mica
- `setup.rs` - First-run setup: microphone check, model choice, practice scenes
- `ai.rs` / `llm_client.rs` / `secrets.rs` - Your own AI provider; API keys in Windows Credential Manager
- `cli.rs` - CLI argument definitions (clap derive)
- `shortcut/` - Global keyboard shortcut handling (handy-keys or Tauri), including the picker's temporary keys
- `settings.rs` - Application settings management
- `overlay.rs` - The Legato bar (recording overlay window and its states, including the step a dictation is on after you stop talking, which it also names while the bubble has the dictation), and the live transcript box it shows at the top or bottom of the screen while the bubble has a dictation (Settings › Bar)
- `bubble.rs` - The desktop bubble: a dot that stays on screen with Dictate, Notes and the Scratchpad on hover; dragged, it settles in the middle of the nearest edge; steps aside for fullscreen windows. While it shows, a dictation's states go to it instead of the bar (`overlay.rs`), on its own `bubble-dictation` event; hovered during one, Dictate becomes Done (then ripples while the words are written up) and Cancel replaces the Scratchpad
- `tray.rs` - System tray icon and menu
- `signal_handle.rs` - `send_transcription_input()` reusable function
- `handy_migration.rs` - First-run move of Handy's data into Legato's folders
- `utils.rs` - Shared helpers (operation cancellation, env flags, log redaction, x64-on-ARM64 detection)

### Frontend Structure (src/)

- `App.tsx` - Main window shell, setup flow, Mica background
- `components/` - React UI components:
  - `pages/` - The sidebar pages: `home`, `dictionary`, `snippets`, `style`, `transforms`, `notes`, `scratchpad`, `insights`
  - `settings/` - Settings sections (`devices/` for Settings › Devices)
  - `setup/` - First-run setup with practice scenes
  - `model-selector/` - Model management interface
  - `scratchpad/` - Shared Scratchpad parts (preview, note status, version history, Markdown helpers)
  - `notes/` - Shared Notes components
  - `update-checker/` - App update notifications
  - `shared/`, `ui/`, `icons/`, `shell/`, `whats-new/` - Shared components
- `overlay/` - The Legato bar window entry point
- `bubble/` - The desktop bubble window
- `notespanel/` - The Notes side panel window
- `scratchpad/` - The Scratchpad window: tabs, the notes rail and panel, formatting bar, Copy and Send to
- `stores/` - Zustand stores (settings, models, dictionary, snippets, transforms, notes, calls, scratchpad, sync, navigation)
- `hooks/useSettings.ts` - Settings state management hook
- `bindings.ts` - Tauri type bindings (generated by tauri-specta; kept in step by hand when commands change)
- `i18n/locales/en/translation.json` - All UI strings; the `tray`, `handyMigration` and `desktop` sections also feed Rust through `build.rs`
- `lib/types/` - Shared TypeScript type definitions

### Key Architecture Patterns

**Manager Pattern:** Core functionality organized into managers (Audio, Model, Transcription) initialized at startup and managed via Tauri state.

**Command-Event Architecture:** Frontend → Backend via Tauri commands; Backend → Frontend via events.

**Pipeline Processing:** Audio → VAD → speech model → Dictionary/snippets → Cleanup and Style → optional transform → Clipboard/Paste

**State Flow:** Zustand → Tauri Command → Rust State → Persistence (tauri-plugin-store)

### Technology Stack

**Core Libraries:**

- `transcribe-cpp` - Local speech recognition (Whisper, Parakeet, Moonshine, SenseVoice, etc.; GGML/GGUF) with GPU acceleration
- `handy-recorder` - Microphone capture: device selection, resampling to 16 kHz, failure detection
- `transcribe-cpp` (Silero VAD v6, CPU-only, in-process) / `earshot` - Voice Activity Detection
- `handy-keys` / `tauri-plugin-global-shortcut` - Global keyboard shortcuts
- `rodio` - Audio playback for feedback sounds

### Application Flow

1. **Initialization:** App starts minimized to tray, loads settings, initializes managers, writes the jump list
2. **Setup:** First run checks the microphone, picks a model from your Windows input languages, and practises in copies of your apps
3. **Recording:** Global shortcut triggers audio recording with VAD filtering; the app in front decides the speech model and style
4. **Processing:** Audio goes to the speech model in a worker process, then through the Dictionary, snippets, cleanup and Style
5. **Output:** Text pasted to active application via system clipboard; a failed paste lands in the Scratchpad

### Settings System

Settings are stored using Tauri's store plugin with reactive updates:

- Keyboard shortcuts (configurable, supports push-to-talk)
- Audio devices (microphone/output selection)
- Speech model, plus an optional model per kind of app in Style
- Style per kind of app: tone, paste keys, Enter after pasting, trailing space
- Transforms, Ask and your AI provider; Notes, calls, Scratchpad and learning options
- Connections: calendars (their addresses in Credential Manager) and whether AI apps may read Legato over MCP

### Single Instance Architecture

The app enforces single instance behavior — launching when already running brings the settings window to front rather than creating a new process. Remote control flags (`--toggle-transcription`, etc.) work by launching a second instance that sends args to the running instance via `tauri_plugin_single_instance`, then exits.

## Phone Sync

Settings › Devices pairs Legato for Android by QR code and syncs over Wi-Fi (or Netbird) when they share a network and over Bluetooth when they don't, with mutual TLS and no server in between. The wire protocol, merge rules and what syncs are written down in [docs/phone-sync.md](docs/phone-sync.md); keep it current when the protocol changes.

The phone side lives in `phone/`, a Cargo workspace outside the app's build:

- `phone/core` - `legato-phone`, the library both the Android app and the stand-in share, built from the same `src-tauri/src/sync` files, plus the Dictionary and snippet stores and the text rules (`audio_toolkit/text.rs`, `cleanup_rules`, `snippet_rules`, `style_rules`, `learn_rules`) so text from the phone's own model is finished as the PC finishes a dictation; its `stand-in-phone` binary is a Linux/Windows CLI that plays the phone
- `phone/android` - `legato-android`, the UniFFI bindings the Android app calls (Kotlin bindings are generated in CI)
- `android/` - the Android app (Kotlin, Jetpack Compose); it builds in GitHub Actions (`android.yml`), since Google ships the Android build tools for x86-64 only
  - It follows the phone mockup, not stock Material: Legato's tokens in `ui/Theme.kt` (no dynamic colour), Figtree in `res/font` (OFL, licence in `android/third_party`), shared rows, menus and status colours in `ui/Parts.kt`
  - To build an APK without touching `main`, push to another branch and run `gh workflow run android --ref <branch>`

```bash
cd phone
cargo test -p legato-phone                   # unit tests plus an end-to-end PC ↔ phone run
cargo run --bin stand-in-phone -- pair 'legato://pair?...'   # pair with a running Legato (Copy link in the dialog)
cargo run --bin stand-in-phone -- connect    # reconnect later; type `help` for commands
```

## UI Strings

The UI is English only. All user-facing strings still go through i18next, and ESLint enforces this (no hardcoded strings in JSX).

**Adding new text:**

1. Add key to `src/i18n/locales/en/translation.json`
2. Use in component: `const { t } = useTranslation(); t('key.path')`

## Checks

- Frontend tests are assertion scripts (`*.test.ts`) that print `ok`; run them with `bun test <dir>`
- The `*_rules.rs` and store modules have unit tests that run on any OS; the rest of the backend builds only for Windows, so check it with `cargo xwin clippy --target x86_64-pc-windows-msvc` from Linux
- The sync code's tests run on any OS through `phone/core` (`cargo test -p legato-phone` in `phone/`), including an end-to-end PC ↔ phone run
- After editing `translation.json` with a script, run Prettier on it; `bun run format:check` covers it

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
- In the bar's and the bubble's pages, listen with `getCurrentWebviewWindow().listen`: a bare `listen` from `@tauri-apps/api/event` also hears events the backend sends to other windows with `emit_to`

## CLI Parameters

Legato supports command-line parameters for integration with scripts, hotkey tools, and autostart configurations.

**Implementation:** `cli.rs` (definitions), `main.rs` (parsing), `lib.rs` (applying), `signal_handle.rs` (shared logic)

| Flag                     | Description                                                                        |
| ------------------------ | ---------------------------------------------------------------------------------- |
| `--toggle-transcription` | Toggle recording on/off on a running instance                                      |
| `--toggle-post-process`  | Toggle recording with post-processing on/off                                       |
| `--cancel`               | Cancel the current operation on a running instance                                 |
| `--open-scratchpad`      | Open the Scratchpad (also in the jump list)                                        |
| `--new-note`             | Open the Scratchpad on a new note (also in the jump list)                          |
| `--mcp`                  | Serve MCP over stdio for an AI app (its own process; needs Settings › Connections) |
| `--start-hidden`         | Launch without showing the main window (tray icon visible)                         |
| `--no-tray`              | Launch without system tray (closing window quits the app)                          |
| `--debug`                | Enable debug mode with verbose (Trace) logging                                     |

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
