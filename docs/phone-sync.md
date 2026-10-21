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

## Pairing

On the PC, Settings › Devices › Pair a phone shows a QR code holding a pairing link:

```
legato://pair?v=1&id=<pc device id>&name=<pc name>&fp=<pc fingerprint>
             &addr=192.168.1.20:47613,100.97.188.40:47613,bt:AA:BB:CC:DD:EE:FF&secret=<secret>
```

- `addr` lists every address the phone may try: home network first, then Netbird (any
  100.64.0.0/10 address), then the PC's Bluetooth radio as `bt:` and its address, when
  Bluetooth is on. Older phones skip a `bt:` address because it doesn't connect.
- `secret` is 16 random bytes in unpadded base64url. It works once, for 10 minutes. Five wrong
  tries cancel it.
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

### Bluetooth

When the phone can't reach any network address, it connects over Bluetooth Classic RFCOMM
instead. The PC registers an SDP record for service UUID
`d33e3e4c-9cb7-4638-b68d-813565eee1a7` while it listens, and the phone connects to that UUID
on the `bt:` address from the pairing link. RFCOMM carries a byte stream, so everything below
(TLS, frames, messages) is exactly as over TCP. Bluetooth's own pairing isn't needed for
security: TLS with pinned certificates covers that. The phone tries without Bluetooth pairing
first. If the PC won't accept that, Android asks to pair the two once.

On Android, the app opens the RFCOMM socket and hands the core a socket on 127.0.0.1 that
carries it, so the core's connection code stays the same for both wires.

### Frames

Each message is a 4-byte big-endian length followed by that many bytes of UTF-8 JSON. Every
message is an object with a `type`. Frames over 16 MiB are an error and close the connection.
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
{ "type": "transcribe", "id": "…", "pcm": "<base64>" }
```

`pcm` is the recording as 16 kHz mono, 16-bit little-endian samples, in base64. The PC runs it
through its model and the same Dictionary, snippets, cleanup and Style as a dictation on the
PC, and answers with the same `id`:

```json
{"type":"transcript","id":"…","text":"See you at noon."}
{"type":"transcript","id":"…","error":"The speech model didn't load"}
```

Either side keeps syncing while a recording is being written up. A phone that gets an error, or
no answer, uses its own model if it has one. The recording isn't kept on the PC, and the
phone records the dictation in its history, which syncs as usual.

### Keeping the connection

- `{"type":"ping"}` every 30 seconds, answered with `{"type":"pong"}`. Close after 90
  seconds of silence.
- `{"type":"bye","reason":"unpaired"}` means the other side forgot you. Forget it too and close.

## Merging

Every item is replaced as a whole, and the newest version wins:

1. Compare `changed_at` (milliseconds since 1970, from the device that made the change).
2. On a tie, the larger `origin` string wins, so both sides pick the same one.

When an item changed on both devices since they last spoke, that's a merge. Both sides log it
for "See merges", with the item, the time and which device's version was kept. A Scratchpad
note that loses keeps its text as a version, so nothing typed is lost.

A device never sends an item back to the device it came from.

## What syncs

Each kind can be switched off on the PC. Keys are stable across devices.

| Kind         | Key                                   | Body                                                                                                                |
| ------------ | ------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| `dictionary` | the word, lowercased                  | `text`, `case_sensitive`, `pinned`, `heard` (sorted)                                                                |
| `snippets`   | the trigger, lowercased               | `trigger`, `text`, `alone`                                                                                          |
| `history`    | `<device id>:<id on that device>`     | `text`, `heard`, `at` (seconds), `title`, `app`, `category`, `words`, `speech_ms`, `model`, `transform`             |
| `notes`      | `<device id>:<id on that device>`     | `title`, `kind`, `started_at`, `duration_ms`, `app`, `my_notes`, `summary`, `decisions`, `tags`, `actions`, `lines` |
| `scratchpad` | `<device id>:<id on that device>`     | `title`, `auto_title`, `text`, `pinned`, `created_at`, `updated_at`                                                 |
| `transforms` | the transform's ID                    | `name`, `description`, `prompt`, `enabled`, `built_in`, `output`, `show_changes`, `position`                        |
| `style`      | the kind of app (`personal`, `work`…) | `tone`                                                                                                              |

Rules per kind:

- **History** only spreads deletes the person made. When the PC trims old dictations to its
  history limit, the phone keeps them. Dictations from the phone show on Home with its name and
  don't count in Insights, which measures this PC.
- **Notes** sync once they're ready (not while recording or transcribing). The phone records
  voice memos too: `kind` is `memo`, each stretch of about 30 seconds is a line with `who` set to
  `you`, and the recording itself stays on the phone only until it's written up.
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
- Clips Legato writes itself, including ones that just arrived, are never sent back.
- Both sides keep the last 20 clips.

On the phone, Settings › Clipboard › Shared clipboard is Automatic or Manual:

- **Automatic** (the default) puts each text from the PC on the phone's clipboard as it arrives
  (subject to the 2-minute rule above). Android lets an app read the clipboard only while it's in
  front, so what you copy on the phone goes to the PC when Legato's window gets focus, unless it's
  the clip that just came from there.
- **Manual** moves nothing on its own. Clips from the PC wait in Recent clips; the phone sends
  with Send the clipboard now, or Send to PC in any app's share menu and on selected text.
