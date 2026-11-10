# Getting started

Setup checks your microphone, downloads a speech model for your languages and lets you practise. Then hold Ctrl+Space in any app and talk.

## Before you start

Install Legato first. [Install on Windows](install.md) says which file to download. When Legato opens for the first time, setup runs in its window. It has four steps, named at the top: Microphone, Model, Practice and Your words.

If you close Legato part-way through, setup opens on the same step next time. Skip setup leaves it and opens Legato as it is.

If you used Handy before, Legato brings over its settings, history and recordings on first start. Handy has to be closed for that, and Legato offers to open Windows Settings so you can uninstall it.

## Check your microphone

Legato listens only when you ask it to, and turns speech into text on this PC.

1. Legato checks that Windows lets desktop apps use the microphone. If it says "Windows is blocking the microphone", choose Open privacy settings, turn on "Let desktop apps access your microphone", then come back and choose Check again.
2. Pick a microphone. Windows default follows whichever microphone Windows uses.
3. Say a sentence or two. When the level moves and setup says "Legato can hear you", choose Continue.

If nothing comes through, pick another microphone or check that it isn't muted. You can change it later in Settings › Audio › Microphone.

## Pick a speech model

Setup offers two models. Which one comes first depends on your Windows input languages, the keyboard languages Windows lists in its language bar. The cards say whether each is faster or more accurate, which languages it covers and how much it downloads. On a PC with a graphics card, setup names the card, since the model runs on it.

| Your Windows input languages     | First choice                                                               | Second choice                     |
| -------------------------------- | -------------------------------------------------------------------------- | --------------------------------- |
| English only                     | Parakeet Unified EN 0.6B (697 MB)                                          | Nemotron Streaming 3.5 (716 MB)   |
| Another language Nemotron covers | Nemotron Streaming 3.5, 28 languages (716 MB)                              | Parakeet Unified EN 0.6B (697 MB) |
| A language outside Nemotron's 28 | Cohere Transcribe (1.6 GB) or Whisper Medium (793 MB), whichever covers it | Parakeet Unified EN 0.6B          |

A model picked for another language says so: "Recommended, because German is one of your Windows input languages". Continue downloads the model and loads it. See all models opens the Models page once setup is done, with every model Legato can run.

