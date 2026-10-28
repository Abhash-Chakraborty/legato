# Command line and debug mode

## Debug mode

Legato has a debug mode for development and troubleshooting. Open it by pressing `Ctrl+Shift+D`.

## Command-line flags

Legato supports command-line flags for controlling a running instance and customizing startup behavior.

**Remote control flags** (sent to an already-running instance via the single-instance plugin):

```powershell
legato --toggle-transcription    # Toggle recording on/off
legato --toggle-post-process     # Toggle recording with post-processing on/off
legato --cancel                  # Cancel the current operation
legato --open-scratchpad         # Open the Scratchpad
legato --new-note                # Open the Scratchpad on a new note
```

**For AI apps:** `legato --mcp` serves Legato over the Model Context Protocol on stdio, so an AI app can read your dictations, notes, Dictionary and Scratchpad. It runs as its own process and needs "Let AI apps read Legato" turned on in Settings › Connections.

**Startup flags:**

```powershell
legato --start-hidden            # Start without showing the main window
legato --no-tray                 # Start without the system tray icon
legato --debug                   # Enable debug mode with verbose logging
legato --help                    # Show all available flags
```

Flags can be combined for autostart scenarios:

```powershell
legato --start-hidden --no-tray
```
