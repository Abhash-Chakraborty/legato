# Style

Style formats each dictation for the kind of app you're dictating into: the tone, the paste keys, Enter after pasting, the trailing space and the speech model.

## How Legato knows the app

When a dictation starts, Legato reads the window in front: the program, and in a browser the tab's title. Each app or website belongs to one kind of app, and that kind decides the style. Reading the window is a few Windows calls, done on this PC.

> **Note:** Style works with Settings › Privacy › Remember which app I dictated into turned off. Legato still looks at the app in front while you dictate, and keeps no record of it afterwards.

Legato knows these apps out of the box:

| Kind of app       | Desktop apps                                                                                                                   | Websites, by tab title                 |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------- |
| Personal messages | WhatsApp, Telegram, Discord, Signal                                                                                            | WhatsApp, Telegram, Discord, Instagram |
| Work messages     | Slack, Microsoft Teams                                                                                                         | Slack, Microsoft Teams, LinkedIn       |
| Email             | Outlook (classic and new), Thunderbird, Windows Mail                                                                           | Gmail, Outlook                         |
| Documents         | Word, OneNote, Notion, Obsidian                                                                                                | Notion, Google Docs                    |
| AI prompts        | Claude, ChatGPT, and Claude Code in a terminal                                                                                 | Claude, ChatGPT, Gemini, Perplexity    |
| Code & terminals  | VS Code, Cursor, Windsurf, Zed, Visual Studio, IntelliJ IDEA, Windows Terminal, WezTerm, Alacritty, PowerShell, Command Prompt | GitHub                                 |
| Other             | Everything else                                                                                                                |                                        |

Websites are recognised in Chrome, Edge, Firefox, Brave, Opera, Vivaldi, Arc and Zen. Claude Code counts as an AI prompt when a terminal's title says Claude Code.

The top of the Style page shows the app and tone of your last dictation. How apps are matched explains the same rules.

## Move an app to another kind

Each kind of app lists its apps, with Edit apps to change them. Changes save as you go, and they win over the built-in lists.

- **Move here** takes an app from Other apps seen recently, the apps you've dictated into lately.
- **Add website** adds a site by text in its browser tab's title, such as Linear or Jira.
- **Pick a window**: click it, then switch to the app within 3 seconds, and Legato adds whatever comes to the front.
- **Move … to Other** takes an app out of this kind.
- **Restore defaults** puts back the built-in apps for this kind.

## Kinds of app and their defaults

Each kind offers three tones. New installs start with these:

| Kind of app       | Tones offered                | Starts with | Paste settings that differ from Settings › Output                     |
| ----------------- | ---------------------------- | ----------- | --------------------------------------------------------------------- |
| Personal messages | Formal, Casual, Lowercase    | Casual      | None                                                                  |
| Work messages     | Formal, Casual, Lowercase    | Formal      | None                                                                  |
| Email             | Formal, Casual, Excited      | Formal      | After pasting: Nothing; Trailing space: Off                           |
| Documents         | Formal, Casual, Notes        | Formal      | After pasting: Nothing                                                |
| AI prompts        | As said, Tidy, Structured    | Tidy        | None                                                                  |
| Code & terminals  | As said, Lowercase, Sentence | Lowercase   | Paste with: Shift+Insert; After pasting: Nothing; Trailing space: Off |
| Other             | Formal, Casual, Lowercase    | Casual      | None                                                                  |

## Tones

Tones are rule-based and applied after cleanup. No AI is involved.

| Tone       | What it does                                                                          |
| ---------- | ------------------------------------------------------------------------------------- |
| Formal     | Capitals and full punctuation. In email, the greeting and sign-off on their own lines |
| Casual     | Capitals, lighter punctuation. In email, one paragraph                                |
| Lowercase  | No capitals, lighter punctuation                                                      |
| Excited    | Warmer, exclamation marks allowed (email)                                             |
| Notes      | Lists when you enumerate, no final periods (documents)                                |
| As said    | Exactly what cleanup left                                                             |
| Tidy       | Sentences and capitals only                                                           |
| Structured | Paragraphs, numbered lists when you enumerate (AI prompts)                            |
| Sentence   | A capital and a final period (code)                                                   |

