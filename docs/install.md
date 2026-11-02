# Install on Windows

Legato runs on Windows 10 and 11, on x64 and on Arm (Surface Pro X, Copilot+ PCs).

Speech recognition runs on your PC: on the GPU through Vulkan on x64, and on the CPU on Arm.

## Which file

| You have                      | Download                                              |
| ----------------------------- | ----------------------------------------------------- |
| A Windows PC                  | `Legato_<version>_x64-setup.exe`                      |
| A Windows PC on Arm           | `Legato_<version>_arm64-setup.exe`                    |
| Apps you run as administrator | `Legato_<version>_x64_en-US.msi` (or `arm64`) instead |

All of them are on the [latest release](https://github.com/Abhash-Chakraborty/legato/releases/latest).

## Install

1. Run the setup file. It installs for your account and needs no administrator rights.
2. The installers aren't code-signed yet, so SmartScreen may say "Windows protected your PC". Choose More info, then Run anyway. You can [verify the download](verify.md) against the release signature first.
3. Setup checks your microphone and offers two speech models, the first chosen for your Windows input languages. It says how much the one you pick will download, then downloads it. [Getting started](getting-started.md) covers the rest of setup.

After that, Legato looks for a new version when it starts and asks before installing it. Turn the check off in Settings › About.

## Apps run as administrator

Windows doesn't let a normal app type into an app running as administrator. To dictate into those, install the `.msi` instead (it installs for all users), then turn on Settings › General › Work in apps run as administrator.

## Your phone

Legato for Android dictates in any app on your phone and syncs with the PC over Wi-Fi or Bluetooth. See [Legato for Android](android.md).

## Uninstall

Uninstall Legato from Windows Settings › Apps. If you installed it with the setup file, the uninstaller asks whether to delete the app data. Ticked, it removes Legato's data folders and the keys and addresses it kept in Windows Credential Manager. Unticked, they stay for a reinstall. Downloaded models stay in the shared Hugging Face cache (`%USERPROFILE%\.cache\huggingface`), since other tools may use them. [Privacy and your data](privacy.md#where-your-data-lives) lists every folder.
