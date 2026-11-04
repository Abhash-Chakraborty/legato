# Notes and meetings

Record meetings and voice memos on your PC, with a live transcript, voices told apart and, if you add an AI provider, a summary.

## Record a meeting or a memo

Open Notes in the sidebar and choose Record. There are two kinds of note:

| Kind       | What it records                                                                                                                     |
| ---------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| Meeting    | Your microphone and the call's sound, as two tracks, so the transcript knows what you said (You) and what came from the call (Them) |
| Voice memo | Only your microphone                                                                                                                |

For a meeting, Other side picks the sound to record. Pick the call app playing now and only that app is recorded, so music or a video elsewhere stays out. Pick Everything this PC plays and notifications and videos are recorded too. If your Windows can't record one app on its own, Legato records everything and says so.

Each track is cut into lines at pauses, and every line is transcribed on your PC while the recording runs. The note is ready a few seconds after you stop. When the call comes back through your speakers and the microphone picks it up, Legato drops that echo instead of writing the line twice.

You can also start from anywhere:

- **Start or stop notes** (Win+Alt+M by default) starts a meeting with the call app you're in, or stops the recording.
- Take notes on the bubble, or Notes on the island, which also start a meeting with the call app you're in. See [Dictation](dictation.md) for the bubble and the island.

Settings › Features › Notes turns the whole feature off. That hides the Notes page, its shortcut and its buttons.

## Calls Legato notices

With Settings › Context & calls › Detect calls and offer notes on, Legato notices when Teams, Zoom, Slack, Discord, WhatsApp or Telegram starts a call, and Google Meet, Teams or Zoom in a browser tab. Windows tells Legato which app is using the microphone; Legato doesn't listen to the audio to find out. For browser calls it reads the window title, never the page, which needs Read browser window titles.

When a call starts, the bar asks "Take notes? They stay on this PC." with Start notes, Not now and Never here. You can change how that goes in Settings › Context & calls:

| Setting                       | What it does                                                                                                                     |
| ----------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| Apps to watch                 | Which apps bring up the card. WhatsApp and Telegram count once a call passes 20 seconds, so voice messages don't                 |
| Start notes automatically     | Starts after a countdown (10 seconds unless you change it); Not now still cancels                                                |
| After Not now                 | Ask again in a few minutes, or not again this call                                                                               |
| Stop when the call ends       | Stops about 20 seconds after the app lets go of the microphone (on unless you turn it off)                                       |
| Stop recording after          | A limit for every note, 2 hours unless you change it. Two minutes before, the bar offers Keep recording, which adds half an hour |
| Open the note when it's ready | Opens Notes on the summary when it's done                                                                                        |

When a call comes back in the same app within 10 minutes of its note stopping, the card offers Continue notes and New note instead. Continue notes records as usual and joins the earlier note when it's ready, as [Merge two notes of one meeting](#merge-two-notes-of-one-meeting) does. With Start notes automatically on, the countdown continues the note.

Test detection shows the card as if a call had started, without recording anything. With a calendar connected, the bar can also offer Join and Join with notes a minute before a meeting; see [Connections](connections.md).

If a recording ends with only a few words, the bar asks whether it was started by mistake, with Keep and Discard. Left alone, the note is kept. Discard moves it to Recently deleted.

## While it records

The Notes page shows a strip with the time, Pause, Resume and Stop. Paused time isn't recorded. Live transcript shows the lines as they're written, a moment after each pause in speech, and while it's shown the line still being spoken previews too. Hide folds it away (or untick Show the live transcript when you start).

Notes started from the bubble or the island open the Notes side panel instead, docked at the right of the screen. It has the live transcript, a box for your own notes, Pause and Stop. What did I miss? recaps the last few minutes with your AI. Closing the panel leaves the recording running; the bar's Notes pill, or a green glow on the bubble, shows it's still going.

My notes is a private text box on every note. Type while you listen, or choose Dictate here. What you write there is saved with the note and included when you copy or export it.

> **Note:** Settings › Legato bar › Hide from screenshots and screen sharing keeps the bar, the bubble and the Notes side panel out of a screen you share. Still let people know you're recording.

## Telling voices apart

A meeting's lines can each get the voice that said them: Speaker 1, Speaker 2, or a name you give. This needs the speaker model (NVIDIA Streaming Sortformer, about 139 MB, run on your PC). Your first meeting downloads it, or download it in Settings › Context & calls › Speaker model. Tell speakers apart turns the feature off.

Voices are told apart every so often during the meeting and once more after it stops. The model holds four voices at a time. When a meeting fills all four, Legato listens to it again in one-minute windows and joins the voices up, so a bigger call keeps people apart.

To name someone, click their speaker label in the transcript and type a name. To give one line to someone else, use Said by in the line's menu. To run it on a finished note, such as one recorded before the model was downloaded, choose Find speakers again in the transcript's ⋯ menu.

With Remember voices on, naming a speaker keeps an 8-second clip of their voice. In later meetings, a voice that matches gets that name, shown with a dot. The clips stay on this PC and are never synced or sent anywhere. Settings › Context & calls › Saved voices lists them, and Forget removes one.

