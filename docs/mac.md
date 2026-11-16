# Legato on a Mac

Legato is being brought to macOS, for Macs with Apple silicon. This page says how far that has come.

It is a preview. There is no Mac download on the releases page yet, and so far every build and check has run on a build machine, not in someone's hands. The app is built from the [`mac` branch](https://github.com/Abhash-Chakraborty/legato/tree/mac); each build there keeps a `.dmg` for 30 days.

## What the build checks

Every build of the branch is tried on a Mac build machine before it counts:

- The app builds for Apple silicon and is packed as `Legato.app` and a `.dmg`.
- It lists the Mac's GPU (Metal) and CPU as places to run a speech model.
- It transcribes a sentence spoken by macOS's own voice and gets the words right.
- It starts, stays up and draws its window.

A build machine has no microphone and nobody to answer macOS's permission questions. So the microphone, the shortcut and the paste into another app have not been tried there. They are built and waiting for a person with a Mac.

## What is built

- **Dictation.** Hold or press Option+Space, speak, and the text is pasted into the app in front with Command+V. The model runs on the Mac's GPU.
- **Setup.** The first step asks for the microphone and for Accessibility, and waits until both are on.
- **The menu bar icon.** It changes while Legato listens and while it writes, and its menu picks the model, copies the last dictation and quits. Closing the window leaves Legato in the menu bar.
- **Sounds.** The start and stop sounds are on by default on a Mac, since the Legato bar isn't there yet to show that it's listening.
- **The window.** It keeps the Mac's own close, minimise and zoom buttons, and shortcuts are shown with the Mac's key names.
- **Your words.** The Dictionary, snippets, cleanup and Style's tone work as on Windows, and so do Home and Insights.
- **Keys.** An AI provider's API key is kept in your login keychain.
- **Starting at login.** Settings › General › Launch at login adds a launch agent for your account.

## Shortcuts

Windows' defaults belong to macOS and its apps on a Mac, so the Mac has its own:

| Does                        | Windows          | Mac                |
| --------------------------- | ---------------- | ------------------ |
| Dictate                     | Ctrl+Space       | Option+Space       |
| Dictate, then transform     | Ctrl+Shift+Space | Option+Shift+Space |
| Add selection to Dictionary | Win+Alt+W        | Control+Option+W   |
| Open the Scratchpad         | Win+Alt+S        | Control+Option+S   |
| Start or stop notes         | Win+Alt+M        | Control+Option+M   |
| Paste last dictation        | Alt+Shift+Z      | Option+Shift+Z     |

Every other Win+Alt shortcut becomes Control+Option with the same letter or number. Change any of them in Settings › Shortcuts.

## Permissions

macOS asks before an app may do two of the things Legato needs:

- **Microphone**, to hear you. macOS asks the first time Legato listens.
- **Accessibility**, to hear its shortcut while another app is in front and to paste for you. Setup opens System Settings › Privacy & Security › Accessibility, where you switch Legato on.

The preview isn't signed with an Apple developer certificate. Two things follow. macOS won't open it from a double-click the first time: right-click the app and choose Open, or allow it under System Settings › Privacy & Security. And macOS treats each new build as a new app, so it asks for both permissions again after an update, and may ask before giving Legato a key it saved in the keychain.

## Not on the Mac yet

- **The Legato bar, the island and the bubble.** Nothing floats on screen while you dictate. The menu bar icon and the sounds say what Legato is doing. What the bar shows on Windows has nowhere to show: live text, the transform picker, a transform's changes with Keep and Undo, and Correct last dictation.
- **Knowing the app in front.** Style can't yet tell an email from a chat on a Mac, so one tone and one set of paste settings apply everywhere.
- **Meetings.** Notes records your microphone, but not the other side of a call, and Legato doesn't notice calls starting.
- **Reading the screen.** Use words on screen, screenshots in notes and Learn from my edits need parts that are Windows-only so far.
- **Phone sync.** Pairing a phone has not been brought over.
- **Pausing or lowering other sound.** Mute while dictating works; pausing players and lowering their volume don't.
- **Updates.** The Mac app doesn't update itself yet.
- **Intel Macs.** The build is for Apple silicon only.

Settings leaves out the rows for what isn't there.

## For people building it

`.github/workflows/macos.yml` builds the branch and runs the checks above. `scripts/mac_icon.py` writes the Mac icon from the logo. The Mac's bundle settings are in `src-tauri/tauri.macos.conf.json`, next to `Info.plist` and `Entitlements.plist`.
