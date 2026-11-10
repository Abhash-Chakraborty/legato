# Privacy and your data

Legato turns speech into text on your PC. This page covers what stays there, what can leave and when, where your data is kept, and how to delete it.

## What stays on your PC

- **Your voice.** Recording, voice detection and the speech model all run on this PC. Audio is never sent to an AI provider or any other service.
- **What you said.** Dictations, notes, transcripts, your Dictionary, snippets and the Scratchpad live in files on this PC.
- **Speakers.** The speaker model that tells voices apart in Notes runs on this PC. When you name a speaker, the 8-second clip Legato keeps of their voice is never synced or sent anywhere.
- **Calls.** To notice a call, Legato asks Windows which app is using the microphone; it doesn't listen to the call's audio for this. For Meet, Teams or Zoom in a browser it reads the window title, never the page.
- **Learn from my edits.** Ten seconds after a paste, Legato reads the text field once, on this PC, to pick up words you corrected. It's a switch on the [Dictionary](dictionary.md) page.

## No telemetry

Legato has no analytics, no telemetry and no crash reporting, and it has no account to sign in to. Its logs stay on this PC until you attach them to a bug report yourself.

## What can leave your PC, and when

| Goes to                         | When                                                                             | What                                                                                     |
| ------------------------------- | -------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| Hugging Face (`huggingface.co`) | You download a speech model, or the speaker model for Notes                      | A request for the model file                                                             |
| `blob.handy.computer`           | A speech model download from Hugging Face fails                                  | The same request, to the mirror Legato falls back on                                     |
| GitHub                          | At start, while Check for updates automatically is on                            | A request for the latest release's details. An update downloads only when you install it |
| Your AI provider                | Only after you add one in Settings › AI, and only for the uses you turn on there | Text, never audio                                                                        |
| Your calendars                  | Every ten minutes, for each calendar you added                                   | A request for its secret iCal address                                                    |
| Your Inyon hub                  | While paired                                                                     | Your answers to agents, what you say to the companion, notes you send to a session       |
| Your phone or tablet            | While paired and within reach                                                    | Synced data, straight to the device (see [Phone sync](#phone-sync))                      |

### Your AI provider

Legato works without one. If you add a provider in Settings › AI, this is what it gets, for each use you turn on under Use AI for:

- **Correction pass with your Dictionary:** each dictation's text, with your Dictionary words.
- **Transforms:** the text being transformed and the transform's prompt.
- **Notes summaries and Ask:** a recording's transcript, for its summary, decisions and action items, and your questions about it.

Ollama runs models on this PC, so with Ollama nothing leaves it. Settings › AI says which kind you have: "Text for the uses you turned on goes to …" for a cloud provider. API keys are sent only to the provider they belong to. More in [Transforms, Ask and your AI provider](transforms.md).

### AI apps

With Let AI apps read Legato on, AI apps on this PC can read your dictations, notes, Dictionary, Scratchpad and meetings through `legato --mcp`. Legato hands them the data on this PC, but the AI app may send it on to its own model. It's off until you turn it on. See [Connections](connections.md#ai-apps).

## Phone sync

A paired phone connects straight to the PC: over Wi-Fi when they share a network, over Bluetooth when they don't. There's no account and no server in between. The link is TLS with a certificate on both sides, and after pairing each side accepts only the other's certificate. The PC listens for phones only while pairing is open or a phone is paired.

Your Dictionary, snippets, history, notes, Scratchpad, transforms and style tones sync. You can switch each kind off in Settings › Devices › What syncs. Audio recordings, screenshots in notes, speech models, the microphone, shortcuts, the paste method and Legato bar settings never sync.

API keys stay on the PC unless you turn on Share AI keys with your phone. Then they go only over the encrypted link, and the phone keeps them encrypted. See [Legato for Android](android.md) and the [phone sync protocol](phone-sync.md#what-syncs).

## Where your data lives

| Where                                                     | What                                                                                                                                                |
| --------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| `%APPDATA%\io.github.abhash-chakraborty.legato\`          | `settings_store.json` (settings), `history.db` (dictations, notes, Dictionary, snippets, Scratchpad, transforms), `calendar.json` (cached meetings) |
| …`\recordings\`                                           | Audio of dictations and notes (WAV)                                                                                                                 |
| …`\voices\`                                               | Saved voice clips of people you named                                                                                                               |
| …`\models\`                                               | The speaker model, and speech models in older formats or fetched from the mirror                                                                    |
| `%LOCALAPPDATA%\io.github.abhash-chakraborty.legato\logs` | Log files                                                                                                                                           |
| `%USERPROFILE%\.cache\huggingface\hub`                    | Speech models from Hugging Face, in the cache other tools share (`HF_HOME` moves it)                                                                |
| Windows Credential Manager                                | Secrets (see below)                                                                                                                                 |
| A folder you pick                                         | Word copies of notes, if you set up [Documents](connections.md#documents)                                                                           |

Settings › About › App data and Logs open these folders.

### Secrets

API keys, calendar addresses, the Inyon hub's token and this PC's phone sync identity are kept in Windows Credential Manager as generic credentials named `Legato/…`, never in the settings file. A copied settings file or a log can't carry them.

## How long Legato keeps things

- **Dictations:** Settings › Privacy › History. Keep at most sets how many (500 to start with), and Keep recordings sets how long dictations and their recordings stay (After 2 weeks to start with). Never keeps them until you delete them.
- **Note audio:** Settings › Context & calls › Recordings › Keep note audio. Older audio is deleted; the transcript, summary and tags stay, but you can't play those notes back or name their speakers. New installs start at 3 months. Settings from before this option keep audio forever.
- **Notes and Scratchpad notes** stay until you delete them.
- **App names:** Settings › Privacy › Apps › Remember which app I dictated into. Off, Legato keeps no app names or window titles in history, the Dictionary, transform runs or the Scratchpad. Style still uses the app in front while you dictate, and forgets it once the text is pasted.

## Keep Legato out of screen shares

Settings › Legato bar › Hide from screenshots and screen sharing is on to start with. The bar, the island, the bubble and the Notes side panel stay on your screen but don't show in screenshots, recordings or a screen you share in a call. Turn it off if you want them in a recording.

The Scratchpad window has its own switch, Hide from screen sharing, at the top of the window and on the Scratchpad page. It's off to start with.

## Delete things

| To delete                 | Do this                                                                                                            |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| A dictation               | Delete it on Home. Its recording goes with it                                                                      |
| A note or Scratchpad note | Delete it. It waits 30 days in Recently deleted (a note keeps its recording there); Delete now removes it for good |
| A saved voice             | Settings › Context & calls › Speakers in notes › Saved voices › Forget                                             |
| An API key                | Settings › AI, then remove the key                                                                                 |
| A calendar                | Remove it in Settings › Connections › Meetings                                                                     |
| A speech model            | Delete it on the Models page                                                                                       |
| The speaker model         | Settings › Context & calls › Speakers in notes › Speaker model › Delete                                            |
| Everything                | Uninstall Legato and tick the box to delete its app data (see below)                                               |

Uninstalling Legato installed with the setup file asks whether to delete the app data. With that ticked, both data folders and every `Legato/…` entry in Credential Manager go. Left unticked, they stay for a reinstall. Speech models stay in the Hugging Face cache either way, since other tools may use them; delete them on the Models page first if you want them gone. Word copies in your notes folder stay too.
