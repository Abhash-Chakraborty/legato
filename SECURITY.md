# Security

## Reporting a problem

Please don't open a public issue for a security problem. Report it privately through [GitHub's security advisories](https://github.com/Abhash-Chakraborty/legato/security/advisories/new) instead. You'll get a reply within a week, and a fix ships in the next release, with credit to you if you'd like it.

Only the latest release gets fixes.

## What Legato keeps and sends

- Audio is transcribed on your PC. Recordings, history, notes and settings stay in your Windows profile.
- API keys and calendar addresses are stored in Windows Credential Manager, not in files.
- Legato sends text over the internet only to the AI provider you add yourself, and to GitHub to check for updates (which you can turn off).
- Phone sync is a direct, mutually authenticated TLS connection between your devices, with no server in between ([docs/phone-sync.md](docs/phone-sync.md)).
- Releases are signed for the updater; see [Verify a download](docs/verify.md). The installers aren't code-signed yet.
