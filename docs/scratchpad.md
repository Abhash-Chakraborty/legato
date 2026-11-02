# Home, Scratchpad and Insights

Home lists your dictations with their recordings, the Scratchpad holds notes you type or dictate, and Insights counts how you dictate.

## Home

Home lists what you've dictated, newest first, grouped by day. The line at the top counts today's dictations, words and time of speech.

Each row shows the text and where it went: the app it was pasted into, how long the paste took, the transform it ran and how many words were fixed. Hover a row to play the recording, copy the text or open More actions:

| Action                             | What it does                                                         |
| ---------------------------------- | -------------------------------------------------------------------- |
| Transcribe again                   | Runs the recording through the speech model and the same steps again |
| Fix a word                         | Corrects a word and, if you like, teaches it to the Dictionary       |
| Copy the text before the transform | For a transformed dictation, the words as you said them              |
| Star                               | Keeps it under the Starred filter                                    |
| Save audio as WAV…                 | Saves the recording as a file                                        |
| Delete                             | Removes it, with Undo for a few seconds                              |

Search (press `/` or Ctrl+F) looks through transcripts and app names. The filters show All, Starred, Fixed or Transformed dictations, or one app's. Rows also take keys: ↑ and ↓ (or J and K) move, Enter copies, F fixes a word, S stars, Del deletes.

The rail at the right shows what's set up right now (model, microphone, spoken language, AI and your Dictate shortcut) and the steps a dictation goes through, each a link to where you change it.

### Dictations that need another go

A dictation that went wrong stays in Home:

- **A dictation that didn't transcribe** says "This one didn't transcribe." with Try again.
- **A dictation you cancelled** stays too, marked "Cancelled before it was transcribed.", with Transcribe.
- **A dictation the AI skipped** was pasted without its AI step, because your AI provider wasn't reachable, didn't answer in time or turned the request down. The row says why, such as "Ollama isn't running, so the AI step was skipped.", with Try again.

Try again and Transcribe run the recording through the speech model and the same steps as the original dictation, with the style of the app it went to. The row updates; nothing is pasted.

A dictation that couldn't be pasted shows "Not pasted" with the reason, and its text is waiting in the Scratchpad (see [Failed pastes](#failed-pastes)). Dictations from Legato for Android show On and the phone's name; their audio stays on the phone.

### How long Home keeps things

Settings › Privacy › Keep at most sets how many dictations Home keeps, and Keep recordings sets how long they and their recordings stay. Settings › Privacy › Remember which app I dictated into adds the app to each row so Home can filter by it; turned off, Legato keeps no app names or window titles. See [Privacy and your data](privacy.md).

## The Scratchpad

The Scratchpad is a small floating window of notes you can type or dictate into instead of the app in front. Open it with Open Scratchpad (Win+Alt+S by default), from the island or the bubble, or with `legato --open-scratchpad`. Hold the shortcut instead of tapping it to dictate straight into a new note.

In the window:

| Keys           | What they do                           |
| -------------- | -------------------------------------- |
| Ctrl+Space     | Dictate here, at the cursor            |
| Ctrl+Enter     | Send the note to the app you came from |
| Ctrl+T         | New tab (Ctrl+N works too)             |
| Ctrl+W         | Close the tab                          |
| Ctrl+Tab       | Next tab                               |
| Ctrl+F         | Search notes                           |
| Ctrl+B, Ctrl+I | Bold, italic                           |

Up to five notes stay open as tabs. The rail on the left shows your notes, search, Transforms and Formatting. Notes are written in Markdown; the formatting bar adds headings, bulleted and numbered lists, checklists and code. Switch between Write and Preview, and tick checklist items off in Preview.

Keep on top, a button at the top of the window and a switch on the Scratchpad page, floats it above other apps until you close it. Every note saves as you type.

### Send to

Send to pastes the note into the app that was in front when the window opened. To send it elsewhere, open the picker and choose any open window. With Legato paired with Inyon, its agent sessions are listed there too; see [Connections](connections.md).

Transform, in the rail, rewrites the selected part of a note, or the whole note when nothing is selected, with one of your [transforms](transforms.md). Esc stops it.

### Version history

A version is kept each time a note is saved, up to 50 per note. Version history, in the note's menu, shows them; Restore this version brings one back.

### Failed pastes

When Legato can't paste a dictation, the text goes into a new Scratchpad note marked with the app it was meant for, and the bar says so. With Settings › Output › Open the Scratchpad when a paste fails on (it is unless you change it), the window opens on that note so you can copy it or send it again. For an app run as administrator, the text is copied as well; see [Install on Windows](install.md#apps-run-as-administrator).

### The Scratchpad page

The Scratchpad page in the main window lists every note, with All and Pinned tabs and search across titles and text. Edit a note in place, or choose Open in window. When opened picks what the shortcut opens: Resume last note, New note or Last pinned note.

Pin the notes you come back to. A deleted note goes to Recently deleted with its versions, and you can restore it for 30 days. Settings › Features › Scratchpad hides the page, its shortcut and its buttons, though a failed paste still lands there. Scratchpad notes sync with [Legato for Android](android.md).

## Insights

Insights is worked out on this PC from your dictations. Its numbers stay when you delete transcripts. Pick a range at the top: 7 days, 30 days or All.

**Usage**

- **Words**: how many you dictated, the time of speech, and the change from the previous 7 or 30 days, as a chart by day or week.
- **Speed**: your words a minute, against typing at about 40 wpm, and the trend over recent weeks. It shows once you've spoken for ten seconds in the range.
- **Fixes**: filler words removed, Dictionary corrections, snippets expanded and transforms run.
- **Where your words go**: words by app and by category. Click an app to see its dictations on Home. This needs Remember which app I dictated into.
- **Activity**: words a day, and your streak of days in a row. Click a day to open it on Home.

**Habits**

- **Words Legato fixes most**, from your Dictionary, with Open Dictionary.
- **Filler words removed**, word by word.
- **When you dictate**, by hour, with your busiest stretch.
- **Average dictation**, in words and seconds of speech.
- **Suggested words**: terms the model wrote more than one way in the last 30 days. Add one and every version comes out the same.

**Accuracy & speed** compares the speech models you've used: words and dictations, how long each takes to transcribe once you stop speaking, how much ran on the GPU, the average time to paste, and fixes per 1,000 words. Use switches to another model.

Settings › Features › Insights hides the page.
