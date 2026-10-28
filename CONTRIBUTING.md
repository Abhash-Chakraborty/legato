# Contributing

Bug reports, ideas and pull requests are welcome. For anything bigger than a small fix, open an issue first so we can agree on the approach before you spend time on it.

## Set up

Legato is a Tauri 2 app: a Rust backend in `src-tauri/` and a React and TypeScript frontend in `src/`. It builds on Windows (x64 or ARM64). [BUILD.md](BUILD.md) lists the tools: Rust, Bun, the Visual Studio C++ tools, CMake and the Vulkan SDK.

```powershell
bun install
bun run tauri dev
```

The first build compiles the speech engine and takes a while; later ones are quick.

## Where things are

- `src-tauri/src/` holds the backend. Each `*_rules.rs` file is pure logic with unit tests that run on any OS; the Windows-only glue lives beside it.
- `src-tauri/src/managers/` holds audio, models, transcription and the SQLite stores.
- `src/components/` holds the UI, with one folder per page and per settings section.
- `phone/` and `android/` hold the Android app and the sync code it shares with the PC ([docs/phone-sync.md](docs/phone-sync.md)).

## Before you open a pull request

```powershell
bun run lint          # ESLint
bun run format:check  # Prettier and cargo fmt
cd src-tauri; cargo clippy; cargo test
```

- Every string the UI shows goes through i18next: add it to `src/i18n/locales/en/translation.json` and use `t("key")`. ESLint fails on text written straight into JSX.
- Keep pure logic in a `*_rules.rs` module with tests, so it can be checked without Windows.
- Commit messages use conventional prefixes (`feat:`, `fix:`, `docs:`, `refactor:`, `chore:`) and say why the change is needed.

## Reporting a bug

Use the [bug form](https://github.com/Abhash-Chakraborty/legato/issues/new/choose). The version, how Legato was installed and the end of the log usually tell us what went wrong. For a security problem, see [SECURITY.md](SECURITY.md) instead.
