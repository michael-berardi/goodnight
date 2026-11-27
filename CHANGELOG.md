# Changelog

Changes are recorded in [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format. Versions follow [Semantic Versioning](https://semver.org/).

## [0.5.2] - 2026-11-26

### Added

- Automatic retention bounds for local debris: an abandoned atomic-write temporary file is swept at the next launch after one hour, and the Windows update-check record after 30 days. The policy lives in `core/src/retention.rs` and the sweep in `windows/src-tauri/src/housekeeping.rs`.
- The Windows updater's abandoned installer in `%TEMP%` is removed at the next launch, and its staging folder if empty; only this app's own staging names are opened.
- Regression tests tying the documented lifetimes, the sweep wiring, the update-check opt-outs and the local-file inventory together.

### Changed

- `PRIVACY.md` is now the full disclosure: every file written on macOS and Windows, what it contains, its lifetime and how to remove it. The README's **Privacy** section is a short summary linking to it.
- The Windows uninstaller now removes the machine-wide `GdiIcmGammaRange` value only when it still holds the value this installer wrote, and cleans the uninstalling profile's settings and cache folders. In-app updates keep all of them.
- The Windows background check remains anonymous and daily with a Settings opt-out and the `GOODNIGHT_NO_UPDATE_CHECK=1` / CI kill switches; it is skipped for standalone hidden or agent launches, while the `--autostart --hidden` login item still checks.

### Fixed

- Version aligned at 0.5.2 across both Cargo manifests and lockfiles, the Tauri config, the macOS Info.plist (build 7), the update docs and the privacy test expectations. README install links still name the published v0.5.0 assets.

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
