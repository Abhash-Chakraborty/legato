# Phone sync protocol

How Legato on Windows and Legato for Android keep each other up to date. There's no server
in between: the phone connects straight to the PC over Wi-Fi when they're on the same
network (or Netbird, for anyone who runs it) and over Bluetooth when they aren't. Both ends
check each other's certificate, and each keeps its own full copy.

This page is the contract the Android app implements. The PC side lives in
`src-tauri/src/sync/`. `phone/core` (`legato-phone`) is the phone side of it, shared by the Android app and the
`stand-in-phone` CLI, a command-line phone that speaks the
protocol, for testing the PC before the Android app exists.

## Identity

Each device has:

- a **device ID**, a random UUID made once;
- a **name** the person can change ("Pixel 8", "Abhash's desk PC");
- a **certificate**: self-signed X.509 with an ECDSA P-256 key, made once and kept.

A device is known by its **fingerprint**, the SHA-256 of its certificate's DER bytes, written
as unpadded base64url (43 characters). Names are labels only. Trust comes from the fingerprint.

On the phone, the private key is sealed with AES-256-GCM by a key held in the Android Keystore
(in StrongBox when the phone has it) and kept as `identity.sealed`; a plain `identity.json` from
an older version is moved into it on first start. A copy of the app's files can't pose as the
phone. If the Keystore key is gone (the phone was reset), the phone makes a new identity and has
to pair again. Neither file is backed up or carried to a new phone.

## Pairing

On the PC, Settings › Devices › Pair a phone shows a QR code holding a pairing link:

```
legato://pair?v=1&id=<pc device id>&name=<pc name>&fp=<pc fingerprint>
             &addr=192.168.1.20:47613,100.64.12.40:47613,bt:AA:BB:CC:DD:EE:FF&secret=<secret>
```

- `addr` lists every address the phone may try: home network first, then Netbird (any
  100.64.0.0/10 address), then the PC's Bluetooth radio as `bt:` and its address, when
  Bluetooth is on. Older phones skip a `bt:` address because it doesn't connect. Only private,
  link-local and 100.64.0.0/10 addresses go in; the phone refuses a link that names any other
  address, since such a link came from someone else's computer.
- `secret` is 16 random bytes in unpadded base64url. It works once, for 10 minutes. Wrong tries don't
  cancel it, so no one else on the network can end a pairing by guessing.
- Values are percent-encoded.

The phone tries the network addresses all at once and takes the first that answers; when
none does within 4 seconds, it tries Bluetooth. A phone linked over Bluetooth looks for the PC
on the network every 30 seconds and moves there once it answers and no recording is waiting
on the PC, since Bluetooth is slow for recordings. Either way it checks that the PC's
certificate matches `fp`. Then it sends `hello` with the secret. If the secret is right, the PC stores the
phone's device ID and fingerprint and answers `welcome`. From then on, both sides only accept
each other's stored fingerprint. Until the PC has heard the `hello`, the phone keeps the
secret and sends it on every retry, so pairing while the PC is briefly out of reach finishes
once it answers.

The phone never pairs straight from a link. Scanned, pasted or opened from another app, the
link is shown first: the PC's name, its addresses and, when the phone is paired with another
PC, which one it replaces. It pairs when you tap Pair. The app's `legato://pair` filter isn't
BROWSABLE, so a web page can't open it.

The PC also offers the link as text ("Copy link") so the stand-in phone can use it.

A short typed code isn't offered yet. A six-digit code can be brute-forced offline by anyone
who sits in the middle of the connection, so it needs a PAKE such as SPAKE2 on both platforms
first.

## Connection

- The phone opens TCP to the PC, on port 47613 by default. If that port is taken, the PC uses
  the next free port up to 47622, and the pairing link carries the actual one.
- TLS 1.3 only, with certificates on both sides. Neither side checks a chain: the phone pins
  the PC's fingerprint, and the PC accepts any client certificate during the handshake, then
  checks its fingerprint against the paired phones or the pairing secret.
- The PC listens only while pairing is open or a phone is paired. The first time, Windows asks
  whether to allow Legato on private networks.
- The PC drops a TCP connection before TLS unless it comes from this PC, the LAN (10/8,
  172.16/12, 192.168/16, fc00::/7), a link-local address, or a NetBird or Tailscale address
  (100.64/10). At most 8 connections, over TCP and Bluetooth together, may be in their
  handshake at once; more are dropped until one is admitted or gives up.

### Bluetooth

