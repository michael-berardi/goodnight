# Changelog

Changes are recorded in [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format. Versions follow [Semantic Versioning](https://semver.org/).

## [0.3.0] - 2026-11-26

### Changed

- Automatic update checks are now off by default on macOS and Windows. They run only after the user enables them; a manual check is still always available. Updates remain signed and verified.

### Added

- A `## Privacy` section in the README describing the only network request, local storage, retention and deletion.
- A privacy regression test that fails if telemetry/analytics packages are reintroduced or either platform defaults to automatic update checks.

### Fixed

- Windows now creates its settings directory and file with private permissions on Unix-like hosts (0700/0600); it continues to inherit the user-only AppData ACL on Windows.

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
