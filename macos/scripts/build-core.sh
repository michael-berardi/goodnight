#!/bin/sh
# Builds the shared engine (../core) into macos/Core/GoodnightCore.xcframework and refreshes its
# Swift bindings. Needs Rust (with the x86_64-apple-darwin and aarch64-apple-darwin targets) and
# the Carapace tool:
#   cargo install --git https://github.com/michael-berardi/carapace --tag v0.1.0 cargo-carapace
set -eu
cd "$(dirname "$0")/../../core"

if ! command -v cargo-carapace >/dev/null 2>&1; then
  echo "error: cargo-carapace is not installed. Run:" >&2
  echo "  cargo install --git https://github.com/michael-berardi/carapace --tag v0.1.0 cargo-carapace" >&2
  exit 1
fi

cargo carapace gen swift -p goodnight-core --out ../macos/Sources/GoodNight
cargo carapace build apple -p goodnight-core --out ../macos/Core
