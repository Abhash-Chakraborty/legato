# Transforms, Ask and your AI provider

Rewrite a selection or your last dictation with your own AI model, or ask it something by voice. Here is what each feature sends, and when.

## Turn them on

Transforms need an AI provider, set up in Settings › AI (see [Add your AI provider](#add-your-ai-provider) below). They start off on a new install. Turn them on with the switch on the Transforms page, or in Settings › Features › Transforms and Ask. While they're off, their shortcuts and Ask do nothing, and no text goes to your AI model for them.

Speech recognition never needs a provider. Dictation, the Dictionary, snippets and Light cleanup all run on your PC without one.

## Run a transform

Every transform works on the text you select in any app. With nothing selected, it works on your last dictation. If that dictation went into the window in front and still sits right before the cursor, Legato selects it again so the result replaces it; otherwise the result goes in at the cursor.

There are two ways to pick one:

- **Its own shortcut.** The transforms Legato ships come with Win+Alt+1 to Win+Alt+5. Set or change one with Set shortcut on its card.
- **The picker.** Transform the selection (Win+Alt+T by default) opens a list on the bar. Use ↑ and ↓ to choose, Enter to run and Esc to close. It lists the transforms that are on, in the order of the cards on the Transforms page; drag a card to change it.

A transform on the selection waits up to 30 seconds for the model. Nothing is pasted until it answers, the selection stays as it was if it doesn't, and Cancel on the bar stops it. In the [Scratchpad](scratchpad.md) window, Transform rewrites the selected part of a note, or the whole note.

## The transforms Legato ships

| Transform       | What it does                                                        | Starts |
| --------------- | ------------------------------------------------------------------- | ------ |
| Polish          | Fixes grammar and self-corrections, tightens wording, keeps my tone | On     |
| Prompt engineer | Turns a spoken request into a clear prompt for Claude or ChatGPT    | On     |
| Bullet points   | Turns a ramble into a tight list, every detail kept                 | On     |
| To German       | Natural German; names, numbers and code untouched                   | On     |
| Commit message  | A conventional commit from a spoken summary                         | On     |
| Reply draft     | Drafts a reply to the selected message in my voice, below it        | Off    |

Polish and Prompt engineer are built in: you can change them but not delete them. Each card shows how often the transform ran and when it last did.

## Make your own

New transform opens the editor:

- **Name**, **Description** and **Prompt**: what you ask the model to do with the text.
- **Insert** adds a variable to the prompt: `{{app}}` (the app the text is in), `{{dictionary}}` (your Dictionary words, most used first) or `{{clipboard}}` (what's on the clipboard). The clipboard is read only when the prompt asks for it.
- **Model**: the one in Settings › AI, or its own provider and model. A transform's own model uses the key saved for that provider.
- **Result**: Replace the text, Add below (the selection stays and the result goes under it) or Copy (the result goes on the clipboard and nothing changes).
- **Show changes**: the bar shows what changed, with Keep and Undo.
- **Shortcut**: runs it on the selection in any app, once it's saved.

Test runs the transform from the editor. Try it on the Transforms page runs any transform on text you type or a recent dictation and shows the changes word by word. Recent runs lists every run with what happened to it: Kept, Undone, Copied or Failed.

## After every dictation

Under After every dictation, Run after every dictation picks a transform that rewrites each dictation before it's pasted. Three rules skip it:

| Rule                       | Default | Why                                                                |
| -------------------------- | ------- | ------------------------------------------------------------------ |
| Skip in code and terminals | On      | Code editors and terminals get exactly what you said               |
| Skip in AI prompts         | On      | Claude, ChatGPT and other AI apps rewrite prompts anyway           |
| Skip dictations under      | 5 words | Short replies are pasted as you said them; 0 runs it on everything |

To transform just one dictation, use **Dictate then transform** (Ctrl+Shift+Space by default). Dictate then transform runs picks which transform it uses; Polish unless you change it.

If a transform can't run, your words are pasted as you said them and the bar says why.

## Ask

Ask (Win+Alt+A by default) is for one-off requests. Select some text, hold the shortcut and say what to do, like "make it shorter", "answer in German" or "turn this into steps". Let go and the answer replaces the selection. Without a selection, Ask works on your last dictation.

Your spoken instruction is transcribed on your PC like any dictation; only the words go to the model. Ask has its own switch on the Transforms page.

## Keep and Undo on the bar

After a transform with Show changes on, the bar shows the text before and after for two seconds, with Keep and Undo. Point at it to keep it open. The change is already in place; Keep just closes the bar.

Undo presses Ctrl+Z in the app, which still has focus since the bar never takes it. After a transform the original text also goes on the clipboard, in case the app can't undo the paste. Show the last changes (Win+Alt+O by default) brings the last before and after back.

Settings › Legato bar › Show changes after a transform turns this off for every transform. Home keeps every Before and After either way.

## Add your AI provider

Settings › AI › Provider lists these:

| Provider    | Runs where           | Key                                    |
| ----------- | -------------------- | -------------------------------------- |
| Ollama      | On this PC           | None                                   |
| OpenAI      | OpenAI's servers     | From platform.openai.com               |
| Anthropic   | Anthropic's servers  | From console.anthropic.com             |
| OpenRouter  | OpenRouter's servers | One key for models from many providers |
| Groq        | Groq's servers       | From console.groq.com                  |
| Your server | Wherever it runs     | Optional                               |

Your server is any OpenAI-compatible server, such as LM Studio or vLLM. Give its Base URL (ending in `/v1`) and its API format: OpenAI-compatible or Anthropic Messages. Ollama's Base URL can point at another PC too.

Legato sends text and keys only over https, with two exceptions for plain http: an address on this PC, or a NetBird or Tailscale IP, where the tunnel encrypts it. A provider on this PC is marked Local, and text sent to it never leaves the PC.

API keys are kept in Windows Credential Manager, not in Legato's settings file, and each is sent only to its provider. Pick a model from the list (Fetch models asks the provider for it) or type its exact name, then choose Test connection. The status card at the top of Settings › AI says how the provider last answered and what to do about a problem.

## A model for each task

Model for each task can give three jobs their own provider and model:

- **Dictation**: Cleanup at Medium and the correction pass. A small, fast model keeps dictation quick.
- **Transforms and Ask**: transforms without a model of their own, and the Ask shortcut.
- **Notes**: summaries, titles and tags, questions and follow-ups. A larger model writes better summaries, and a meeting can wait a few seconds for one.

Each is Same as above unless you change it.

## When the AI is slow or down

A dictation never waits long for the AI. Give up on a dictation after (6 seconds unless you change it, from 2 to 120) sets how long; after that the text is pasted without AI changes, and the bar says which provider didn't answer. Home marks that dictation with the reason and Try again. Notes requests have their own limit, Give up on a Notes request after (120 seconds by default), since a long meeting takes longer.

## What is sent and when

Audio is never sent anywhere. Only text goes to your provider, and only for the uses you turn on:

| What runs                            | When                                                                                                                                  | What the provider gets                                                                                                                       |
| ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| Correction pass with your Dictionary | Each dictation, when on in Settings › AI › Use AI for (off by default) and the dictation has at least 8 words (Skip short dictations) | The dictation and your Dictionary: each word, how it's heard and what kind of word it is                                                     |
| Cleanup at Medium                    | Each dictation of 8 words or more, when Style's cleanup is set to Medium                                                              | The dictation after Light cleanup, with snippets held back as placeholders, plus your Dictionary when the correction pass is on              |
| A transform                          | When you run it, or after each dictation if you picked one                                                                            | The text, the transform's prompt, and `{{app}}`, `{{dictionary}}` (up to 100 words) or `{{clipboard}}` only if the prompt uses them          |
| Ask                                  | When you use it                                                                                                                       | Your spoken instruction as text, and the selection or last dictation                                                                         |
| Note summary                         | After a note stops, and again after corrections, when Notes summaries and Ask is on                                                   | The transcript with who said each line, the names of people in the meeting, up to 80 Dictionary words, your name and the tags your notes use |
| Ask about a note, What did I miss?   | When you ask                                                                                                                          | The transcript, or its last few minutes                                                                                                      |
| Ask your notes                       | When you ask                                                                                                                          | The summaries, decisions and next steps of your summarised notes                                                                             |
| Find misheard words                  | When you choose it                                                                                                                    | The transcript, its title and up to 200 Dictionary words                                                                                     |
| Draft follow-up                      | When you choose it                                                                                                                    | The note's summary                                                                                                                           |

Settings › AI ends with a line saying where this text goes: to your PC for a local provider, or to the provider's address. Settings › Devices › Share AI keys with your phone lets Legato for Android run transforms with your key; see [Legato for Android](android.md#transforms) and [Privacy and your data](privacy.md).
