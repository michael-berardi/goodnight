# Privacy

Good Night changes display colours locally. It has no account, no analytics and no telemetry.
Nothing about your display, schedule or location leaves the device.

## Update checks

A check is one anonymous GET of the app's signed update feed on GitHub Releases: `appcast.xml`
on macOS, `latest.json` on Windows. The request carries the fixed User-Agent
`goodnight-update-check` and no identifier, app version, usage or machine data (on macOS also a
fixed `Accept-Language: en`; Sparkle's system profile stays off).

- At most once a day, in the background, starting a minute after launch, never during startup.
- A failed check is recorded with its cause (Settings shows "last check failed") and waits for
  the next day; it is not retried in a loop.
- Off when **Check for updates automatically** is off, when `GOODNIGHT_NO_UPDATE_CHECK=1` is
  set, or when `CI` is set. **Check Now** still works.

Downloading an update is a second request, to the release asset the signed feed names.

## What is stored

- Settings, including the two update switches: `settings.json` on Windows, the app's
  preferences on macOS (Sparkle keeps its update settings and last check time there too).
- Windows only: `updates.json` (last check time, last failure cause, skipped version, last
  version shown) and `updates.log` (background update events, at most 256 KB). Both sit next
  to `settings.json`.

## Removing it

- Turning **Check for updates automatically** off on Windows deletes `updates.json` and
  `updates.log`. Uninstalling deletes them too; choosing "Delete the application data" in the
  uninstaller also deletes the settings.
- The Windows updater stages the installer in the temp folder and runs it after Good Night
  quits; the next launch deletes that folder.
- On macOS, Sparkle downloads into its own cache and removes the download after installing.
  Deleting the app and its preferences removes everything.