You can switch models any time on the Models page, and Style can pick a different one for each kind of app. [Dictation](dictation.md#speech-models-on-the-pc) compares them.

## Practise in three scenes

Next come three copies of everyday apps: a chat reply, an email and a shopping list in a notes app. Each shows a line to read out loud, ums and a self-correction included. Nothing is sent anywhere, and these practice dictations aren't saved to Home.

1. Hold the shortcut (Ctrl+Space unless you changed it) and read the line.
2. Let go. The text lands in the scene's message, email or page.
3. Under What Legato did, see what changed: the filler words it removed, the words it dropped because you corrected yourself, the list it made in the notes app and the style it used for that kind of app.

While setup is open, the shortcut records only in a practice scene, so nothing is pasted behind the setup window. If the shortcut does nothing, open Not working?. Another app may already use those keys, since input switchers and launchers often do. Test without the keyboard lets you hold the mic button in the scene instead.

## Teach Legato a word

The email in the practice has two names a speech model often gets wrong. If one came out wrong, setup shows how Legato wrote it and asks for the right spelling. Add puts it in your Dictionary, and from then on it's spelled your way in every app. Skip leaves it out.

## You're set

The last screen lists the keys to remember and how Legato is set up, with a link back to each step.

- **Start Legato when Windows starts** is ticked when you reach this screen; untick it if you'd rather start Legato yourself. It's the same as Launch at login in Settings › General, and Start hidden there decides whether Legato opens to the tray or to its window.
- **How much of Legato do you want?** Just dictation gives you dictation, the Dictionary, Snippets and Style. Everything also turns on Notes with call offers, the Scratchpad, the bubble on the desktop and Insights. You can change this later in Settings › Features.

Choose Open Legato. From now on Legato runs in the tray, and the window can stay closed.

Skipped setup, or want to practise again? Settings › About › Run setup again starts it from the microphone check. Run this way, it leaves Start Legato when Windows starts as you set it.

## Get to know Legato

Setup teaches dictation. The rest is on Home, in a short list called Get to know Legato. It shows one feature you haven't tried at a time: what it's for, its shortcut, and Show me, which takes you there. All lists every row.

A row ticks itself once Legato can see the feature in use: a word in your Dictionary, a snippet, a note, a paired phone, a calendar, an AI model. For the others, Show me ticks it. Skip ticks a row you don't want, and the list goes away once every row is ticked or you close it. Settings › About › Show the list on Home brings it back.

## Your first dictation

1. Click into a text field in any app, such as a chat or an email.
2. Hold Ctrl+Space and talk.
3. Let go. Legato writes up what you said on this PC and pastes it where the cursor is.

To keep talking without holding the keys, tap Ctrl+Space instead. Legato listens until you tap it again. Press Esc while it listens to stop without pasting anything.

The first dictation after Legato starts may show "Loading model..." for a moment while the speech model loads. Every dictation also lands in Home, where you can copy it, play it back or fix a word Legato got wrong.

## Default shortcuts

These work in any app, even with Legato's window closed. Change them in Settings › Shortcuts.

| Shortcut                    | Keys             | What it does                                                        |
| --------------------------- | ---------------- | ------------------------------------------------------------------- |
| Dictate                     | Ctrl+Space       | Hold to talk and let go to paste, or tap to start and tap to stop   |
| Cancel                      | Esc              | Stops a recording without pasting; only while Legato is listening   |
| Dictate then transform      | Ctrl+Shift+Space | Dictates, then runs a transform before pasting (needs Transforms)   |
| Add selection to Dictionary | Win+Alt+W        | Adds the word selected in any app, spelled exactly as selected      |
| Transform the selection     | Win+Alt+T        | Picks a transform for the selected text (needs Transforms)          |
| Ask                         | Win+Alt+A        | Hold and say what to do with the selection (needs Transforms)       |
| Show the last changes       | Win+Alt+O        | Shows what the last transform changed, with Undo (needs Transforms) |
| Paste last dictation        | Alt+Shift+Z      | Pastes your last dictation again                                    |
| Copy last dictation         | Alt+Shift+X      | Puts your last dictation on the clipboard                           |
| Correct last dictation      | Alt+Shift+C      | Opens your last dictation on the bar to fix; the fix is learned     |
| Open Scratchpad             | Win+Alt+S        | Press to open the Scratchpad; hold to dictate into a new note       |
| Start or stop notes         | Win+Alt+M        | Starts recording a meeting, or stops it (needs Notes)               |
| Screenshot into notes       | Win+Alt+P        | Puts a screenshot in the note that's recording (needs Notes)        |
| Talk to the companion       | Win+Alt+I        | Hold and speak to Inyon's companion agent (needs an Inyon pairing)  |

If two shortcuts share the same keys, Settings says which one does nothing so you can give it other keys.

## The Legato bar

While you dictate, a small bar at the bottom of the screen shows that Legato is listening, with Cancel, a waveform and the words as you say them when your model writes as you speak. After you stop, it names the step Legato is on, then says how many words went where, with Undo.

Settings › Legato bar changes its style and position, turns it into an island that hangs from the top of the screen, or swaps it for a bubble on the desktop. [The bar, the island and the bubble](dictation.md#the-bar-the-island-and-the-bubble) has the details.

## Where to go next

- [Dictation](dictation.md): shortcuts, the bar, pasting and the speech models.
- [Dictionary and snippets](dictionary.md): names spelled your way, and short phrases that paste saved text.
- [Style](style.md): the tone and paste keys for each kind of app.
- [Transforms, Ask and your AI provider](transforms.md): rewrite text with your own AI model, if you want one.
- [Notes and meetings](notes.md): voice memos and meeting notes.
- [Home, Scratchpad and Insights](scratchpad.md): your history and the notes window.
- [A map of Settings](settings.md) and [Privacy and your data](privacy.md).
- [Legato for Android](android.md) and [Troubleshooting](troubleshooting.md).
