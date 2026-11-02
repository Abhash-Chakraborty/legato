# Command line and debug mode

Legato's command-line flags control a running Legato from scripts and hotkey tools, change how it starts, and turn on debug mode.

## Debug mode

Legato has a debug mode for development and troubleshooting. Press `Ctrl+Shift+D` in the Legato window to turn it on or off. With it on, Settings gets a Debug section with the log level, a live log viewer and a few input settings. See [Settings](settings.md#debug).

## Running legato.exe

The installers don't add Legato to your `PATH`, so call it by its full path. The setup file installs it in `%LOCALAPPDATA%\Legato`, the `.msi` in `C:\Program Files\Legato`:

```powershell
& "$env:LOCALAPPDATA\Legato\legato.exe" --toggle-transcription
```

The examples below write `legato` for short.

## Command-line flags

Legato supports command-line flags for controlling a running instance and customizing startup behavior.

**Remote control flags** (sent to an already-running instance via the single-instance plugin):

```powershell
legato --toggle-transcription    # Start or stop a dictation, like the Dictate shortcut
legato --toggle-post-process     # Start or stop a dictation, like Dictate then transform
legato --cancel                  # Cancel the current operation
legato --open-scratchpad         # Open the Scratchpad
legato --new-note                # Open the Scratchpad on a new note
```

Without any of these, starting Legato again just brings its window to the front. If Legato isn't running yet, `--open-scratchpad` and `--new-note` start it and open only the Scratchpad.

**For AI apps:** `legato --mcp` serves Legato over the Model Context Protocol on stdio, so an AI app can read your dictations, notes, Dictionary, Scratchpad and meetings. It runs as its own process and needs "Let AI apps read Legato" turned on in Settings › Connections. [Connections](connections.md#ai-apps) shows how to add it to an AI app.

**Startup flags:**

```powershell
legato --start-hidden            # Start without showing the main window
legato --no-tray                 # Start without the system tray icon
legato --debug                   # Turn on debug mode with Trace logging
legato --help                    # Show all available flags
```

These last for that run only and don't change your saved settings.

Without a tray icon there would be no way back to a hidden window, so `--start-hidden` with `--no-tray` (or with Show tray icon off) still shows the window.

## Transcribing a file

These flags run Legato once without its window, print the result and exit. They don't reach a running Legato, and the model must already be downloaded.

```powershell
legato --list-models                       # Model ids, with ✓ for downloaded ones
legato --transcribe-file talk.wav          # Transcribe a WAV with the selected model
legato -f talk.wav --model <id> --json     # With another model, as JSON
legato --list-devices                      # Compute devices the speech engine can use
```

| Flag                      | What it does                                                                   |
| ------------------------- | ------------------------------------------------------------------------------ |
| `-f`, `--transcribe-file` | Transcribes this WAV. It must be 16 kHz, mono, 16-bit PCM                      |
| `--model <id>`            | The model to use, by its id from `--list-models`; the selected model otherwise |
| `--device-index <n>`      | The compute device, by its index from `--list-devices` (Whisper-family models) |
| `--repeat <n>`            | Runs the transcription n times and reports the fastest                         |
| `--json`                  | Prints the result, or the model list, as JSON                                  |

Legato is a windowed app, so PowerShell shows what it prints only when you pipe or redirect it, for example `legato --list-models | Out-String` or `legato --help > help.txt`.
