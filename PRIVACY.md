# Privacy

Good Night warms and dims your screen locally. It has no account, no sign-in, no usage
reporting, no ads, no crash reporting and no install ID or machine fingerprint. Its automatic
network feature is the anonymous daily update check described in [docs/updates.md](docs/updates.md):
one HTTPS GET for the public signed release feed, which sends no settings, warmth schedule, prompts,
file contents, usage data or unique identifier. GitHub sees your IP address and ordinary
transport headers, as it would for any web request. Accepting an update downloads the public
release from the same host.

This page lists the files and folders Good Night writes, what is in them, how long they live and
what removes them. Nothing Good Night writes itself grows without a bound: the short-lived files
are swept at launch or trimmed while the app runs, and the settings are yours or the uninstaller's
to remove. Sparkle and WebView2 keep their own caches, described below.

## What is written, and for how long

| Where | What it holds | Lifetime | What removes it |
|---|---|---|---|
| macOS: `~/Library/Preferences/com.libertydesignstudio.goodnight.plist` | Warmth, brightness, day/night levels, Follow the Sun, menu-bar and Dock visibility, both update switches and `installUpdatesAutomatically`. Sparkle records its own last-check time here. | Kept until you delete it. No remote copy exists. | `defaults delete com.libertydesignstudio.goodnight`, or the profile reset below. |
| Windows: `%APPDATA%\com.libertydesignstudio.goodnight\settings.json` | The same settings, plus a `settingsVersion` field used by the 0.2/0.3 migrations. Private to your user account (0600 where Unix permissions apply). | Kept until you delete it. | The uninstaller, or deleting the file. |
| Windows: `%APPDATA%\com.libertydesignstudio.goodnight\updates.json` | The last check time, one short failure cause, the skipped version and the version last shown. Never a request body, prompt or file. Written 0600 where Unix permissions apply. | Rewritten at most once a day and swept at the next launch once it is **30 days** old. Turning checks off deletes it immediately. | The launch sweep, turning checks off, or the uninstaller. |
| Windows: `%LOCALAPPDATA%\com.libertydesignstudio.goodnight\update-check.json` | The same kind of check record, written by earlier privacy builds. The merged shell does not create it, but clears a leftover copy with the same **30-day** bound. | Rewritten at most once a day in those builds; swept at the next launch once 30 days old. | The launch sweep, or deleting the file. |
| Windows: `settings.json.tmp`, `updates.json.tmp` and `update-check.json.tmp` beside the files above | The half-written next version of one of those files, during an atomic write. | Normally microseconds: each write is renamed into place immediately. A crash can abandon one; it is swept at the next launch once it is **1 hour** old. | The launch sweep, which runs before the app reads anything at startup. |
| Windows: `%APPDATA%\com.libertydesignstudio.goodnight\updates.log` | Short background update events (checks, results, automatic installs), one line each. No settings, file contents or network payloads. | Kept to the newest whole lines within **256 KB**; older lines are trimmed at every write. Turning checks off or uninstalling deletes the file. | The size-bound writer, turning checks off, or the uninstaller. |
| macOS: unified log | Diagnostics from the app and Sparkle, if macOS chooses to record them. No account data. | Apple's own log retention. | macOS log retention. |
| Windows: terminal console | Diagnostics (a rejected display change, an engine fault) are printed only when launched from a terminal; there is **no Windows log file** of our own. | The console session. | Closing the terminal. |
| Windows installer: `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\ICM\GdiIcmGammaRange = 256` | One machine-wide DWORD that lets the display driver accept the full gamma range, the same setting f.lux needs. No personal data. | Until the app is uninstalled. | The uninstaller deletes it, but only while it still holds the value the installer wrote. |
| Windows WebView2: `%LOCALAPPDATA%\com.libertydesignstudio.goodnight` and WebView2's subfolders there | Browser data the WebView2 runtime creates for the interface: cookies, cached resources, permissions and similar browsing data. Good Night never reads it; it holds no settings or update history. | Managed by WebView2, not by Good Night's startup sweep. | A true uninstall of this profile's app-data folder, or deleting the folder yourself after quitting. An in-app update does not remove them. |
| Windows install: `C:\Program Files\Good Night` and the `Uninstall` registry entry | The app, its uninstaller and bundled UI assets. WebView2 is a system runtime on Windows 10 and 11, with the per-user data listed above. | Until the app is uninstalled. | The uninstaller. |
| macOS install: `Good Night.app` in `/Applications` | The app and the shared Rust engine it bundles. | Until you remove it. | Dragging it to the Trash; turn off **Start at login** in Settings first so no login item registration is left behind. |

Two things Good Night never writes and never asks for: a per-install or per-machine identifier,
and anything about your prompts, files, clipboard, keystrokes, screenshots or location. Follow the
Sun uses your time zone's reference city, not your location, and no location permission is
requested.

The update download itself is a copy of the public release. On Windows the updater saves it as
`%TEMP%\Good Night-<version>-updater-<random>\Good Night-<version>-installer.exe` and runs it from
there after Good Night exits. At the next launch `windows/src-tauri/src/updates.rs` (function
`remove_staged_installers`) deletes that installer and the staging folder if nothing else is in
it; it opens only folders with this app's own `Good Night-<version>-updater-<random>` name and
never recurses into a folder holding other files. On macOS Sparkle saves downloaded archives
under `~/Library/Caches/com.libertydesignstudio.goodnight/org.sparkle-project.Sparkle/PersistentDownloads/`,
not in the system temporary folder; Sparkle prunes items there once they are **10 days** old, and
macOS may clear caches sooner. Both are ordinary public artifacts holding none of your data, and
neither holds your settings.

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
  files, the machine-wide gamma-range value (only if it still holds 256), and the uninstalling
  account's settings and cache folders, including its WebView2 data and the update cache and log.
  An in-app update does not remove them. If another Windows account has also run Good Night, its
  own `%APPDATA%\com.libertydesignstudio.goodnight` and
  `%LOCALAPPDATA%\com.libertydesignstudio.goodnight` folders belong to that account; sign in as
  that user and delete those two folders.

After that, the app-written settings and profile folders are gone from that account. Sparkle's
macOS cache and other Windows accounts need the separate steps above.

## Bounds you can rely on

- At most **one automatic** update check per 24 hours, and only while update checks are on; Check
  Now is a request you start, and update downloads are requests you start unless you opted into
  automatic installation, in which case the updater downloads without waiting for you. See
  [docs/updates.md](docs/updates.md) for the full contract, the opt-out switch and the
  `GOODNIGHT_NO_UPDATE_CHECK=1` environment kill switch.
- Good Night's own profile files are bounded: Windows keeps one settings file, one update cache
  record, one bounded log, and a temporary file only while a write is in flight. WebView2 and
  Sparkle manage their own caches as described above.
- No file grows without a bound: the update cache is a fixed set of short fields, the log is
  capped at 256 KB, and the settings file has a fixed set of numeric and boolean fields.
- The lifetime policy is `core/src/retention.rs`; the startup file sweep is
  `windows/src-tauri/src/housekeeping.rs`. It applies `TEMP_LIFETIME_SECS` (1 hour) and
  `CHECK_RECORD_LIFETIME_SECS` (30 days). The updater's abandoned installer has a separate,
  single-purpose cleanup in `windows/src-tauri/src/updates.rs` at the next launch. The tests in
  `core/tests/privacy.rs` fail if this page, those constants and the sweep wiring drift apart.
