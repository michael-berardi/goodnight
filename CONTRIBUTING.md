# Contributing

Issues and focused pull requests are welcome.

## Checks

Run the shared Rust engine tests from the repository root:

```sh
cargo test --manifest-path core/Cargo.toml -j4
```

See [README.md](README.md#build-from-source) for full macOS (SwiftUI + `cargo-carapace`) and Windows (Tauri) build instructions. Checks run locally; this repository does not use GitHub Actions workflows.

## Ground rules

- Keep warmth curve, schedule, and settings logic in `core/` so macOS and Windows stay in step.
- Do not edit generated bindings (`macos/.../GoodNightEngine.swift`) by hand; regenerate them with `macos/scripts/build-core.sh`.
- Preserve normal-quit display calibration restoration and safe minimum launch brightness (at least 30%).
- Do not commit signing keys, notarization credentials, or local build artifacts.
