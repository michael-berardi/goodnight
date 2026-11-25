# Security Policy

## Scope

Good Night adjusts display colour lookup tables locally using a shared Rust engine (`core/`) with SwiftUI (macOS) and Tauri (Windows) shells. It requires no account and collects no telemetry.

Security-relevant surfaces include:

- Update feed and signature verification (Sparkle on macOS, Tauri updater on Windows) contacting GitHub Releases.
- Display gamma/LUT handling and the Windows installer registry setting (`GdiIcmGammaRange`).
- Local settings storage and global keyboard shortcut registration.

## Reporting a Vulnerability

Please report security issues privately through GitHub's
[security advisories](https://github.com/michael-berardi/goodnight/security/advisories/new)
rather than opening a public issue. Include the version, OS, and reproduction steps.
