# Privacy

Good Night warms and dims your screen locally. It has no account, no sign-in, no usage
reporting, no ads, no crash reporting and no install ID or machine fingerprint. Its only network
feature is the anonymous daily update check described in [docs/updates.md](docs/updates.md): one
HTTPS GET for the public signed release feed, which sends no settings, warmth schedule, prompts,
file contents, usage data or unique identifier. GitHub sees your IP address and ordinary
transport headers, as it would for any web request.

This page lists every file Good Night writes, what is in it, how long it lives and what removes
it. Nothing here grows without a bound: the two short-lived files are deleted automatically, and
the two long-lived ones are your own settings, deleted by you or by the uninstaller.

## What is written, and for how long

| Where | What it holds | Lifetime | What removes it |
|---|---|---|---|
| macOS: `~/Library/Preferences/com.libertydesignstudio.goodnight.plist` | Warmth, brightness, day/night levels, Follow the Sun, menu-bar and Dock visibility, and the update preference. Sparkle records its own last-check time here. | Kept until you delete it. No remote copy exists. | `defaults delete com.libertydesignstudio.goodnight`, or the profile reset below. |
| Windows: `%APPDATA%\com.libertydesignstudio.goodnight\settings.json` | The same settings, plus a `settingsVersion` field used by the 0.2/0.3 migrations. Private to your user account. | Kept until you delete it. | The uninstaller, or deleting the file. |
| Windows: `%LOCALAPPDATA%\com.libertydesignstudio.goodnight\update-check.json` | `lastCheckedAt` (a Unix timestamp) and, if the last check failed, one short cause string. Never a request body, prompt or file. | Rewritten at most once a day and swept at the next launch once it is **30 days** old, so a months-old record cannot linger. | The automatic sweep, the uninstaller, or deleting the file. |
| Windows: `settings.json.tmp` and `update-check.json.tmp` beside the two files above | The half-written next version of one of those files, during an atomic write. | Normally microseconds: each write is renamed into place immediately. A crash can abandon one; it is swept at the next launch once it is **1 hour** old. | The automatic sweep, which runs before the app reads anything at startup. |
| Logs | Good Night writes **no log files**. Diagnostics (a rejected display change, an engine fault, an uninstallable update) go to the macOS unified log or to the Windows console of the terminal that launched the app, and the console output vanishes with the terminal. | macOS unified log: Apple's own retention. Windows: nothing on disk. | macOS log retention; nothing to delete on Windows. |
| Windows installer: `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\ICM\GdiIcmGammaRange = 256` | One machine-wide DWORD that lets the display driver accept the full gamma range, the same setting f.lux needs. No personal data. | Until the app is uninstalled. | The uninstaller deletes it, but only while it still holds the value the installer wrote. |
| Windows install: `C:\Program Files\Good Night` and the `Uninstall` registry entry | The app, its uninstaller and the WebView2 runtime files Tauri needs. | Until the app is uninstalled. | The uninstaller. |
| macOS install: `Good Night.app` in `/Applications` | The app and the shared Rust engine it bundles. | Until you remove it. | Dragging it to the Trash; turn off **Start at login** first so no login item registration is left behind. |

Two things Good Night never writes and never asks for: a per-install or per-machine identifier,
and anything about your prompts, files, clipboard, keystrokes, screenshots or location. Follow the
Sun uses your time zone's reference city, not your location, and no location permission is
requested.

The update download itself is a copy of the public release. On Windows the installer runs from
the updater's temporary folder, and on macOS Sparkle downloads to the system temporary folder;
both are ordinary public artifacts holding none of your data, and the operating system clears
those folders on its own schedule. Neither is a file Good Night writes, and neither is a place
your settings are stored.

The interface uses bundled assets and system fonts, with no third-party fonts, scripts or web
services. GitHub is the only third party contacted, and only for the update check described in
[docs/updates.md](docs/updates.md); there are no user-configured cloud services.

## Deleting everything

Quit Good Night first, so nothing is rewritten after you clean.

- **macOS:** `defaults delete com.libertydesignstudio.goodnight`, then drag `Good Night.app` to
  the Trash. If you enabled it, turn off **Start at login** in Settings before deleting the app.
- **Windows:** uninstall Good Night from **Settings → Apps**. The uninstaller removes the program
  files, the machine-wide gamma-range value and that profile's settings and cache folders. If
  another Windows account has also run Good Night, its own
  `%APPDATA%\com.libertydesignstudio.goodnight` and
  `%LOCALAPPDATA%\com.libertydesignstudio.goodnight` folders belong to that account and are not
  reachable from an elevated uninstaller run under a different account; sign in as that user and
  delete those two folders.

After that, nothing Good Night wrote remains on the machine.

## Bounds you can rely on

- At most **one** network request per 24 hours, and only while update checks are on. See
  [docs/updates.md](docs/updates.md) for the full contract, the opt-out switch and the
  `GOODNIGHT_NO_UPDATE_CHECK=1` environment kill switch.
- At most **three** files in your profile: one settings file, one update-check record, one
  temporary file that exists only while a write is in flight.
- No file grows without a bound: the update-check record is a fixed-size timestamp plus one
  short cause string, and the settings file has a fixed set of numeric and boolean fields.
- The automatic sweep is the code in `windows/src-tauri/src/housekeeping.rs`; the lifetimes it
  applies are `TEMP_LIFETIME_SECS` (1 hour) and `CHECK_RECORD_LIFETIME_SECS` (30 days) in
  `core/src/retention.rs`. The tests in `core/tests/privacy.rs` fail if this page, those
  constants and the sweep drift apart.
