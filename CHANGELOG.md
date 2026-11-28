# Changelog

Changes are recorded in [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format. Versions follow [Semantic Versioning](https://semver.org/).

## [0.5.3] - 2026-11-28

### Fixed

- On Windows, clicking any setting no longer freezes Good Night. The setting was applied to the
  screen, then the interface stopped responding and `settings.json` was never written. Two halves
  of the same wait were to blame: the click ran on the interface thread and waited there for the
  settings engine, and the engine reported the new setting by updating the tray tooltip, which
  Windows carries out on that same interface thread. Each waited for the other. The setting commands
  now hand their change to the engine and return at once, in the order the clicks arrived, and the
  tray tooltip is handed to the interface thread instead of waited for. macOS never had this shape.
- A Windows uninstall while Good Night is running now asks the app to quit, which restores the
  screen's normal warmth, and stops the browser processes behind its window before removing the app
  data, so an uninstall leaves no `%APPDATA%` or `%LOCALAPPDATA%` folder behind for the account that
  ran it. A forced stop follows if the app does not quit within ten seconds. If a folder is still
  locked after five seconds, the next Windows start removes what is left.
- Regression tests hold both halves: `core/tests/shell_threads.rs` runs the real engine through the
  shape that hung Windows, checks that no shell command waits for the engine, and checks the tray
  hand-off, and `core/tests/privacy.rs` checks that the uninstaller asks the app to quit and stops
  it before deleting its data, and only for a real uninstall.

## [0.5.2] - 2026-11-27

### Added

- Bounded local files on Windows: an abandoned `updates.json.tmp` is deleted at the next launch once it is an hour old, and an `updates.json` that no check has rewritten for 30 days is deleted too (a skipped version is then offered again). Settings never expire. The policy is in `core/src/retention.rs`; the sweep in `windows/src-tauri/src/housekeeping.rs` runs at startup, before any file is read.
- Privacy regression tests that keep PRIVACY.md, the lifetimes, the launch sweep, the uninstall hook and the README summary in agreement.

### Changed

- A real Windows uninstall now also removes the machine-wide `GdiIcmGammaRange` value, but only while it still holds the 256 the installer wrote, and the uninstalling account's settings, update files and WebView2 data, without needing **Delete the application data**. In-app updates keep all of them.
- The Windows updater's leftover installer in the temp folder is removed more carefully: a minute after the next launch, only the `Good Night-<version>-installer.exe` inside a `Good Night-<version>-updater-<random>` folder is deleted, then the folder if it is empty. A folder holding anything else is kept.
- PRIVACY.md lists every file and registry value Good Night writes on each platform, what it holds, how long it lives and what removes it, including Sparkle's cache and the login items. The README **Privacy** section stays a short summary that links it.
- The daily update check is unchanged: anonymous, on by default, off with **Settings › Updates**, `GOODNIGHT_NO_UPDATE_CHECK=1` or `CI`.

### Fixed

- The docs no longer say Sparkle removes each macOS download after installing: Sparkle keeps downloads in its cache and deletes those 10 or more days old when it next downloads or installs.

## [0.5.0] - 2026-11-26

### Added

- An update prompt with release notes: **Install Update**, **Remind Me Later** or **Skip This Version**, plus an opt-in to install future updates automatically (off by default).
- **Settings › Updates**: check automatically (on), install automatically (off), and the version with **Check Now** on one row.
- On Windows, automatic installs wait until no Good Night window is open and you have been away for 10 minutes.
- `GOODNIGHT_NO_UPDATE_CHECK=1` or `CI` turns background checks off.
- PRIVACY.md lists what a check sends, what is stored where, and how to delete it.

### Changed

- Update checks are anonymous: a fixed User-Agent, no app version, at most once a day and never during startup.
- On Windows, a failed check shows "last check failed" in Settings instead of an error, and update files stay small and are removed when checks are turned off or the app is uninstalled.
- Windows settings are written owner-only (0700 folder, 0600 file where the system supports it; the user-only AppData folder otherwise).
- A privacy test fails if analytics, telemetry or crash-reporting packages appear in the sources.

## [0.2.0] - 2026-10-16

### Changed

- Unified macOS and Windows on one shared Rust engine (`core/`, built on Carapace) for warmth curves, sun times, schedules, and settings, matching the 0.1.0 curve across eight time zones.
- Windows window and tray flyout previews now render colours from the shared engine.
- Fixed building from source on current Swift toolchains.

## [0.1.0] - 2026-09-30

### Added

- Initial release for macOS 13+ (signed and notarized) and Windows 10/11 x64 (preview).
- Warmth slider from 6,500K to 1,200K with six presets (Day, Golden, Sunset, Candle, Night, Midnight) and software brightness dimming down to black.
- Follow the Sun scheduling based on the system time zone's reference city (no location permission).
- Optional menu bar/Dock visibility on macOS, tray/window modes on Windows, global shortcuts, and signed automatic updates.
