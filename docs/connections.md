# Connections

Settings › Connections links Legato to your calendars, a folder of Word copies of your notes, AI apps that read Legato, and an Inyon hub.

None of these need an account. Legato never signs in to Google or Microsoft: calendars come from secret iCal addresses, and Word copies reach the cloud through the Google Drive or OneDrive app you already run.

## Calendars

With a calendar added, Legato knows your meetings:

- A meeting note takes the name of the meeting on your calendar, and the meeting's attendees join the note.
- The desktop bubble shows your next meeting.
- With Offer to join meetings on, the bar offers Join and Join with notes a minute before a meeting starts.
- AI apps you allow can ask for your upcoming meetings (see [AI apps](#ai-apps)).

Coming up, under the calendar list, shows the next meetings Legato knows about.

### Add a calendar

1. Copy your calendar's secret iCal address.
   - **Google Calendar:** open Settings › your calendar › Integrate calendar, and copy Secret address in iCal format.
   - **Outlook:** publish the calendar and copy its ICS link. In Outlook on the web this is under Settings › Calendar › Shared calendars › Publish a calendar.
2. In Legato, open Settings › Connections › Meetings, paste it into Secret iCal address, and give it a name if you like.
3. Choose Add calendar. Legato fetches it once to check it, then says how many meetings it found this week.

Add as many calendars as you want, from different accounts. A `webcal://` address works too; Legato fetches it over https. Plain `http://` addresses aren't accepted.

Legato fetches each calendar every ten minutes, straight from this PC. Choose Refresh to fetch now. It keeps the meetings from a day back to two weeks ahead in `calendar.json` in its data folder, so they're there at the next start even without a network.

> **Note:** Anyone with a secret iCal address can read that calendar. Legato keeps the addresses in Windows Credential Manager, never in its settings file. Removing a calendar offers Undo for a moment, then the address is gone.

## Documents

Keep notes in a folder writes a Word copy (`.docx`) of every note into a folder you pick. Google Docs opens these copies with their formatting.

- If Google Drive for desktop or OneDrive is installed, Legato offers Use Google Drive or Use OneDrive. Either makes a `Legato notes` folder inside it.
- Choose folder… picks any other folder.

The copies reach the cloud through that app's own sync. Legato itself never signs in anywhere or goes online for them.

How the copies behave:

- A copy is rewritten shortly after its note changes, and renamed when the note's title changes.
- Deleting a note in Legato leaves its last copy in the folder.
- Legato never overwrites a file with the same name that it didn't write.
- If the folder isn't there (Google Drive isn't running, say), copies wait until it is.

Stop turns the copies off. Files already in the folder stay.

## AI apps

`legato --mcp` serves Legato's data over the Model Context Protocol, so an AI app on this PC (Claude, Cursor or any other MCP app) can read it. The AI app starts this process itself and talks to it over stdio. It reads Legato's data folder directly, so Legato doesn't need to be running.

### Turn it on

1. Open Settings › Connections › AI apps and turn on Let AI apps read Legato. The same switch is in Settings › Features under Other apps.
2. Under Add Legato to an AI app, choose Add to Claude or Add to Cursor if Legato found that app on this PC. Legato adds itself to the app's MCP servers and leaves the rest of its settings alone.
3. Restart the AI app.

For any other app, choose Copy setup and paste it into the app's MCP configuration. It looks like this, with the path to your own `legato.exe`:

```json
{
  "mcpServers": {
    "legato": {
      "command": "C:\\Users\\you\\AppData\\Local\\Legato\\legato.exe",
      "args": ["--mcp"]
    }
  }
}
```

The setup file installs Legato in `%LOCALAPPDATA%\Legato`; the `.msi` installs it in `C:\Program Files\Legato`. For Claude Desktop, this goes in `claude_desktop_config.json` in `%APPDATA%\Claude`. For Cursor, it goes in `%USERPROFILE%\.cursor\mcp.json`.

### What an AI app can do

| Tool                  | What it does                                                                                |
| --------------------- | ------------------------------------------------------------------------------------------- |
| `search_dictations`   | Searches your dictations, newest first, with the app each went into and when                |
| `list_notes`          | Lists your meeting and voice notes, with title, date, length and tags                       |
| `get_note`            | Reads one note in full: summary, decisions, action items, your own notes and the transcript |
| `list_dictionary`     | Reads your Dictionary, with the ways the speech model misheard each word                    |
| `upcoming_meetings`   | Lists meetings from the calendars you added                                                 |
| `list_scratchpad`     | Reads your Scratchpad notes, pinned first                                                   |
| `add_scratchpad_note` | Saves text as a new Scratchpad note                                                         |

Adding a Scratchpad note is the only change an AI app can make. Legato checks the switch on every request, so turning off Let AI apps read Legato stops answers at once, without restarting the AI app.

> **Note:** What an AI app reads goes to that app, and from there to whatever model it uses. Turn this on only for apps you'd trust with your notes.

## Inyon

[Inyon](https://github.com/Abhash-Chakraborty/inyon) lets you watch and answer the coding agents running in tmux on your machines. Paired with an Inyon hub, Legato shows an agent that's waiting for you on the bar (or the island), and you answer it there.

### Pair with a hub

1. In Inyon, open Settings › Paired devices.
2. Either copy the pairing link and paste it into Hub address or pairing link in Settings › Connections › Inyon, or type the hub's address and the six-character code from Pair a companion app. On the hub's computer, `inyon hub companion-code` prints a code too.
3. Choose Pair.

A code works once, for five minutes. Legato pairs over https, or over plain http only on this PC or a NetBird or Tailscale address, so nobody on the network can read the hub's token. The token is kept in Windows Credential Manager.

Remove unpairs Legato. Remove Legato in Inyon's Paired devices too.

### Agents on the bar

When an agent asks something, the bar shows the agent, its question and its options. While you dictate, or while the bar shows a call card, the agent waits its turn. Click an option to answer.

To answer by voice, choose Answer by voice on the card and say an option, like "yes" or "two", or say what to do instead. Press again to send.

Agents on the bar, under the pairing, turns this off but keeps the pairing.

### The companion

Inyon's companion agent looks after your other sessions. Hold Talk to the companion (Win+Alt+I unless you changed it in Settings › Shortcuts) and speak. The bar says "Companion is thinking…", and its reply shows on the bar's companion card. Choose Reply on the card to answer by voice.

The Scratchpad's Send to also lists the hub's agent sessions, so you can send a note to an agent. See [Home, Scratchpad and Insights](scratchpad.md).