When the phone can't reach any network address, it connects over Bluetooth Classic RFCOMM
instead. The PC registers an SDP record for service UUID
`d33e3e4c-9cb7-4638-b68d-813565eee1a7` while it listens, and the phone connects to that UUID
on the `bt:` address from the pairing link. RFCOMM carries a byte stream, so everything below
(TLS, frames, messages) is exactly as over TCP. Bluetooth's own pairing isn't needed for
security: TLS with pinned certificates covers that. The phone tries without Bluetooth pairing
first. If the PC won't accept that, Android asks to pair the two once.

On Android, the app opens the RFCOMM socket and hands the core a port on 127.0.0.1 that
carries it, so the core's connection code stays the same for both wires. Any app on the phone
can reach that port, so the core makes a fresh random token for each link, and the bridge
carries only the first connection that sends it before anything else, within 10 seconds.
Another app that connects first is dropped and can't take the link.

### Frames

Each message is a 4-byte big-endian length followed by that many bytes of UTF-8 JSON. Every
message is an object with a `type`. Frames over 16 MiB are an error and close the connection.
Before a phone is admitted, its first frame (the `hello`) may be at most 8 KiB.
A receiver ignores types it doesn't know, except during the handshake.

## Messages

### Handshake

The phone speaks first:

```json
{
  "type": "hello",
  "protocol": 1,
  "device_id": "…",
  "name": "Pixel 8",
  "platform": "android",
  "app_version": "0.9.8",
  "secret": "…"
}
```

`secret` is only sent when pairing. The PC answers with one of:

```json
{"type":"welcome","protocol":1,"device_id":"…","name":"Desk PC","platform":"windows",
 "app_version":"0.9.8","kinds":["dictionary","snippets","history"],"clipboard":true,
 "addrs":["192.168.1.20:47613","bt:AA:BB:CC:DD:EE:FF"]}
{"type":"refused","reason":"unknown_device"}
```