In Google Meet, Legato also reads the names Meet shows in the call window while the note records. Only names are read. They join the note's people, so the summary can name them and naming a voice offers them.

## The summary

With an AI provider set up in Settings › AI, and Notes summaries and Ask turned on there, each note gets a summary after it stops. It has the main points, Decisions, Next steps with who owns each one, and Details that play the recording from the moment they came up. A note shorter than about 40 words is too short to summarise.

The summary also names the note while it still has its default title, and tags it while it has no tags. A title or tags you set yourself are never changed.

The summary is shaped for a kind of meeting, such as Team meeting, One-on-one, Sales call or Lecture. Pick another under Kind of meeting and it's written again that way, and later summaries of that note keep it. Regenerate writes it again with the AI as it's set up now.

Tick next steps off as they're done. Next steps on the Notes page lists them from every note: Yours, Everyone's and Done. Set your name in Settings › General › Your name so steps meant for you say You.

When you correct the transcript, the summary updates on its own about 20 seconds after your last correction. The next steps, and a title the summary gave, come with it; steps you ticked stay ticked. Update now does it straight away.

Without an AI provider, notes still get a full transcript. [Transforms, Ask and your AI provider](transforms.md#what-is-sent-and-when) lists what a summary sends.

## Play it back word by word

Choose Play recording on a note. The line being played is marked, and so is the word being said in it. Click a word to play from there, or a line's time to play from the start of the line. Space plays and pauses. The transcript scrolls along with the recording unless you scroll it yourself.

Exact word times come from speech models that give them, such as Parakeet. With Whisper, Legato knows only when each line starts and ends, so the highlight moves through the line evenly.

## Fix what it misheard

- **Correct a line:** double-click it, or choose Edit in its menu. Change one word or several; the word times follow the new text. Line changed comes with Undo.
- **Fix it everywhere:** when your correction swapped one word, Legato offers to fix the same word in the rest of the note (Fix 3 more lines, say) and Add to Dictionary so future dictations and notes get it right.
- **Add a word from the transcript:** select a word, or a few, and choose Add to Dictionary. Type how it should be spelled, and Legato writes it that way in every line of the note and hears it that way from then on.
- **Find misheard words:** in the transcript's ⋯ menu, your AI reads the transcript for words that look misheard, using your Dictionary. Each fix is shown in its line and ticked; untick any that were right, then choose Fix.

A line that couldn't be transcribed shows a count with Try again. See [Dictionary and snippets](dictionary.md) for how the Dictionary works.

## Ask, follow-ups and scheduling

- **Ask about this note** on the Summary tab answers from the transcript. Times in the answer open the transcript at that line.
- **Ask your notes** on the Notes page asks one question across the summaries of every note, such as "What did I promise people?".
- **Draft follow-up** writes a follow-up email from the summary and opens it in the [Scratchpad](scratchpad.md).
- **Schedule a follow-up**, in the note's More actions, fills in a new event and opens Google Calendar, Outlook or Outlook.com in your browser. You save it there. Legato never signs in to your calendar.

## Copy, export and organise

A note's More actions menu has:

- **Copy for Google Docs**, which copies the note with its headings and lists, ready to paste into a Google Doc
- **Save as Word document** and **Export as Markdown**
- **Copy summary** and **Copy transcript**
- **Merge with another note…**, for one meeting saved as two notes (see below)
- **Move to folder** and **Delete**

To keep a Word copy of every note in a folder, such as your Google Drive or OneDrive folder, see [Connections](connections.md). Neither copy nor export goes over the network.

Folders hold notes you group together. Make one with New folder…; deleting a folder keeps its notes. Tags sit under the title, and Add tag offers the tags your other notes use. Show filters the list by Meetings, Memos or a tag, and search covers titles, tags, summaries, transcripts and your own notes.

A deleted note goes to Recently deleted with its recording. You can restore it for 30 days, after which it's gone for good.

## Merge two notes of one meeting

When a call drops for longer than about 20 seconds, its note stops. If the call comes back within 10 minutes, Continue notes on the card adds to the same note. Otherwise, or if you started a new note anyway, the meeting is in two notes. To put them back together, open either note and choose More actions › Merge with another note…. The list offers notes recorded within 12 hours of it, with the nearest one before it picked.

The two become the note that started first. Its recording carries on after a second of silence with the later one, the transcript and word times move along with it, and voices told apart in either keep their names. A voice Legato recognised in both keeps one number. Tags, tasks, attendees and My notes join, and the summary is written again over the whole meeting.

Undo on the message that follows puts both notes back as they were. If the earlier note's audio has already been deleted (see below), the later note's recording can't be kept.

## How long audio is kept

Settings › Context & calls › Keep note audio deletes recordings older than the time you pick: 1 week, 1 month, 3 months (the default), 1 year or Forever. The transcript, summary and tags stay. Once its audio is gone you can't play a note back or name its speakers.

Notes sync to Legato for Android with their transcripts, summaries and action items; the recordings stay on the PC. See [Legato for Android](android.md#notes-and-memos) and [Privacy and your data](privacy.md).
