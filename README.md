# Legato

Legato is a dictation app for Windows, and its speech recognition runs locally on your computer. Hold a shortcut, speak, and the text appears where you're typing.

Status: in development. Legato is a public fork of Handy with my changes.

To build it on Windows, see [BUILD.md](BUILD.md).

Legato started as a fork of [Handy](https://github.com/cjpais/Handy) by CJ Pais and its contributors (MIT).

## Debug mode

Legato has a debug mode for development and troubleshooting. Open it by pressing `Ctrl+Shift+D`.

## CLI parameters

Legato supports command-line flags for controlling a running instance and customizing startup behavior. Largely this is a beta feature.

**Remote control flags** (sent to an already-running instance via the single-instance plugin):

```powershell
legato --toggle-transcription    # Toggle recording on/off
legato --toggle-post-process     # Toggle recording with post-processing on/off
legato --cancel                  # Cancel the current operation
legato --open-scratchpad         # Open the Scratchpad
legato --new-note                # Open the Scratchpad on a new note
```

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

## Verify release signatures

Legato release artifacts are signed with Tauri's updater signature format. The public key is stored in [`src-tauri/tauri.conf.json`](src-tauri/tauri.conf.json) under `plugins.updater.pubkey`.

To verify a release manually, set `$Artifact` to the filename you downloaded, save the `pubkey` value from `src-tauri/tauri.conf.json` to `legato.pub.b64`, then decode the public key and matching `.sig` file from base64 and verify the artifact with `minisign`. In PowerShell:

```powershell
# Replace with the file you downloaded
$Artifact = "Legato_0.9.8_x64-setup.exe"

$pub = (Get-Content legato.pub.b64 -Raw).Trim()
[IO.File]::WriteAllBytes("$PWD\legato.pub", [Convert]::FromBase64String($pub))

$sig = (Get-Content "$Artifact.sig" -Raw).Trim()
[IO.File]::WriteAllBytes("$PWD\$Artifact.minisig", [Convert]::FromBase64String($sig))

minisign -Vm $Artifact -p legato.pub -x "$Artifact.minisig"
```

On success, `minisign` prints:

```text
Signature and comment signature verified
```

Do not use `gpg` for these `.sig` files.

## Troubleshooting

### Previous clipboard content is pasted instead of the transcription

If the transcription is correct in **History** but Legato inserts text you copied earlier, the problem is paste timing. With the standard clipboard paste method, Legato restores your previous clipboard after a fixed delay. Under load, the receiving application may read the clipboard only after that restoration.

1. Open Legato's settings window and press `Ctrl+Shift+D` to reveal **Debug**.
2. Try **Reliable Paste (Beta)** in Debug with a clipboard paste method selected. It uses clipboard read notifications to delay restoration instead of relying on the standard fixed delay. Test it in the application where the problem occurs; it is still experimental.
3. If Reliable Paste is disabled, increase **Paste Delay (After)** in Debug and test again. This controls the wait before restoring your previous clipboard. **Paste Delay (Before)** controls the wait before sending the paste keystroke and addresses a different part of the operation. These delay settings apply to the standard paste path, not Reliable Paste.

### Manual model installation (for proxy users or network restrictions)

If you're behind a proxy, firewall, or in a restricted network environment where Legato cannot download models automatically, you can manually download and install them. The URLs are publicly accessible from any browser.

#### Step 1: Find your app data directory

1. Open Legato settings
2. Navigate to the **About** section
3. Copy the "App Data Directory" path shown there, or press `Ctrl+Shift+D` to open the debug menu

The typical path is `C:\Users\{username}\AppData\Roaming\io.github.abhash-chakraborty.legato\`.

#### Step 2: Create the models directory

Inside your app data directory, create a `models` folder if it doesn't already exist:

```powershell
New-Item -ItemType Directory -Force -Path "$env:APPDATA\io.github.abhash-chakraborty.legato\models"
```

#### Step 3: Download model files

Download the models you want from below

**Whisper models (single .bin files):**

- Small (487 MB): `https://blob.handy.computer/ggml-small.bin`
- Medium (492 MB): `https://blob.handy.computer/whisper-medium-q4_1.bin`
- Turbo (1600 MB): `https://blob.handy.computer/ggml-large-v3-turbo.bin`
- Large (1100 MB): `https://blob.handy.computer/ggml-large-v3-q5_0.bin`

**Parakeet Unified EN 0.6B (single `.gguf` file, recommended):**

- Q8_0 (731 MB): `https://huggingface.co/handy-computer/parakeet-unified-en-0.6b-gguf/resolve/main/parakeet-unified-en-0.6b-Q8_0.gguf`

#### Step 4: Install models

**For Whisper models (.bin files):**

Simply place the `.bin` file directly into the `models` directory:

```
{app_data_dir}/models/
├── ggml-small.bin
├── whisper-medium-q4_1.bin
├── ggml-large-v3-turbo.bin
└── ggml-large-v3-q5_0.bin
```

**For GGUF models (.gguf files):**

Place the `.gguf` file directly into the `models` directory, exactly like the Whisper `.bin` files above. Legato also picks up models already present in the shared Hugging Face cache (`%USERPROFILE%\.cache\huggingface\hub`), so a copy downloaded by another tool works without being moved.

**Important notes:**

- Do not rename the `.bin` or `.gguf` files—use the exact filenames from the download URLs
- After placing the files, restart Legato to detect the new models

#### Step 5: Verify the installation

1. Restart Legato
2. Open Settings → Models
3. Your manually installed models should now appear as "Downloaded"
4. Select the model you want to use and test transcription

### Custom Whisper models

Legato can auto-discover custom Whisper GGML models placed in the `models` directory. This is useful for users who want to use fine-tuned or community models not included in the default model list.

**How to use:**

1. Obtain a Whisper model in GGML `.bin` format (e.g., from [Hugging Face](https://huggingface.co/models?search=whisper%20ggml))
2. Place the `.bin` file in your `models` directory (see the path above)
3. Restart Legato to discover the new model
4. The model will appear in the "Custom Models" section of the Models settings page

**Important:**

- Community models are user-provided and may not receive troubleshooting assistance
- The model must be a valid Whisper GGML format (`.bin` file)
- Model name is derived from the filename (e.g., `my-custom-model.bin` → "My Custom Model")

### Vulkan overlays and capture tools (`LEGATO_KEEP_VULKAN_IMPLICIT_LAYERS`)

Legato asks the Vulkan loader to skip implicit layers to avoid crashes caused by overlay and capture hooks. GPU acceleration remains enabled; this does not change system-wide settings.

To opt out for GPU selection or debugging tools, fully quit Legato (including the tray icon), then run both commands in the same PowerShell window:

```powershell
$env:LEGATO_KEEP_VULKAN_IMPLICIT_LAYERS = "1"
& "$env:ProgramFiles\Legato\legato.exe"
```

Adjust the executable path if needed. This override only applies to apps launched from that PowerShell session, not the Start menu. Legato also preserves any existing `VK_LOADER_LAYERS_DISABLE` value.

## License

MIT License - see [LICENSE](LICENSE) file for details.

Legato is an independent fork and isn't affiliated with or endorsed by the Handy project.

## Acknowledgments

- **Whisper** by OpenAI for the speech recognition model
- **ggml** for an amazing cross-platform tensor library
- **Silero** for great lightweight VAD
- **Tauri** team for the excellent Rust-based app framework
