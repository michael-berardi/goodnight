# Changelog

Changes are recorded in [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format. Versions follow [Semantic Versioning](https://semver.org/).

## [0.3.0] - 2026-11-26

### Added

- An update prompt with release notes: **Install Update**, **Remind Me Later** or **Skip This Version**, plus an opt-in to install future updates automatically (off by default).
- **Settings › Updates**: check automatically (on), install automatically (off), and the version with **Check Now** on one row.
- On Windows, automatic installs wait until no Good Night window is open and you have been away for 10 minutes.
- `GOODNIGHT_NO_UPDATE_CHECK=1` or `CI` turns background checks off.

### Changed

- Update checks are anonymous: a fixed User-Agent, no app version, at most once a day and never during startup.
- On Windows, a failed check shows "last check failed" in Settings instead of an error, and update files stay small and are removed when checks are turned off or the app is uninstalled.

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
