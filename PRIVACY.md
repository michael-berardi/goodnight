# Privacy

Good Night warms and dims your screen locally. It has no account, no sign-in, no usage
reporting, no ads, no crash reporting and no install ID or machine fingerprint. Its only network
feature is the anonymous daily update check described in [docs/updates.md](docs/updates.md): one
HTTPS GET for the public signed release feed, which sends no settings, warmth schedule, prompts,
file contents, usage data or unique identifier. GitHub sees your IP address and ordinary
transport headers, as it would for any web request.

This page lists the files and folders Good Night writes, what is in them, how long they live and
what removes them. Nothing Good Night writes itself grows without a bound: the two short-lived
Windows files are deleted automatically, and the settings are yours or the uninstaller's to
remove. Sparkle and WebView2 keep their own caches, described below.

## What is written, and for how long

| Where | What it holds | Lifetime | What removes it |
|---|---|---|---|
| macOS: `~/Library/Preferences/com.libertydesignstudio.goodnight.plist` | Warmth, brightness, day/night levels, Follow the Sun, menu-bar and Dock visibility, and the update preference. Sparkle records its own last-check time here. | Kept until you delete it. No remote copy exists. | `defaults delete com.libertydesignstudio.goodnight`, or the profile reset below. |
| Windows: `%APPDATA%\com.libertydesignstudio.goodnight\settings.json` | The same settings, plus a `settingsVersion` field used by the 0.2/0.3 migrations. Private to your user account. | Kept until you delete it. | The uninstaller, or deleting the file. |
| Windows: `%LOCALAPPDATA%\com.libertydesignstudio.goodnight\update-check.json` | `lastCheckedAt` (a Unix timestamp) and, if the last check failed, one short cause string. Never a request body, prompt or file. | Rewritten at most once a day and swept at the next launch once it is **30 days** old, so a months-old record cannot linger. | The automatic sweep, the uninstaller, or deleting the file. |
| Windows: `settings.json.tmp` and `update-check.json.tmp` beside the two files above | The half-written next version of one of those files, during an atomic write. | Normally microseconds: each write is renamed into place immediately. A crash can abandon one; it is swept at the next launch once it is **1 hour** old. | The automatic sweep, which runs before the app reads anything at startup. |
| Logs | Good Night writes **no log files**. Diagnostics (a rejected display change, an engine fault, an uninstallable update) go to the macOS unified log or to the Windows console of the terminal that launched the app, and the console output vanishes with the terminal. | macOS unified log: Apple's own retention. Windows: nothing on disk. | macOS log retention; nothing to delete on Windows. |
| Windows installer: `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\ICM\GdiIcmGammaRange = 256` | One machine-wide DWORD that lets the display driver accept the full gamma range, the same setting f.lux needs. No personal data. | Until the app is uninstalled. | The uninstaller deletes it, but only while it still holds the value the installer wrote. |
| Windows WebView2: `%LOCALAPPDATA%\com.libertydesignstudio.goodnight` and WebView2's subfolders there | Browser data the WebView2 runtime creates for the interface: cookies, cached resources, permissions and similar browsing data. Good Night never reads it; it holds no settings or update history. | Managed by WebView2, not by Good Night's startup sweep. | A true uninstall of this profile's app-data folder, or deleting the folder yourself after quitting. |
| Windows install: `C:\Program Files\Good Night` and the `Uninstall` registry entry | The app, its uninstaller and bundled UI assets. WebView2 is a system runtime on Windows 10 and 11, with the per-user data listed above. | Until the app is uninstalled. | The uninstaller. |
| macOS install: `Good Night.app` in `/Applications` | The app and the shared Rust engine it bundles. | Until you remove it. | Dragging it to the Trash; turn off **Start at login** first so no login item registration is left behind. |

Two things Good Night never writes and never asks for: a per-install or per-machine identifier,
and anything about your prompts, files, clipboard, keystrokes, screenshots or location. Follow the
Sun uses your time zone's reference city, not your location, and no location permission is
requested.

The update download itself is a copy of the public release. On Windows the updater runs the
installer from its temporary folder. On macOS Sparkle saves downloaded archives under
`~/Library/Caches/com.libertydesignstudio.goodnight/org.sparkle-project.Sparkle/PersistentDownloads/`,
not in the system temporary folder; Sparkle prunes items there once they are **10 days** old, and
macOS may clear caches sooner. Both are ordinary public artifacts holding none of your data, and
neither is a file Good Night writes or a place your settings are stored.

The interface uses bundled assets and system fonts, with no third-party fonts, scripts or web
services. GitHub is the only third party contacted, and only for update checks and update
downloads described in [docs/updates.md](docs/updates.md); there are no user-configured cloud
services.

## Deleting everything

Quit Good Night first, so nothing is rewritten after you clean.

- **macOS:** `defaults delete com.libertydesignstudio.goodnight`, then drag `Good Night.app` to
  the Trash. If you enabled it, turn off **Start at login** in Settings before deleting the app.
  Sparkle's download cache under `~/Library/Caches/com.libertydesignstudio.goodnight/` is pruned
  by Sparkle and macOS; delete it yourself if you want it gone immediately.
- **Windows:** uninstall Good Night from **Settings → Apps**. A true uninstall removes the program
  files, the machine-wide gamma-range value and the uninstalling account's settings and cache
  folders, including its WebView2 data. An in-app update does not remove them. If another Windows
  account has also run Good Night, its own `%APPDATA%\com.libertydesignstudio.goodnight` and
  `%LOCALAPPDATA%\com.libertydesignstudio.goodnight` folders belong to that account; sign in as
  that user and delete those two folders.

After that, the app-written settings and profile folders are gone from that account. Sparkle's
macOS cache and other Windows accounts need the separate steps above.

## Bounds you can rely on

- At most **one automatic** update check per 24 hours, and only while update checks are on; Check Now is a request you start, and update downloads are requests you start unless you opted into automatic installation, in which case the updater downloads without waiting for you. See
  [docs/updates.md](docs/updates.md) for the full contract, the opt-out switch and the
  `GOODNIGHT_NO_UPDATE_CHECK=1` environment kill switch.
- Good Night's own profile files are bounded: Windows keeps one settings file, one update-check
  record, and a temporary file only while a write is in flight. WebView2 and Sparkle manage their
  own caches as described above.
- No file grows without a bound: the update-check record is a fixed-size timestamp plus one
  short cause string, and the settings file has a fixed set of numeric and boolean fields.
- The automatic sweep is the code in `windows/src-tauri/src/housekeeping.rs`; the lifetimes it
  applies are `TEMP_LIFETIME_SECS` (1 hour) and `CHECK_RECORD_LIFETIME_SECS` (30 days) in
  `core/src/retention.rs`. The tests in `core/tests/privacy.rs` fail if this page, those
  constants and the sweep drift apart.
