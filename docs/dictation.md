# Dictation

Hold a shortcut, talk, and Legato pastes the text where you're typing. This page covers the shortcuts, the bar, pasting and the speech models on your PC.

## Start and stop

Dictate is Ctrl+Space unless you changed it in Settings › Shortcuts. Settings › Shortcuts › Shortcut behaviour decides how it starts and stops:

| Option | What happens                                                                                 |
| ------ | -------------------------------------------------------------------------------------------- |
| Auto   | Hold to talk and let go to paste, or tap to keep listening until you tap again. The default. |
| Hold   | Legato listens only while you hold the keys.                                                 |
| Toggle | Tap to start listening, tap again to paste.                                                  |

With Auto, a press longer than the Hold threshold (300 ms unless you change it) counts as a hold.

Cancel (Esc) stops a recording without pasting anything. Legato takes Esc only while it's listening, so the key works as usual in your apps the rest of the time. A dictation left running for 20 minutes is finished as if you had pressed the shortcut again.

You can also start and stop from the bar's buttons, the bubble, the Dictate item in Legato's taskbar jump list, or the command line (`legato --toggle-transcription`, see [Command line](cli.md)).

Dictate then transform (Ctrl+Shift+Space) records the same way, then runs a transform on the text before pasting. It needs Transforms turned on; see [Transforms, Ask and your AI provider](transforms.md).

## Other dictation shortcuts