Reasons: `unknown_device` (not paired, and no secret), `bad_secret`, `expired` (no pairing
open), `protocol` (a version the PC doesn't speak). The PC closes after `refused`. Only
`refused` makes the phone forget the PC; a closed port, a timeout or a failed handshake
means it's out of reach, and the phone tries again.

`addrs`, when present, lists where the PC can be reached now, in the pairing link's format.
The phone saves them in place of the ones from pairing, so a new Wi-Fi address, or
Bluetooth turned on after pairing, works from the next connection.

`kinds` lists what the PC syncs right now. `clipboard` says whether the shared clipboard is
on. Either can change during a connection. The PC then sends a new `welcome`, and the phone
updates without reconnecting.

### Sync

After `welcome`, both sides use the same messages:

```json
{"type":"pull","after":120,"kinds":["dictionary","snippets"],"limit":500}
{"type":"changes","items":[…],"upto":161,"more":false}
{"type":"nudge"}
```

- `pull` asks for changes the receiver made after its own sequence number `after`. The answer
  is `changes`. Send `upto` back as `after` next time. If `more` is true, pull again right
  away.
- A `pull` also tells the receiver that the sender holds everything up to `after`. The
  receiver uses that to spot edits made on both sides.
- Both sides pull as soon as the handshake is done, and again after a `nudge`, which means "I
  have new changes".

Each item in `changes`:

```json
{
  "kind": "dictionary",
  "key": "datev",
  "body": {
    "text": "Datev",
    "case_sensitive": false,
    "pinned": true,
    "heard": ["data v", "date of"]
  },
  "changed_at": 1792830000000,
  "origin": "<device id that made this version>"
}
```

`body` is `null` when the item was deleted (a tombstone). Tombstones are kept, so a device
that was away for a month still learns about deletes.

### Transcribing on the PC

A `welcome` with `"transcribe": true` means the PC will turn the phone's recordings into text
with its own speech model, which is bigger and quicker than the phone's. The phone sends:

```json
{ "type": "transcribe", "id": "…", "pcm": "<base64>", "app": "personal" }
```

`pcm` is the recording as 16 kHz mono, 16-bit little-endian samples, in base64. `app` is the
kind of app the text is for, as Style names it (`personal`, `work`, `email`, `documents`, `ai`,
`code`, `other`); the phone sorts the app in front by its package, and leaves `app` out when it
doesn't know. The PC runs the recording through its model and the same Dictionary, snippets,
cleanup and Style as a dictation on the PC in that kind of app, and answers with the same `id`.
`"app": "memo"` is a piece of a voice memo: the PC answers with the transcript as its own Notes
keep it (the Dictionary, fillers removed), without cleanup, transforms or Style. A PC older than
that reads it as an unknown app.

```json
{"type":"transcript","id":"…","text":"See you at noon."}
{"type":"transcript","id":"…","error":"The speech model didn't load"}
```

When the phone starts listening it sends a `transcribe` with an empty `pcm`. The PC loads its
model then, if it had put it away, so the recording doesn't wait for it, and answers with an
empty `text` that the phone ignores.

Either side keeps syncing while a recording is being written up. A phone that gets an error, or
no answer, uses its own model if it has one. The phone's model takes a long recording in pieces
of up to 30 seconds, each cut at its quietest moment, since small models give up past that. The recording isn't kept on the PC, and the
phone records the dictation in its history, which syncs as usual.

### Transforms

A `welcome` with `"transforms": true` means the PC runs its transforms for the phone. The
transforms themselves sync as the `transforms` kind, so the phone lists them away from the PC too.
While connected the phone sends:

```json
{
  "type": "run_transform",
  "id": "…",
  "transform_id": "polish",
  "text": "…",
  "app": "email"
}
```

and the PC runs it with its own AI provider and key, exactly as on the PC, answering:

```json
{"type":"transformed","id":"…","text":"…"}
{"type":"transformed","id":"…"}
{"type":"transformed","id":"…","error":"The AI provider turned the key down"}
```

No `text` and no `error` means the model had nothing to change.

To run transforms away from the PC, the phone needs the provider. Right after `welcome`, and
again when the setting changes, the PC sends:

```json
{
  "type": "ai",
  "providers": [
    {
      "id": "openai",
      "label": "OpenAI",
      "base_url": "https://api.openai.com/v1",
      "api_format": "openai",
      "model": "gpt-5-nano",
      "key": "…"
    }
  ],
  "own_models": [
    {
      "transform_id": "polish",
      "provider_id": "anthropic",
      "model": "claude-haiku-4-5"
    }
  ],
  "cleanup": {
    "provider_id": "openai",
    "model": "gpt-5-nano",
    "min_words": 3,
    "dictionary": true
  }
}
```

- The first provider is the one Settings › AI picks for transforms; `own_models` lists transforms
  that use a model of their own.
- `cleanup` is there while the PC runs Cleanup at Medium on dictations with a provider the phone
  can reach. The phone then cleans up what its own model writes up the same way: the PC's prompt,
  the Dictionary along when `dictionary` is set, nothing shorter than `min_words`, and the PC's
  checks on the answer. A slow (over 5 seconds) or failed answer leaves the rule-based text.
- Providers on this PC only (a local server) are left out.
- `key` is there only while Settings › Devices › Share AI keys with your phone is on (the
  default). Turning it off sends the list again without keys, and the phone drops the ones it
  kept. So does unpairing.
- `{"type":"ai_pull"}` asks for the list again.

The phone hands the keys to the app, which seals them with the Android Keystore (AES-256-GCM)
and never shows them again; only the rest of the list is written to disk. Away from the PC, the
phone builds the same prompt as the PC and calls the provider itself, over HTTPS only and without
following redirects, so a key can't be sent anywhere but its provider.

### Keeping the connection

- `{"type":"ping"}` every 30 seconds, answered with `{"type":"pong"}`. Close after 90
  seconds of silence.
- Before a recording or a transform goes to the PC, the phone sends a `ping` and waits up to 4
  seconds for anything to come back. Android freezes an app in the background, so a link can look
  open on the phone long after the PC gave up on it. A link that doesn't answer is closed and
  dialled again at once, and the recording waits up to 8 seconds for it before the phone writes
  it up with its own model.
- `{"type":"bye","reason":"unpaired"}` means the other side forgot you. Forget it too and close.
- When the PC can't be reached the phone waits 2 seconds before trying again, doubling each
  time up to 5 minutes, give or take a fifth so phones on one network don't all try at once.
  Opening the app or joining a new network tries again at once.

## Merging

Every item is replaced as a whole, and the newest version wins:

1. Compare `changed_at` (milliseconds since 1970, from the device that made the change).
2. On a tie, the larger `origin` string wins, so both sides pick the same one.

When an item changed on both devices since they last spoke, that's a merge. Both sides log it
for "See merges", with the item, the time and which device's version was kept. A Scratchpad
note that loses keeps its text as a version, so nothing typed is lost.

A device never sends an item back to the device it came from.

Two bodies are the same item when their JSON matches with keys sorted and `null` fields left
out, so a field added later (sent as `null` when unset) changes nothing for items without it.

## What syncs

Each kind can be switched off on the PC. Keys are stable across devices.

| Kind         | Key                                   | Body                                                                                                                                  |
| ------------ | ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| `dictionary` | the word, lowercased                  | `text`, `case_sensitive`, `pinned`, `heard` (sorted), `kind` (`null` clears it; a side that sends none keeps its own)                 |
| `snippets`   | the trigger, lowercased               | `trigger`, `text`, `alone`                                                                                                            |
| `history`    | `<device id>:<id on that device>`     | `text`, `heard`, `at` (seconds), `title`, `app`, `category`, `words`, `speech_ms`, `model`, `transform`, `cancelled` (only when true) |
| `notes`      | `<device id>:<id on that device>`     | `title`, `kind`, `started_at`, `duration_ms`, `app`, `my_notes`, `summary`, `decisions`, `tags`, `actions`, `lines`                   |
| `scratchpad` | `<device id>:<id on that device>`     | `title`, `auto_title`, `text`, `pinned`, `created_at`, `updated_at`                                                                   |
| `transforms` | the transform's ID                    | `name`, `description`, `prompt`, `enabled`, `built_in`, `output`, `show_changes`, `position`                                          |
| `style`      | the kind of app (`personal`, `work`…) | `tone`                                                                                                                                |

Rules per kind:

- **History** only spreads deletes the person made. When the PC trims old dictations to its
  history limit, the phone keeps them. Dictations from the phone show on Home with its name and
  don't count in Insights, which measures this PC.
- **History and notes** from another device carry `device`, that device's name, so a phone's
  dictation the PC passes on to a tablet shows the phone's name there too. A device sets it to
  the sender's name when a row arrives without one, and own rows go out without it. The ledger's
  hash leaves `device` out, so a device older than the field can't start a row bouncing.
- **Notes** sync once they're ready (not while recording or transcribing). The phone records
  voice memos too: `kind` is `memo`, each stretch of about 30 seconds is a line with `who` set to
  `you`, and the recording itself stays on the phone only until it's written up. A memo that
  arrives on the PC without a `summary` gets one from the PC's AI for Notes (with a title and tags
  while it still has the default ones), and the edit goes back like any other. Once the
  PC's speaker model has told voices apart, a line carries `speaker` (1, 2, …) and the note
  `speakers`, a list of `{speaker, name}` for the voices given a name; both are left out when
  unset, so older notes hash as before. Saved voice clips stay on the PC. A note in a folder
  carries `folder`, the folder's name, also left out when unset; a device makes the folder when
  a note arrives in one it doesn't have. Empty folders stay on the device they were made on.
