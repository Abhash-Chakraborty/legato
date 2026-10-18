# Handy

**A free, open source, and extensible speech-to-text application that works completely offline.**

Handy is a Windows desktop application that provides simple, privacy-focused speech transcription. Press a shortcut, speak, and have your words appear in any text field. This happens on your own computer without sending any information to the cloud.

## How It Works

1. **Press** a configurable keyboard shortcut: hold it to record and release to stop, or tap it to toggle recording on and off (Hold-only and Toggle-only modes are also available)
2. **Speak** your words while the shortcut is active
3. **Release** and Handy processes your speech using Whisper
4. **Get** your transcribed text pasted directly into whatever app you're using

The process is entirely local:

- Silence is filtered using VAD (Voice Activity Detection) with Silero
- Transcription uses your choice of models:
  - **Whisper models** (Small/Medium/Turbo/Large) with GPU acceleration when available
  - **Parakeet V3** - CPU-optimized model with excellent performance and automatic language detection
- Runs on Windows x64 and ARM64

## Quick Start

### Installation

1. Download the latest installer from the [releases page](https://github.com/Abhash-Chakraborty/Handy/releases)
2. Install the application
3. Launch Handy and allow microphone access if it asks
4. Configure your preferred keyboard shortcuts in Settings
5. Start transcribing!

### Development Setup

For detailed build instructions, see [BUILD.md](BUILD.md).

## Debug Mode

Handy includes an advanced debug mode for development and troubleshooting. Access it by pressing `Ctrl+Shift+D`.

## CLI Parameters

Handy supports command-line flags for controlling a running instance and customizing startup behavior. Largely this is a beta feature.

**Remote control flags** (sent to an already-running instance via the single-instance plugin):

```powershell
handy --toggle-transcription    # Toggle recording on/off
handy --toggle-post-process     # Toggle recording with post-processing on/off
handy --cancel                  # Cancel the current operation
```

**Startup flags:**

```powershell
handy --start-hidden            # Start without showing the main window
handy --no-tray                 # Start without the system tray icon
handy --debug                   # Enable debug mode with verbose logging
handy --help                    # Show all available flags
```

Flags can be combined for autostart scenarios:

```powershell
handy --start-hidden --no-tray
```

## Verify Release Signatures

Handy release artifacts are signed with Tauri's updater signature format. The public key is stored in [`src-tauri/tauri.conf.json`](src-tauri/tauri.conf.json) under `plugins.updater.pubkey`.

To verify a release manually, set `$Artifact` to the filename you downloaded, save the `pubkey` value from `src-tauri/tauri.conf.json` to `handy.pub.b64`, then decode the public key and matching `.sig` file from base64 and verify the artifact with `minisign`. In PowerShell:

```powershell
# Replace with the file you downloaded
$Artifact = "Handy_0.9.8_x64-setup.exe"

$pub = (Get-Content handy.pub.b64 -Raw).Trim()
[IO.File]::WriteAllBytes("$PWD\handy.pub", [Convert]::FromBase64String($pub))

$sig = (Get-Content "$Artifact.sig" -Raw).Trim()
[IO.File]::WriteAllBytes("$PWD\$Artifact.minisig", [Convert]::FromBase64String($sig))

minisign -Vm $Artifact -p handy.pub -x "$Artifact.minisig"
```

On success, `minisign` prints:

```text
Signature and comment signature verified
```

Do not use `gpg` for these `.sig` files.

## Troubleshooting

### Previous Clipboard Content Is Pasted Instead of the Transcription

If the transcription is correct in **History** but Handy inserts text you copied earlier, the problem is paste timing. With the standard clipboard paste method, Handy restores your previous clipboard after a fixed delay. Under load, the receiving application may read the clipboard only after that restoration.

1. Open Handy's settings window and press `Ctrl+Shift+D` to reveal **Debug**.
2. Try **Reliable Paste (Beta)** in Debug with a clipboard paste method selected. It uses clipboard read notifications to delay restoration instead of relying on the standard fixed delay. Test it in the application where the problem occurs; it is still experimental.
3. If Reliable Paste is disabled, increase **Paste Delay (After)** in Debug and test again. This controls the wait before restoring your previous clipboard. **Paste Delay (Before)** controls the wait before sending the paste keystroke and addresses a different part of the operation. These delay settings apply to the standard paste path, not Reliable Paste.

### Manual Model Installation (For Proxy Users or Network Restrictions)

If you're behind a proxy, firewall, or in a restricted network environment where Handy cannot download models automatically, you can manually download and install them. The URLs are publicly accessible from any browser.

#### Step 1: Find Your App Data Directory

1. Open Handy settings
2. Navigate to the **About** section
3. Copy the "App Data Directory" path shown there, or press `Ctrl+Shift+D` to open the debug menu

The typical path is `C:\Users\{username}\AppData\Roaming\com.pais.handy\`.

#### Step 2: Create Models Directory

Inside your app data directory, create a `models` folder if it doesn't already exist:

```powershell
New-Item -ItemType Directory -Force -Path "$env:APPDATA\com.pais.handy\models"
```

#### Step 3: Download Model Files

Download the models you want from below

**Whisper Models (single .bin files):**

- Small (487 MB): `https://blob.handy.computer/ggml-small.bin`
- Medium (492 MB): `https://blob.handy.computer/whisper-medium-q4_1.bin`
- Turbo (1600 MB): `https://blob.handy.computer/ggml-large-v3-turbo.bin`
- Large (1100 MB): `https://blob.handy.computer/ggml-large-v3-q5_0.bin`

**Parakeet Unified EN 0.6B (single `.gguf` file, recommended):**

- Q8_0 (731 MB): `https://huggingface.co/handy-computer/parakeet-unified-en-0.6b-gguf/resolve/main/parakeet-unified-en-0.6b-Q8_0.gguf`

#### Step 4: Install Models

**For Whisper Models (.bin files):**

Simply place the `.bin` file directly into the `models` directory:

```
{app_data_dir}/models/
├── ggml-small.bin
├── whisper-medium-q4_1.bin
├── ggml-large-v3-turbo.bin
└── ggml-large-v3-q5_0.bin
```

**For GGUF Models (.gguf files):**

Place the `.gguf` file directly into the `models` directory, exactly like the Whisper `.bin` files above. Handy also picks up models already present in the shared Hugging Face cache (`%USERPROFILE%\.cache\huggingface\hub`), so a copy downloaded by another tool works without being moved.

**Important Notes:**

- Do not rename the `.bin` or `.gguf` files—use the exact filenames from the download URLs
- After placing the files, restart Handy to detect the new models

#### Step 5: Verify Installation

1. Restart Handy
2. Open Settings → Models
3. Your manually installed models should now appear as "Downloaded"
4. Select the model you want to use and test transcription

### Custom Whisper Models

Handy can auto-discover custom Whisper GGML models placed in the `models` directory. This is useful for users who want to use fine-tuned or community models not included in the default model list.

**How to use:**

1. Obtain a Whisper model in GGML `.bin` format (e.g., from [Hugging Face](https://huggingface.co/models?search=whisper%20ggml))
2. Place the `.bin` file in your `models` directory (see the path above)
3. Restart Handy to discover the new model
4. The model will appear in the "Custom Models" section of the Models settings page

**Important:**

- Community models are user-provided and may not receive troubleshooting assistance
- The model must be a valid Whisper GGML format (`.bin` file)
- Model name is derived from the filename (e.g., `my-custom-model.bin` → "My Custom Model")

### Vulkan Overlays and Capture Tools (`HANDY_KEEP_VULKAN_IMPLICIT_LAYERS`)

Handy asks the Vulkan loader to skip implicit layers to avoid crashes caused by overlay and capture hooks. GPU acceleration remains enabled; this does not change system-wide settings.

To opt out for GPU selection or debugging tools, fully quit Handy (including the tray icon), then run both commands in the same PowerShell window:

```powershell
$env:HANDY_KEEP_VULKAN_IMPLICIT_LAYERS = "1"
& "$env:ProgramFiles\Handy\handy.exe"
```

Adjust the executable path if needed. This override only applies to apps launched from that PowerShell session, not the Start menu. Handy also preserves any existing `VK_LOADER_LAYERS_DISABLE` value.

## License

MIT License - see [LICENSE](LICENSE) file for details.

Handy is open-source software, but the Handy name, logo, icon, and brand assets are not open-source. Unofficial forks, rewrites, and redistributions must use their own branding and must not imply endorsement or affiliation.

## Acknowledgments

- **[Handy](https://github.com/cjpais/Handy)** by CJ Pais and its contributors, the upstream project this fork is built on
- **Whisper** by OpenAI for the speech recognition model
- **ggml** for an amazing cross-platform tensor library
- **Silero** for great lightweight VAD
- **Tauri** team for the excellent Rust-based app framework
