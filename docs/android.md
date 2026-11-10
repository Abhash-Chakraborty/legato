# Legato for Android

Legato for Android dictates in any app on your phone or tablet, with its own speech model or your PC's, and stays in step with Legato on Windows.

It keeps your Dictionary, snippets, history, notes and Scratchpad the same on both, and works on its own too. It needs Android 10 or later on an arm64 (64-bit Arm) device.

## Install

1. On the phone, download `Legato-android-<version>.apk` from the [latest release](https://github.com/Abhash-Chakraborty/legato/releases/latest) and open it.
2. Android asks once to allow installs from your browser. Play Protect may offer to scan the app first; when it says the app looks safe, choose Install.
3. Open Legato. Its first-run steps offer a speech model to download, ask for the microphone, Legato under Accessibility, Display over other apps and (on Android 13 and later) notifications, and say why each is needed. The last steps pair it with your PC and show how a dictation goes. Settings › About › Setup guide runs them again.

Legato isn't on Google Play yet. The release's `legato-android.json` lists the APK's SHA-256 if you want to check the download.

> **Note:** On Android 13 and later, the Legato bubble's switch under Settings › Accessibility › Downloaded apps is greyed out as a restricted setting for an app installed from a download. Open Legato's App info, choose Allow restricted settings (in the ⋮ menu on most phones, further down on some), then turn the bubble on.

## Pair with your PC

Pairing is optional. Without it, the phone writes up what you say with its own model.

1. On the PC, open Legato › Settings › Devices › Pair a phone. It shows a QR code.
2. On the phone, choose Pair with your PC and scan the code with the camera, or paste the link from Copy link.
3. The phone names the PC it found. Tap Pair.

Your Dictionary, snippets, history, notes and Scratchpad come over at once. From then on, changes go both ways whenever the two can reach each other: over Wi-Fi when the phone is on the PC's network, and over Bluetooth when it's nearby but not on the same network. There's no account and no server in between, and each side checks the other's certificate. [Phone sync protocol](phone-sync.md) has the details.

Only pair with a code your own PC shows. Pairing with a new PC replaces the old one.

## Dictate in any app

Whenever the keyboard is open, a small bubble sits beside it. Gboard stays your keyboard. The bubble stays away from password and number fields, and from some banking and password manager apps.

1. Tap a text field. The bubble shows beside the keyboard.
2. Tap the bubble and talk. The line moves with your voice; ✕ stops and keeps what you said in Home.
3. Tap the check. A spinner shows while it's written up, then the words are in the field.

While the phone is connected, your PC's speech model writes up what you say, with your Dictionary and style. If the PC is slow and the phone has a model of its own, the phone starts on it too after a couple of seconds and types whichever text is back first. Settings › Dictation has both switches: Use your PC's speech model, and Don't wait for a slow PC.

A dictation keeps listening when you switch apps or pull down the notifications. To end it when the keyboard closes instead, turn off Keep listening when the keyboard closes in Settings › Bubble over Gboard.

A dictation that can't be written up (no PC, no model, a dropped link) keeps its audio in Home and is written up once your PC is back, or when you tap Try again.

## Speech models on the phone

Settings › Dictation › Speech model on this phone downloads a model that runs with no internet. Parakeet 110M is the one it offers first. Bigger models are more accurate and use more space, memory and battery; each holds about its size in memory while loaded and is put away after five minutes unused, or sooner when Android runs short of memory.

| Model             | Languages                        | What it's like                                                                          |
| ----------------- | -------------------------------- | --------------------------------------------------------------------------------------- |
| Moonshine Tiny    | English                          | The smallest and quickest, the lightest on the battery; makes the most mistakes         |
| Moonshine Base    | English                          | Quick, and more accurate than Tiny                                                      |
| Whisper Base      | 99 languages, Hindi among them   | Light and quick, less accurate than the English-only models                             |
| Parakeet 110M     | English                          | As quick as Moonshine Base and clearly more accurate: the best light choice for English |
| Moonshine Small   | English                          | Quick, and close to Parakeet 110M                                                       |
| Canary 180M Flash | English, German, Spanish, French | Quick, and more accurate than Parakeet 110M                                             |
| Whisper Small     | 99 languages, Hindi among them   | Slower                                                                                  |
| Moonshine Medium  | English                          | Accurate and still fairly quick                                                         |
| Parakeet 0.6B v2  | English                          | Very accurate, and quick for its size                                                   |
| Parakeet 0.6B v3  | 25 European languages            | Nearly as accurate as v2 in English                                                     |
| Qwen3-ASR 0.6B    | 30 languages, Hindi among them   | Accurate, slower than Parakeet                                                          |
| Canary 1B Flash   | English, German, Spanish, French | The most accurate here, the heaviest on memory and battery                              |

Whisper, Parakeet 0.6B v3 and Qwen3-ASR tell the language apart on their own. The Canary models write in your phone's language when they know it, and English otherwise.

## Notes and memos

The Notes tab records voice memos. Recording carries on with the screen off or another app in front, and the notification has Stop and save. Your words show half a minute at a time as you talk, and once the phone reaches your PC, the PC adds a summary, a title, tags and action items.

A part of a memo that can't be written up (no PC, no model, a dropped link) keeps its audio and is written up once your PC is back, or from Try again in the memo's menu. Long-press a memo made on the phone to delete it, with Undo.

Meetings and memos recorded on the PC show up here too, with their summary, action items and transcript, but not their recordings. Delete those on the PC. See [Notes and meetings](notes.md).

## Home, Words and the Scratchpad

- **Home** lists your dictations, from the phone and the PC, with search. Each has Copy, Share, Transform, Add a word from this and Delete. A cancelled or failed dictation waits at the top with Try again (or Write it up) and Discard it.
- **Words** holds your Dictionary and snippets, the same ones as on the PC. Add a word with how it's heard, or a snippet with the phrase you say and the text Legato types. See [Dictionary and snippets](dictionary.md).
- **Scratchpad** keeps notes you type or dictate, in step with the [Scratchpad](scratchpad.md) on your PC.

## Transforms

Transforms rewrite text with the AI provider you set up on your PC.

- **After each dictation:** pick a transform in Settings › Transforms and it rewrites what you say before it's typed. If it can't run, your words are typed as you said them.
- **On selected text:** select text in any app and pick Transform from its menu.
- **In Legato:** transform a dictation in Home, a Scratchpad note (or just the selected part) or a note's transcript, from its menu.

A transform is told what kind of app you're typing in, and its result is written the way that app shows formatting: `*bold*` in WhatsApp, `**bold**` in Telegram, Markdown in Mattermost and Discord, plain text in an email. When the transform runs on the PC, it knows only the kind of app, so a chat gets plain text.

Without a shared key, transforms run on the PC while the phone is connected, and the PC's AI key stays there. Turn on Share AI keys with your phone on the PC (Settings › Devices) and the phone sends transforms straight to your AI provider, connected or not, and asks the PC only if that fails. The phone keeps the key sealed in its secure hardware, and with it, what the phone writes up with its own model gets the same AI cleanup as on your PC. Settings › Transforms › Where they run says which applies. See [Transforms, Ask and your AI provider](transforms.md).

## Shared clipboard

Settings › Clipboard has two modes.

- **Automatic:** what you copy on the PC lands on the phone's clipboard at once. What you copy on the phone goes to the PC too. Android lets only the app in front read the clipboard, so the Legato bubble's Accessibility service notices the copy (Android 13 and later); otherwise the copy goes the next time you open Legato. What a password manager marks as private stays on the phone.
- **Manual:** nothing moves on its own. Send from Settings, from any app's share menu or from selected text (Send to PC), and pick the PC's copies from Recent clips.

## Updates

Legato checks GitHub Releases for a new version when it opens, at most every six hours, and offers it with Install and Later. Check by hand, or turn off Check for updates automatically, in Settings › About. Install downloads the APK, checks it against the release's checksum and hands it to Android's installer. An update keeps your pairing and notes, and what's new shows once it's installed.

From 0.4.1, updates install in a way that keeps the Legato bubble on in Accessibility. The update from 0.4.0 to 0.4.1 still goes the old way, so the bubble may come back off once: allow restricted settings as in [Install](#install), then turn it on again.

## Troubleshooting

| What you see                              | What to do                                                                                                                                                             |
| ----------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| The Accessibility switch is greyed out    | Allow restricted settings in Legato's App info (see [Install](#install)), then turn the bubble on.                                                                     |
| The bubble doesn't show over the keyboard | Check that Display over other apps and the Legato bubble under Accessibility are both on. Settings › Bubble over Gboard says which one is missing.                     |
| "Not connected to your PC right now"      | Put the phone on the PC's Wi-Fi, or turn on Bluetooth with the PC nearby. Legato must be running on the PC.                                                            |
| "Nothing can write it up yet"             | Connect to your PC or download a speech model in Settings › Dictation. The audio stays in Home until then.                                                             |
| Something else                            | Settings › About › Advanced has the connection details and the log. [Open an issue](https://github.com/Abhash-Chakraborty/legato/issues/new/choose) with what you saw. |
