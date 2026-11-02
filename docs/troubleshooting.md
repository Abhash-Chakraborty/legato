# Troubleshooting

Fixes for the wrong text being pasted, installing speech models by hand, custom Whisper models, and crashes caused by Vulkan overlays.

## Previous clipboard content is pasted instead of the transcription

If the transcription is right on **Home** but Legato inserts text you copied earlier, the problem is paste timing. With a clipboard paste method, Legato puts your previous clipboard back after a fixed delay. Under load, the receiving app may read the clipboard only after that.

1. Open Settings › Output and check that Paste method is one of the Clipboard options.
2. Turn on **Reliable paste** under it. It puts your old clipboard back only after the app has read the transcript, instead of after a fixed delay. Test it in the app where the problem happens.

If nothing gets pasted at all, that's a different problem. Press `Ctrl+Shift+D` to show **Debug** in Settings and raise **Paste delay** under Input. It sets the wait between copying the text and pressing the paste keys.

## Manual model installation (for proxy users or network restrictions)

If you're behind a proxy or firewall where Legato can't download models, you can download them yourself. The URLs open in any browser. When Hugging Face can't be reached, Legato already tries its mirror at `blob.handy.computer` on its own, so check whether that mirror is blocked too.

### Step 1: Find your app data directory

1. Open Settings › About.
2. Under Troubleshooting, App data opens the folder.

The usual path is `C:\Users\{username}\AppData\Roaming\io.github.abhash-chakraborty.legato\`.

### Step 2: Create the models directory

Inside your app data directory, create a `models` folder if it doesn't already exist:

```powershell
New-Item -ItemType Directory -Force -Path "$env:APPDATA\io.github.abhash-chakraborty.legato\models"
```

### Step 3: Download model files

Legato's models are GGUF files in the [handy-computer](https://huggingface.co/handy-computer) organisation on Hugging Face. Some of the ones setup suggests:

| Model                    | File and size                                                                                                                                                                    |
| ------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Parakeet Unified EN 0.6B | [`parakeet-unified-en-0.6b-Q8_0.gguf`](https://huggingface.co/handy-computer/parakeet-unified-en-0.6b-gguf/resolve/main/parakeet-unified-en-0.6b-Q8_0.gguf), 731 MB, English     |
| Canary 180M Flash        | [`canary-180m-flash-Q8_0.gguf`](https://huggingface.co/handy-computer/canary-180m-flash-gguf/resolve/main/canary-180m-flash-Q8_0.gguf), 218 MB, English, German, Spanish, French |
| Whisper Medium           | [`whisper-medium-Q8_0.gguf`](https://huggingface.co/handy-computer/whisper-medium-gguf/resolve/main/whisper-medium-Q8_0.gguf), 832 MB, 99 languages                              |

Any other model follows the same pattern: `https://huggingface.co/handy-computer/<repo>/resolve/main/<file>`.

**Older Whisper models (single .bin files)** still work if you place them in the folder, though Legato no longer offers them for download:

- Small (487 MB): `https://blob.handy.computer/ggml-small.bin`
- Medium (492 MB): `https://blob.handy.computer/whisper-medium-q4_1.bin`
- Turbo (1600 MB): `https://blob.handy.computer/ggml-large-v3-turbo.bin`
- Large (1100 MB): `https://blob.handy.computer/ggml-large-v3-q5_0.bin`

### Step 4: Install models

Place the `.gguf` or `.bin` file directly into the `models` directory:

```
{app_data_dir}/models/
├── parakeet-unified-en-0.6b-Q8_0.gguf
└── ggml-small.bin
```

Legato also picks up models already in the shared Hugging Face cache (`%USERPROFILE%\.cache\huggingface\hub`), so a copy downloaded by another tool works without being moved.

**Important notes:**

- Don't rename the files. Use the exact file names from the download URLs.
- A partly downloaded file (ending in `.partial`) is ignored.

### Step 5: Verify the installation

1. Open the **Models** page in the sidebar and choose the rescan button, or restart Legato.
2. Your models should now be under **Downloaded models**.
3. Select the model you want and try a dictation.

## Custom Whisper models

Legato finds Whisper GGML (`.bin`) and GGUF (`.gguf`) models you place in the `models` directory. Use this for fine-tuned or community models that aren't in Legato's list.

**How to use:**

1. Get a model as a GGML `.bin` or a `.gguf` file (for example, from [Hugging Face](https://huggingface.co/models?search=whisper%20ggml)).
2. Place the file in your `models` directory (see the path above).
3. Rescan on the Models page, or restart Legato.
4. The model shows at the bottom of **Downloaded models**, marked **Custom**.

**Important:**

- Community models are user-provided and may not get troubleshooting help.
- The name comes from the GGUF file's own metadata when it has one, or else from the file name (`my-custom-model.bin` becomes "My Custom Model").

## Vulkan overlays and capture tools (`LEGATO_KEEP_VULKAN_IMPLICIT_LAYERS`)

Legato asks the Vulkan loader to skip implicit layers, to avoid crashes caused by overlay and capture hooks. GPU acceleration stays on, and this doesn't change system-wide settings.

To opt out for GPU selection or debugging tools, fully quit Legato (including the tray icon), then run both commands in the same PowerShell window:

```powershell
$env:LEGATO_KEEP_VULKAN_IMPLICIT_LAYERS = "1"
& "$env:LOCALAPPDATA\Legato\legato.exe"
```

That's where the setup file installs Legato. If you installed the `.msi`, use `& "$env:ProgramFiles\Legato\legato.exe"` instead. The override only applies to apps started from that PowerShell window, not from the Start menu. Legato also keeps any `VK_LOADER_LAYERS_DISABLE` value you set yourself.

## Still stuck

Settings › About › Logs opens the folder with Legato's log files. [Open an issue](https://github.com/Abhash-Chakraborty/legato/issues/new/choose) with what you saw and the log. [Privacy and your data](privacy.md) lists everything Legato keeps on disk.
