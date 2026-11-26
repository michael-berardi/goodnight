# Changelog

Changes are recorded in [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format. Versions follow [Semantic Versioning](https://semver.org/).

## [0.4.1] - 2026-11-26

### Fixed

- macOS update requests now use the fixed anonymous User-Agent `GoodNight-Update-Check/1.0`, set before Sparkle starts, and explicitly disable the system profile (`SUSendProfileInfo=false`).
- Windows login launches with `--autostart --hidden` and still receives the daily check; a standalone `--hidden` invocation remains headless/agent-only and stays silent.
- With `GOODNIGHT_NO_UPDATE_CHECK=1`, **Check now** shows **Checks disabled** instead of falsely reporting **Up to date**.
- Patch version bumped in both app manifests, the bundled version strings and the release links.

## [0.4.0] - 2026-11-26

### Changed

- Daily anonymous update checks are on by default again on macOS and Windows, in interactive use. Automatic installation stays off; macOS still requires the unchecked-by-default opt-in in Sparkle's prompt, and Windows installs only after you choose **Install**.
- Checks are skipped in CI, headless/print launches and when `GOODNIGHT_NO_UPDATE_CHECK=1` is set. **Settings → Updates → Check for updates automatically** remains the persistent opt-out.
- Windows records one check per 24 hours in a private cache file, uses a 4-second timeout, and keeps the cause of a failed check for the Settings sheet. The settings migration now carries forward only the old 0.2 default, leaving an explicit opt-out (or a 0.3 choice) untouched.
- README Privacy/Updates now describe the anonymous request, and `docs/updates.md` carries the `<!-- update-contract: v1 -->` marker.

### Added

- Privacy tests asserting the new defaults, the environment/noninteractive gates, the daily cache and the old-default migration behaviour. A rendered Windows Settings caption documents the opt-out.

## [0.3.0] - 2026-11-26

### Changed

- Automatic update checks are now off by default on macOS and Windows. They run only after the user enables them; a manual check is still always available. Updates remain signed and verified.
- Upgrading from 0.2.0 or earlier also turns automatic checks off, because those versions saved the old default rather than a choice. Turn them back on in Settings if you want them.

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
