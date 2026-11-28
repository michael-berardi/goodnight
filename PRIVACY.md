# Privacy

Good Night changes display colours locally. It has no account, no analytics, no telemetry, no crash reporting,
no ads and no install ID or machine fingerprint. Nothing about your display, schedule or location leaves the
device. The only automatic network feature is the anonymous daily update check below, and you can turn it off.

## Update checks

A check is one anonymous HTTPS GET of the app's signed update feed on GitHub Releases: `appcast.xml`
on macOS, `latest.json` on Windows. The request carries the fixed User-Agent
`goodnight-update-check` and no identifier, app version, usage or machine data (on macOS also a
fixed `Accept-Language: en`; Sparkle's system profile stays off). No settings, warmth schedule or
file contents are sent. As with any web request, GitHub sees your IP address and the request headers.

- At most once a day, in the background, starting a minute after launch, never during startup.
  On Windows the request gives up after 4 seconds.
- A failed check is recorded locally with its cause (Settings shows "last check failed" on
  Windows) and waits for the next day; it is not retried in a loop and not sent anywhere.
- Off when **Check for updates automatically** is off in **Settings › Updates**, when
  `GOODNIGHT_NO_UPDATE_CHECK=1` is set, or when `CI` is set. **Check Now** still works.

Downloading an update is a second request, to the release asset the signed feed names. It happens
when you choose **Install Update**, or by itself only if you opted into installing automatically.
GitHub is the only third party Good Night contacts; there are no user-configured cloud services.
The interface uses bundled assets and system fonts: no third-party fonts, scripts or web requests.

## What is written, and for how long

Settings hold warmth, brightness, schedule and visibility choices and the two update switches, and
nothing else: no prompts, files, credentials or location. No file Good Night writes grows without
a bound.

| Where | What it holds | Lifetime | What removes it |
|---|---|---|---|
| macOS: `~/Library/Preferences/com.libertydesignstudio.goodnight.plist` | Warmth levels, brightness, Follow the Sun, menu-bar and Dock visibility. Sparkle keeps its two update switches, its last check time and any skipped version here too. | Until you delete it. | `defaults delete com.libertydesignstudio.goodnight`. |
| Windows: `%APPDATA%\com.libertydesignstudio.goodnight\settings.json` | The same settings, including both update switches. The folder carries the user-only AppData permissions; on Unix-like hosts the folder is created 0700 and the file 0600. | Until you delete it or uninstall. | The uninstaller, or deleting the folder. |
| Windows: `%APPDATA%\com.libertydesignstudio.goodnight\updates.json` | The last check time, the last failure cause, a skipped version and the version last shown. Written 0600 on Unix-like hosts. | Rewritten by each check and by **Skip This Version**. Once it has gone unwritten for **30 days** it is deleted at the next launch (a skipped version is then offered again). | Turning checks off (immediately), the launch sweep, or the uninstaller. |
| Windows: `%APPDATA%\com.libertydesignstudio.goodnight\updates.json.tmp` | The next `updates.json` while it is being written; it is renamed into place at once. | Only a crash leaves one behind; it is deleted at the next launch once it is **1 hour** old. | The launch sweep, turning checks off, or the uninstaller. |
| Windows: `%APPDATA%\com.libertydesignstudio.goodnight\updates.log` | One line per failed background check, per automatic install and per failed automatic install, each at most 500 characters. No settings, file contents or network payloads. | The newest whole lines within **256 KB**; older lines are dropped at every write. | Turning checks off, or the uninstaller. |
| Windows: `%LOCALAPPDATA%\com.libertydesignstudio.goodnight` | Browser data the WebView2 runtime keeps for the interface (cache, local storage and similar). Good Night never reads it; it holds no settings or update history. | Managed by WebView2. An in-app update does not remove them. | The uninstaller, or deleting the folder after quitting. |
| Windows: `%TEMP%\Good Night-<version>-updater-<random>\Good Night-<version>-installer.exe` | A downloaded update: a copy of the public, signed installer. | Until a minute after the next launch (see below). | The app's own sweep, or the uninstaller. |
| macOS: `~/Library/Caches/com.libertydesignstudio.goodnight/org.sparkle-project.Sparkle/PersistentDownloads/` and the `Installation/` folder beside it | Sparkle's downloads and installation staging: copies of the public, signed release. | Sparkle deletes items **10 days** old or older each time it downloads or installs another update; macOS may clear caches sooner. | Sparkle, macOS, or deleting the folder. |
| Windows login item: the `Good Night` value in `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` | Set only while **Start with Windows** is on: the app's path and `--hidden`. | Until the switch is turned off. | Turning **Start with Windows** off, or the uninstaller (for the account that runs it). |
| macOS login item | Registered only while **Start at login** is on. | Until the switch is turned off. | Turning **Start at login** off (do this before deleting the app). |
| Windows: `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\ICM\GdiIcmGammaRange = 256` | One machine-wide value that lets the display driver accept the full gamma range, the same setting f.lux needs. No personal data. | Until uninstall. | The uninstaller, but only while it still holds the 256 the installer wrote. |
| Diagnostics | macOS: the unified log, under Apple's retention. Windows: standard error, which goes nowhere unless you start the app with its output redirected. | Not stored by Good Night. | Not applicable. |

The Windows updater saves the update as
`%TEMP%\Good Night-<version>-updater-<random>\Good Night-<version>-installer.exe` and runs it
after Good Night exits, so that run cannot delete it. A minute after the next launch
`windows/src-tauri/src/updates.rs` (function `remove_staged_installers`) deletes that installer and
then the folder if nothing else is in it. It opens only folders with this exact name pattern,
deletes only the installer file inside, and keeps any folder that holds anything else. An
installer that is still running is locked by Windows and goes at a later launch.

The lifetime policy is `core/src/retention.rs` (`TEMP_LIFETIME_SECS` is 1 hour,
`CHECK_RECORD_LIFETIME_SECS` is 30 days). The launch sweep, `windows/src-tauri/src/housekeeping.rs`,
runs before the app reads any file, looks only at `updates.json` and `updates.json.tmp` in the
folder above, and never touches `settings.json`. The tests in `core/tests/privacy.rs` fail if this page,
those constants and the sweep wiring drift apart.

## Deleting everything

Delete files by hand only after quitting Good Night, so nothing is rewritten after you clean.

- **macOS:** turn off **Start at login** in Settings, quit Good Night, run
  `defaults delete com.libertydesignstudio.goodnight`, then drag `Good Night.app` to the Trash.
  Delete `~/Library/Caches/com.libertydesignstudio.goodnight/` too if you want Sparkle's cache gone
  at once.
- **Windows:** uninstall Good Night from **Settings → Apps**. An uninstall removes the program
  files, the gamma-range value (only if it still holds 256), your **Start with Windows** entry, the
  updater's folders in your `%TEMP%`, and your account's `%APPDATA%\com.libertydesignstudio.goodnight`
  and `%LOCALAPPDATA%\com.libertydesignstudio.goodnight` folders, with the settings, update cache,
  log and WebView2 data. An uninstall while Good Night is running stops it and the browser
  processes behind its window first, so those two folders go too. An in-app update does not remove
  them. Other Windows accounts that ran Good
  Night keep their own two folders and login item; sign in as that user, turn off **Start with
  Windows** and delete the folders.
