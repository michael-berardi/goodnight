<!-- update-contract: v1 -->

# Updates

Good Night uses the signed feed published with each GitHub release: `appcast.xml` on macOS and
`latest.json` on Windows. Only stable, newer releases are offered; drafts, prereleases and
unparseable versions are ignored.

## Check

- **On by default in interactive use.** macOS schedules Sparkle's check every 86,400 seconds;
  Windows makes its first background check one minute after launch and then at most once every
  24 hours. A private record in the Windows app cache (`update-check.json`, 0600 where Unix
  permissions apply) prevents a check on every restart.
- The check is one anonymous HTTPS GET to the repository's own public release feed. Good Night
  sends no install ID, machine ID, settings, warmth schedule, usage data or file contents. As
  with any web request, GitHub sees your IP address and ordinary transport headers.
- It runs off the startup path: Sparkle schedules it, and Windows uses a detached, non-blocking
  task with a 4-second timeout. A failed check is recorded with its cause and shown if you press
  **Check now**; it is never printed as routine noise or retried in a loop.
- No automatic check runs in CI, in a headless or print-only invocation (`--probe`, Windows
  `--hidden`), or when `GOODNIGHT_NO_UPDATE_CHECK=1` is set. The environment switch is temporary;
  it does not erase your saved preference.
- **Settings → Updates → Check for updates automatically** (on by default) is the persistent
  switch. On macOS use **Good Night › Check for Updates…**; on Windows use **Settings › Check now**.
  A manual check is explicit consent and bypasses the 24-hour interval while refreshing it.

## Prompt and install

- macOS shows Sparkle's non-modal dialog with the new version, your current version, short
  release notes, **Install Update**, **Remind Me Later** and **Skip This Version**.
- On Windows, **Check now** reports the newer version in Settings; **Install** downloads and
  verifies the signed release before installation. Installation waits for your explicit choice.
- **Automatically download and install updates in the future** is unchecked by default in the
  Sparkle dialog. Good Night 0.4.0 does not auto-install on Windows; check results only produce a
  prompt, so a background check cannot replace the version you are using.

## Privacy and retention

The update check is the app's only network feature. The local cache holds only the last check
time and, if a check failed, its cause. Delete it with the app cache folder on Windows; on macOS
Sparkle's schedule is part of the app's normal preferences and can be cleared with
`defaults delete com.libertydesignstudio.goodnight`. See the README's **Privacy** section for the
full list of what is stored and how to remove it.