| Shortcut                    | Keys        | What it does                                                                                 |
| --------------------------- | ----------- | -------------------------------------------------------------------------------------------- |
| Paste last dictation        | Alt+Shift+Z | Pastes what you last dictated again, wherever the cursor is                                  |
| Copy last dictation         | Alt+Shift+X | Puts what you last dictated on the clipboard                                                 |
| Correct last dictation      | Alt+Shift+C | Opens what you last dictated on the bar to fix ([Correct a dictation](#correct-a-dictation)) |
| Add selection to Dictionary | Win+Alt+W   | Adds the selected word to your [Dictionary](dictionary.md)                                   |
| Open Scratchpad             | Win+Alt+S   | Press to open the [Scratchpad](scratchpad.md); hold to dictate into a new note               |

Settings › Shortcuts › Keyboard backend picks how Legato hears keys. The built-in hook handles holds and single keys like Esc. Switch to Windows hotkeys if a game or security tool blocks it.

## The bar, the island and the bubble

Settings › Legato bar decides what shows while you dictate.

- **Style**: None shows no bar (the tray icon still shows when Legato is listening). Minimal shows Cancel, a waveform and Finish, and never grows. Live, the default, shows your words as you say them, the time, and the transform that runs after.
- **Shape**: Bar is a card at the top or bottom of the screen; Position picks which, and Bottom is the default. Island hangs from the top edge and grows with what it shows.
- **When idle**: with the island, Small island keeps it at the top with Dictate, Notes and the Scratchpad on hover. Hide shows it only while there's something to show.
- **Hide from screenshots and screen sharing**: on by default. The bar, the island, the bubble and the Notes side panel stay on your screen but don't show in screenshots, recordings or a screen you share in a call.

### The bubble

Bubble on the desktop puts a small Legato dot in the middle of a screen edge. Hover it for Dictate, Notes and the Scratchpad. Drag it to another edge and it settles in the middle of the nearest one. Right-click it to hide it for an hour, turn it off or open Legato. It steps aside while a window fills the screen.

While the bubble is on, it shows your dictation in place of the bar. Hovered during a dictation, Dictate becomes Done, paste it and Cancel takes the Scratchpad's place. If the work after you stop takes longer than about a second, the bar comes up to name the step. The island stands in for the bubble, so the bubble is off while the island shape is chosen.

### What the bar says

| You see                         | What it means                                                         |
| ------------------------------- | --------------------------------------------------------------------- |
| Loading model...                | The speech model is still loading; your words follow once it's ready  |
| A waveform                      | Legato is listening                                                   |
| Transcribing...                 | The speech model is writing up what you said                          |
| Fixing words...                 | Your AI model is checking for misheard Dictionary words               |
| Cleaning up...                  | Your AI model is smoothing the text (Cleanup at Medium)               |
| Running your transform...       | A transform is rewriting the text                                     |
| Finishing the last dictation... | An earlier dictation is still being finished, and it pastes first     |
| Pasting...                      | The text is going into the app                                        |
| Pasted 12 words into Outlook    | Done, with Correct and Undo. Turn this off with Confirm after pasting |

Once a step has taken two seconds or more, the bar adds its seconds, so a slow step doesn't look stuck.

## Correct a dictation

Correct on the bar after a paste, or Correct last dictation (Alt+Shift+C) within 10 minutes of it, opens the pasted text on the bar for you to edit. Enter puts your version in place of the paste in the app, fixes the dictation on Home, and teaches your [Dictionary](dictionary.md) the words you changed with the form Legato misheard, so the next dictation gets them right. As Learn from my edits does, they wait under Needs review, or with When it finds a word set to Add automatically they're added straight away, and the bar says so with Undo. When the Dictionary, cleanup or Style changed what the speech model wrote, What was heard shows its own words beside yours. Esc, or a click into another app, leaves the text as it was.

The paste is replaced only while it sits right before the cursor in the same window, as transforms need too. If you've typed after it, or the message was sent, the fix goes on the clipboard instead and is still learned.

## Live transcript

Some models write your words as you speak. The Models page marks them Streaming, and with the Live style the bar shows the words as they come. Other models write everything after you stop, so Live shows the time and transform but no words.

With the bubble on, Settings › Legato bar › Live transcript on screen can show a see-through box at the top or bottom of the screen with the words as you say them. It's off by default and needs a streaming model.

## After you stop talking

Every dictation goes through the same steps, in this order:

1. The speech model writes up the recording. Your Dictionary's pinned and recent words go to models that take a hint, and the ways it mishears your words are fixed afterwards.
2. If you turned on the AI correction pass, your AI model looks for Dictionary words the rules missed.
3. Snippet triggers are swapped for their saved text, which nothing after this changes.
4. Cleanup removes fillers, repeats and false starts, at the level set on the Style page.
5. Style applies the tone for the kind of app you're dictating into.
6. A transform runs, if one is set to run after every dictation or you used Dictate then transform.
7. Legato pastes the text with the paste keys for that app.

The AI steps run only if you set up an AI provider and turned them on. If the provider doesn't answer in time, the text is pasted without AI changes and the bar says why. Home's right rail shows these steps as they're set up now.

You can start the next dictation as soon as the last one is transcribed. Dictations still paste in the order you spoke them.

## Pasting

These settings live in Settings › Output. [Style](style.md) can change the first three for each kind of app.

- **Paste method**: Clipboard (Ctrl+V), the default, works in most apps. Clipboard (Ctrl+Shift+V) and Clipboard (Shift+Insert) suit terminals and some editors. Direct types the text, and None doesn't paste at all, so the dictation only lands in Home.
- **Auto-submit after paste**: presses Enter, Win+Enter or Ctrl+Enter after pasting, to send a message or a prompt. Off by default.
- **Add a trailing space**: adds a space so the next dictation doesn't run into this one. Off by default.
- **Clipboard after pasting**: Don't modify clipboard, the default, puts back what you had copied. Copy to Clipboard leaves the transcript there to paste again.
- **Say "press enter" to send**: ending a dictation with "press enter" pastes it without those words, then presses Enter.
- **A message per paragraph in chat apps**: in apps Style calls personal or work messages, each paragraph is sent on its own.

If an earlier clipboard pastes instead of your words, see [Troubleshooting](troubleshooting.md#previous-clipboard-content-is-pasted-instead-of-the-transcription).

## When a paste fails

A dictation that can't be pasted is always saved in the Scratchpad, and the bar says "Couldn't paste · saved in the Scratchpad". With Open the Scratchpad when a paste fails on (the default, in Settings › Output), the Scratchpad window opens on it so you can copy it or send it again. With it off, a Windows notification says where it went. Home shows the row as not pasted, with the reason Windows gave.

## Apps run as administrator

Windows doesn't let a normal app type into an app running as administrator, such as an admin terminal. Legato doesn't try: it puts the text on the clipboard, saves it in the Scratchpad and says so on the bar.

To dictate into those apps, install Legato with the `.msi` and turn on Settings › General › Work in apps run as administrator. Legato then restarts as administrator, and Windows asks once. It needs a Windows account that's an administrator. See [Install on Windows](install.md#apps-run-as-administrator).

## Cancelled and failed dictations

A dictation you cancel is kept in Home if it had at least a second of speech, so a stray Esc doesn't lose your words. If the model is loaded, Legato writes it up anyway and marks it "cancelled, not pasted". Otherwise the row says "Cancelled before it was transcribed" with Transcribe. Ask's instructions and practice dictations in setup aren't kept.

A dictation the model couldn't write up keeps its audio and says "This one didn't transcribe" with Try again. When an AI step was skipped because the provider didn't answer, the row says so, with Try again. Try again runs the recording through every step again and updates Home; it doesn't paste.

## Speech models on the PC

Speech recognition runs on your PC. These are the models Legato recommends, with the download size the Models page shows:

| Model                    | Languages                        | Size   | What it's like                              |
| ------------------------ | -------------------------------- | ------ | ------------------------------------------- |
| Parakeet Unified EN 0.6B | English                          | 697 MB | Fast and accurate, writes as you speak      |
| Nemotron Streaming 3.5   | 28 languages                     | 716 MB | Writes as you speak, across 28 languages    |
| Canary 180M Flash        | English, German, Spanish, French | 208 MB | Tiny and instant, runs well on any hardware |
| Cohere Transcribe        | 14 languages                     | 1.6 GB | The most accurate of these, and slower      |
| Whisper Medium           | 99 languages                     | 793 MB | The most languages, but may run a bit slow  |

The Models page lists more, from Moonshine Tiny (34 MB) to Voxtral Small 24B, with filters for language, live streaming and translation to English. Models in Legato's models folder or the shared Hugging Face cache show up too; the rescan button finds ones added while Legato runs. [Troubleshooting](troubleshooting.md#manual-model-installation-for-proxy-users-or-network-restrictions) covers installing a model by hand.

Settings › Transcription › Spoken language is Auto by default, which detects the language each time. Pick one if it guesses wrong on short dictations. Translate to English writes English whatever language you speak, with a model that can translate, such as Canary 180M Flash or Whisper Medium.

## A model per kind of app

Each kind of app on the Style page has a Speech model setting, Transcribe with. Inherit uses the model chosen on the Models page. Pick another downloaded model and dictating in those apps switches to it, which takes a moment after dictating elsewhere. This works only while Style is on. See [Style](style.md).

## GPU and CPU

On x64 PCs the model runs on the graphics card through Vulkan; on Arm it runs on the CPU. Settings › Transcription › GPU acceleration is Auto by default: it uses the graphics card when it can and switches to the CPU if the driver fails.

Loading a model onto the graphics card can take from one to over 20 seconds on a laptop. Legato works around that in two ways:

- **A recording that ends during the load**: if it's 25 seconds or shorter, Legato writes it up on the CPU meanwhile.
- **A model left idle**: after Unload model after (5 minutes by default, in Settings › Transcription), a model on the graphics card moves to the CPU and waits there. The graphics card is free, and the next dictation starts at once. Used again, the model loads back onto the graphics card while the CPU copy serves. Left alone, it's unloaded after 30 more idle minutes (or the Unload model after time, if that's longer). With less than 1 GB of memory free, it's unloaded instead of waiting.

A model that already runs on the CPU is unloaded after the set time instead. Never keeps the model loaded, and Immediately unloads it after each dictation.

## While you dictate

Settings › Audio › Music and videos lowers other apps to 5% while you dictate and brings them back when you stop; calls stay as they are. It can also pause players, mute everything or leave them alone. Sound feedback, off by default, plays a short sound when Legato starts and stops listening.