Try with your words, under each kind, shows what Legato would paste for a sentence you type. It starts from your last dictation in that kind of app.

## Cleanup for every dictation

Cleanup runs on every dictation in every app, before the tone. Pick a level on the Style page:

| Level  | What it does                                                                            |
| ------ | --------------------------------------------------------------------------------------- |
| None   | Pastes what the model heard, with Dictionary fixes and snippets only                    |
| Light  | Removes fillers, repeats and false starts; writes numbers as digits. No AI. The default |
| Medium | Light, then your AI smooths the grammar without changing what you said                  |

Medium needs an AI provider (see [Transforms, Ask and your AI provider](transforms.md)). Your model is told which app the text goes to and what kind it is, your name, and how your Dictionary spells its words. It keeps code and commands as said in a code editor, cleans up a prompt for an AI chat without answering it, and keeps the language you spoke. The page shows which model it uses, and its example shows only the Light rules, since the AI step runs on real dictations.

Light is made of rules you can switch one by one. All are on by default:

| Rule                | What it does                                                                                                                       |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| Remove filler words | Drops um, uh and other hesitations. The built-in list adds fillers for the language you dictate in; editing it makes your own list |
| Repeated words      | "the the" becomes "the"                                                                                                            |
| Numbers as digits   | "twenty five percent" becomes 25%, "twenty first" becomes 21st. "A million" and numbers under ten stay words                       |
| Spoken punctuation  | Say "comma", "period", "question mark" or "new line" to type them, and "github dot com" for github.com                             |
| Self-corrections    | "at 3, actually 4" keeps 4; "scratch that" drops what came before it                                                               |
| Lists               | "first … second … third" becomes a list where the kind of app lays one out                                                         |
| Paragraphs          | A long dictation breaks into paragraphs where the thought changes; never in code                                                   |

Where lists appear depends on the kind of app and tone. Documents get bullet points, the Notes tone gives bullets and Structured gives numbered lists; elsewhere the things you name stay in a sentence.

Remove filler words is the same switch as Settings › Transcription › Remove filler words.

## Paste settings per kind of app

Under Paste, each kind of app can paste its own way. Inherit uses what's set in Settings › Output, and the option shows that value.

- **Paste with**: Ctrl+V, Ctrl+Shift+V, Shift+Insert, Type it, or Don't paste. Terminals and some editors want Shift+Insert.
- **After pasting**: Nothing, Enter or Ctrl+Enter, pressed once the text is in. Enter sends a chat message.
- **Trailing space**: On or Off. It keeps the next dictation from running into this one.

[Dictation](dictation.md#pasting) explains the Settings › Output versions of these.

## Speech model per kind of app

Speech model › Transcribe with picks the model for dictations into this kind of app. Inherit uses the model chosen on the Models page. You can pick any model you've downloaded; download another on the Models page to make it a choice. Dictating in these apps switches to that model, which takes a moment after dictating elsewhere.

If you delete the model, the setting falls back to Inherit. See [Dictation](dictation.md#speech-models-on-the-pc) for the models.

## Turning Style off

The switch at the top of the Style page turns Style per kind of app off. Dictations then get cleanup and your Settings › Output paste settings, the same in every app, with no tone and the Models page's speech model. Cleanup keeps working either way.

When a phone is paired, the tone for each kind of app syncs with [Legato for Android](android.md). Paste settings stay on this PC.

## See also

- [Dictionary and snippets](dictionary.md): snippets are pasted exactly as saved, untouched by cleanup and tone.
- [Transforms, Ask and your AI provider](transforms.md): a transform can run after every dictation, and skip code and AI prompts.
- [Privacy and your data](privacy.md): Remember which app I dictated into, and what Legato keeps.
