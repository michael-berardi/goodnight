# Privacy

Good Night changes display colours locally. It has no account, no analytics, no telemetry, no crash reporting,
no install ID or machine fingerprint. Nothing about your display, schedule or location leaves the device.
The only network feature is the update check below.

## Update checks

A check is one anonymous HTTPS GET of the app's signed update feed on GitHub Releases: `appcast.xml`
on macOS, `latest.json` on Windows. The request carries the fixed User-Agent
`goodnight-update-check` and no identifier, app version, usage or machine data (on macOS also a
fixed `Accept-Language: en`; Sparkle's system profile stays off). No settings, warmth schedule or
file contents are sent. As with any web request, GitHub sees your IP address and the request headers.

- At most once a day, in the background, starting a minute after launch, never during startup.
- A failed check is recorded locally with its cause (Settings shows "last check failed") and waits
  for the next day; it is not retried in a loop and not sent anywhere.
- Off when **Check for updates automatically** is off, when `GOODNIGHT_NO_UPDATE_CHECK=1` is
  set, or when `CI` is set. **Check Now** still works.

Downloading an update is a second request, to the release asset the signed feed names. GitHub is
the only third party Good Night contacts; there are no user-configured cloud services.

## What is stored

Everything stays on this device:

- Settings: warmth, brightness, schedule, menu-bar/Dock visibility and the two update switches,
  and nothing else: no prompts, files, credentials or location.
  - macOS: `~/Library/Preferences/com.libertydesignstudio.goodnight.plist` (UserDefaults). Sparkle
    keeps its update settings and last check time there too.
  - Windows: `settings.json` in the app's config folder under your user profile
    (`%APPDATA%\com.libertydesignstudio.goodnight`). That folder carries the user-only AppData
    permissions; on Unix-like hosts the folder is created 0700 and the file 0600.
- Windows only: `updates.json` (last check time, last failure cause, skipped version, last
  version shown; written 0600 on Unix-like hosts) and `updates.log` (background update events,
  at most 256 KB). Both sit next to `settings.json`.
- No history files. Other diagnostics go to the macOS unified log or the Windows console.
- The interface uses bundled assets and system fonts: no third-party fonts, scripts or web requests.

## Removing it

- Turning **Check for updates automatically** off on Windows deletes `updates.json` and
  `updates.log`. Uninstalling deletes them too; choosing "Delete the application data" in the
  uninstaller also deletes the settings.
- The Windows updater stages the installer in the temp folder and runs it after Good Night
  quits; the next launch deletes that folder.
- On macOS, Sparkle downloads into its own cache and removes the download after installing.
- To delete everything by hand, quit Good Night, then on macOS run
  `defaults delete com.libertydesignstudio.goodnight` and delete the app; on Windows delete the
  `com.libertydesignstudio.goodnight` folder in `%APPDATA%` (it holds `settings.json`,
  `updates.json` and `updates.log`).
