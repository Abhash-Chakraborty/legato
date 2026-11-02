# Legato for Android

Legato for Android puts dictation in any app on your phone or tablet and keeps your Dictionary, snippets, history, notes and Scratchpad in step with Legato on your PC. It works on its own too, with a speech model on the phone. It needs Android 10 or later on an arm64 (64-bit Arm) device.

## Install

1. On the phone, download `Legato-android-<version>.apk` from the [latest release](https://github.com/Abhash-Chakraborty/legato/releases/latest) and open it.
2. Android asks once to allow installs from your browser. Play Protect may offer to scan the app first; when it says the app looks safe, choose Install.
3. Open Legato. Its first-run steps ask for the microphone, notifications, Display over other apps and the Legato bubble under Accessibility, and say why each is needed.

Legato isn't on Google Play yet. The release's `legato-android.json` lists the APK's SHA-256 if you want to check the download.

> **Note:** On Android 13 and later, the Legato bubble's switch under Settings › Accessibility › Downloaded apps is greyed out as a restricted setting for an app installed from a download. Open Legato's App info, choose Allow restricted settings (in the ⋮ menu on most phones, further down on some), then turn the bubble on.

## Pair with your PC

Pairing is optional. Without it, the phone writes up what you say with its own model.

1. On the PC, open Legato › Settings › Devices › Pair a phone. It shows a QR code.
2. On the phone, choose Pair with your PC and scan the code with the camera, or paste the link from Copy link.

Your Dictionary, snippets, history, notes and Scratchpad come over at once. From then on, changes go both ways whenever the two can reach each other: over Wi-Fi when the phone is on the PC's network, and over Bluetooth when it's nearby but not on the same network. There's no account and no server in between, and each side checks the other's certificate. [Phone sync protocol](phone-sync.md) has the details.

Only pair with a code your own PC shows. Pairing with a new PC replaces the old one.

## Dictate in any app

Whenever the keyboard is open, a small bubble sits beside it. Gboard stays your keyboard.

1. Tap a text field. The bubble shows beside the keyboard.
2. Tap the bubble and talk. The line moves with your voice; ✕ stops and keeps what you said in Home.
3. Tap the check. A spinner shows while it's written up, then the words are in the field.

While the phone is connected, your PC's speech model writes up what you say, with your Dictionary and style. If the PC is slow, the phone starts on it too after a couple of seconds and types whichever text is back first. Turn this off with Don't wait for a slow PC in Settings › Dictation.

A dictation that can't be written up (no PC, no model, a dropped link) keeps its audio in Home and is written up once your PC is back, or when you tap Try again.

## Speech models on the phone

Settings › Dictation › Speech model on this phone downloads a model that runs with no internet. Bigger models are more accurate and use more space, memory and battery; each holds about its size in memory while loaded and is put away after five minutes unused.

| Model             | Languages                        | What it's like                                                                          |
| ----------------- | -------------------------------- | --------------------------------------------------------------------------------------- |
| Moonshine Tiny    | English                          | The smallest and quickest, the lightest on the battery; makes the most mistakes         |
| Moonshine Base    | English                          | Quick, and more accurate than Tiny                                                      |
| Parakeet 110M     | English                          | As quick as Moonshine Base and clearly more accurate: the best light choice for English |
| Moonshine Small   | English                          | Quick, and close to Parakeet 110M                                                       |
| Moonshine Medium  | English                          | Accurate and still fairly quick                                                         |
| Canary 180M Flash | English, German, Spanish, French | Quick, and more accurate than Parakeet 110M                                             |
| Whisper Base      | 99 languages, Hindi among them   | Light and quick, less accurate than the English-only models                             |
| Whisper Small     | 99 languages, Hindi among them   | Slower                                                                                  |
| Parakeet 0.6B v2  | English                          | Very accurate, and quick for its size                                                   |
| Parakeet 0.6B v3  | 25 European languages            | Nearly as accurate as v2 in English                                                     |
| Qwen3-ASR 0.6B    | 30 languages, Hindi among them   | Accurate, slower than Parakeet                                                          |
| Canary 1B Flash   | English, German, Spanish, French | The most accurate here, the heaviest on memory and battery                              |

The multilingual models tell the language apart on their own.

## Notes and memos

The Notes tab records voice memos. Your words show half a minute at a time as you talk, and once the phone reaches your PC, the PC adds a summary, a title and tags. Notes recorded on the PC show up here too.

## Transforms

Transforms rewrite text with the AI provider you set up on your PC.

- **After each dictation:** pick a transform in Settings › Transforms and it rewrites what you say before it's typed. If it can't run, your words are typed as you said them.
- **On selected text:** select text in any app and pick Transform from its menu.

While connected, transforms run on the PC and its AI key stays there. To run them away from the PC, turn on Share AI keys with your phone on the PC; the phone keeps the key sealed in its secure hardware.

## Shared clipboard

Settings › Clipboard has two modes.

- **Automatic:** what you copy on the PC lands on the phone's clipboard at once. What you copy on the phone goes to the PC the next time you open Legato, since Android lets apps read the clipboard only while they're open.
- **Manual:** nothing moves on its own. Send from Settings, from any app's share menu or from selected text (Send to PC), and pick the PC's copies from Recent clips.

## Updates

Legato checks GitHub Releases for a new version when it opens, at most every six hours, and asks before installing it. Check by hand or turn the check off in Settings › About. An update keeps your pairing and notes, and what's new shows once it's installed.

## Troubleshooting

| What you see                              | What to do                                                                                                                                                     |
| ----------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| The Accessibility switch is greyed out    | Allow restricted settings in Legato's App info (see [Install](#install)), then turn the bubble on.                                                             |
| The bubble doesn't show over the keyboard | Check that Display over other apps and the Legato bubble under Accessibility are both on. Settings › Bubble over Gboard says which one is missing.             |
| "Not connected to your PC right now"      | Put the phone on the PC's Wi-Fi, or turn on Bluetooth with the PC nearby. Legato must be running on the PC.                                                    |
| "Nothing can write it up yet"             | Connect to your PC or download a speech model in Settings › Dictation. The audio stays in Home until then.                                                     |
| Something else                            | Settings › Advanced has the connection details and the log. [Open an issue](https://github.com/Abhash-Chakraborty/legato/issues/new/choose) with what you saw. |