- **Notes and the Scratchpad**: a deleted note waits 30 days in Recently deleted on the device
  it was deleted on. Restoring it there sends a newer version, and a device that had removed
  that note puts it back.
- **Transforms** leave out the AI provider and model, since API keys stay on each device.
- **Style** syncs the tone only. Paste method, the key after paste and the trailing space
  belong to the PC.

Never synced: audio recordings, speech models, the microphone, shortcuts, the paste method,
Legato bar settings and API keys.

## Shared clipboard

```json
{"type":"clip","id":"…","kind":"text","text":"…","age_ms":350,"from_app":"Chrome"}
{"type":"clip","id":"…","kind":"image","png":"<base64>","age_ms":350}
```

- Text, and PNG images up to the size the person picked (10 MB at most).
- `age_ms` is how long ago it was copied when it was sent. A clip older than 2 minutes on
  arrival goes to Recent clips and not onto the clipboard, so a phone coming back online
  doesn't overwrite what you copied since. Measuring age rather than sending a clock time
  means clocks don't need to agree.
- The PC never sends a clip that Windows marks private: one with
  `ExcludeClipboardContentFromMonitorProcessing`, or with `CanIncludeInClipboardHistory` or
  `CanUploadToCloudClipboard` set to 0. Password managers set these.
- Nor anything copied in an app on the "never share" list (KeePassXC, Bitwarden and 1Password
  to start with).
- The phone never sends a clip Android marks sensitive (`android.content.extra.IS_SENSITIVE`,
  which password managers set).
- Clips Legato writes itself, including ones that just arrived, are never sent back.
- Both sides keep the last 20 clips.

On the phone, Settings › Clipboard › Shared clipboard is Automatic or Manual:

- **Automatic** (the default) puts each text from the PC on the phone's clipboard as it arrives
  (subject to the 2-minute rule above). Android lets an app read the clipboard only while it's in
  front, so what you copy on the phone goes to the PC when Legato's window gets focus, unless it's
  the clip that just came from there.
- **Manual** moves nothing on its own. Clips from the PC wait in Recent clips; the phone sends
  with Send the clipboard now, or Send to PC in any app's share menu and on selected text.

Send to PC sends at once when it was picked from the system share sheet. Started any other way
(selected text, or another app starting it directly) it asks first, so no app can put text on the
PC's clipboard without you seeing it.
