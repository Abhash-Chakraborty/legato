# Settings

A map of Legato's Settings: what each section holds and which page covers it in more detail.

Open Settings from the bottom of the sidebar. Find a setting, at the top, searches every section by name. Speech models aren't in Settings; they have their own Models page in the sidebar.

Some rows depend on another one and say so when they can't be changed yet, such as "Turn on Sound feedback first."

## General

Your name, so notes give your tasks to You and dictations spell your name right. Startup: Launch at login, Show tray icon, Start hidden and Work in apps run as administrator (see [Install on Windows](install.md#apps-run-as-administrator)). Appearance: Theme.

## Features

Turns whole parts of Legato on or off. What's off leaves the sidebar, the shortcuts and the bar.

| Switch             | What it covers                                                                                                |
| ------------------ | ------------------------------------------------------------------------------------------------------------- |
| Notes              | Voice memos and meeting notes, with call offers and meeting cards. See [Notes and meetings](notes.md)         |
| Scratchpad         | The Scratchpad window. A paste that fails still lands there when this is off. See [Scratchpad](scratchpad.md) |
| Transforms and Ask | Rewriting a selection or your last dictation with your own AI. See [Transforms](transforms.md)                |
| Insights           | How much you dictate, and how often Legato gets a word wrong                                                  |

Under Other apps, Let AI apps read Legato lets MCP apps read your data (see [Connections](connections.md#ai-apps)).

A new install starts with dictation only, and the last step of setup asks which of these you want. Settings saved by older versions keep everything on.

## Shortcuts

Keys that work in any app, even with the window closed: Dictate and Shortcut behaviour, Dictate then transform, Cancel and Add selection to Dictionary; Transform the selection, Ask and Show the last changes; Open Scratchpad; Start or stop notes and Screenshot into notes; Talk to the companion for Inyon; Paste last dictation, Copy last dictation and Correct last dictation; and the Keyboard backend. See [Dictation](dictation.md).

## Audio

The Microphone, Microphone order for when the first isn't plugged in, and Input channel. While you dictate: Music and videos (lower, pause or mute other apps) and Keep the microphone open. Sounds: Sound feedback, the sound, its volume and the output device.

## Transcription

How speech becomes text on this PC. Language: Spoken language, Translate to English and Chinese script. Recording: Voice activity detection, Split long recordings, Voice detection engine and Extra recording buffer. Cleanup: Remove filler words. Engine: GPU acceleration and Unload model after.

## Output

How text gets into the app you're dictating into. Paste: Paste method, Reliable paste, Clipboard after pasting and Open the Scratchpad when a paste fails. After pasting: Auto-submit after paste, Say "press enter" to send, A message per paragraph in chat apps and Add a trailing space. [Style](style.md) can change some of these per kind of app.

## AI

Your own AI provider and model, what Legato may use it for (the correction pass, transforms, Notes summaries and Ask), a model for each task, and time limits. See [Transforms, Ask and your AI provider](transforms.md).

## Context & calls

Detect calls and offer notes, with Read browser window titles, Apps to watch and Test detection. When a call starts and When the call ends set how notes start and stop. Speakers in notes: Speaker model, Tell speakers apart, Remember voices and Saved voices. Recordings: Keep note audio. See [Notes and meetings](notes.md).

## Connections

Calendars, Word copies of notes in a folder, your Inyon hub, and AI apps that may read Legato. See [Connections](connections.md).

## Legato bar

The bar shows what Legato is doing.

| Setting                                  | Options                                                                                                     |
| ---------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| Style                                    | None (no bar; the tray icon still shows when Legato listens), Minimal, or Live (your words as you say them) |
| Shape                                    | Bar, a card at the top or bottom of the screen, or Island, which hangs from the top edge                    |
| When idle                                | For the island: Small island, with Dictate, Notes and the Scratchpad on hover, or Hide                      |
| Position                                 | For the bar: Bottom or Top                                                                                  |
| Background                               | How solid the background behind your words is, on the island and the live transcript box                    |
| Bubble on the desktop                    | A dot at a screen edge that shows your dictation instead of the bar                                         |
| Live transcript on screen                | While the bubble has your dictation: Off, Bottom or Top                                                     |
| Hide from screenshots and screen sharing | Keeps the bar, island, bubble and Notes panel out of captures; on to start with                             |

Messages: Confirm after pasting and Show changes after a transform.

## Devices

Your phones and tablets: pairing, sync status, what syncs and the shared clipboard. See [Legato for Android](android.md).

## Privacy

History: Keep at most and Keep recordings. Apps: Remember which app I dictated into. See [Privacy and your data](privacy.md).

## About

Updates: the version, Check for updates automatically, Show what's new after an update, What's new in this version (the same list again, whenever you want it) and Setup and the tour: Run setup again, and Show the list on Home for [Get to know Legato](getting-started.md#get-to-know-legato). Troubleshooting: App data and Logs open their folders. Open source: a link to the source code.

Legato looks for a new version when it starts, and nothing installs until you choose to. To force checks off without changing the setting, set the environment variable `LEGATO_DISABLE_UPDATER=1` before Legato starts. The switch then shows "Disabled by system configuration (LEGATO_DISABLE_UPDATER)" and can't be turned back on.

## Debug

Press `Ctrl+Shift+D` in the Legato window to turn debug mode on or off. With it on, a Debug section appears at the end of Settings:

- Debug mode, with Turn off.
- Logs: Log level, and Live logs, which shows the log as it's written.
- Input: Paste delay, the wait before the paste keys (raise it if nothing gets pasted), and Always-on microphone.
- Previews of the setup screens and of What's new.
- Check the AI on your fixes: runs Cleanup at Medium on up to 20 of your newest dictations with a word you fixed by hand, starting from what the speech model heard, and counts how many of those words come out spelled your way. It also counts answers that rewrote too much or brought in a word nobody said. It sends those dictations to your AI provider, so it runs only when you press Run the check.

Debug mode is for tracking down problems. See [Troubleshooting](troubleshooting.md) and [Command line](cli.md).
