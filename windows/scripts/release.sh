#!/bin/sh
# Builds the Windows installer and its signed update feed into windows/dist.
#   TAURI_SIGNING_PRIVATE_KEY=<key or path> [TAURI_SIGNING_PRIVATE_KEY_PASSWORD=...] scripts/release.sh
# Runs on Windows, or on macOS/Linux with cargo-xwin and NSIS installed.
set -eu
cd "$(dirname "$0")/.."
: "${TAURI_SIGNING_PRIVATE_KEY:?set TAURI_SIGNING_PRIVATE_KEY to the updater signing key}"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"
repo="${GOODNIGHT_REPO:-michael-berardi/goodnight}"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' src-tauri/Cargo.toml | head -1)
target=x86_64-pc-windows-msvc

cd src-tauri
if [ "$(uname -s)" = "Windows_NT" ] || [ -n "${WINDIR:-}" ]; then
  cargo tauri build --target "$target"
else
  cargo tauri build --runner cargo-xwin --target "$target"
fi
cd ..

built="src-tauri/target/$target/release/bundle/nsis/Good Night_${version}_x64-setup.exe"
mkdir -p dist
exe="dist/GoodNight-${version}-windows-x64-setup.exe"
cp "$built" "$exe"
cp "$built.sig" "$exe.sig"

python3 - "$version" "$repo" "$exe" <<'PY'
import json, sys, datetime, os
version, repo, exe = sys.argv[1:4]
sig = open(exe + ".sig").read().strip()
feed = {
    "version": version,
    "notes": f"Good Night {version}",
    "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "platforms": {"windows-x86_64": {
        "signature": sig,
        "url": f"https://github.com/{repo}/releases/download/v{version}/{os.path.basename(exe)}",
    }},
}
json.dump(feed, open(os.path.join(os.path.dirname(exe), "latest.json"), "w"), indent=2)
PY
shasum -a 256 "$exe"
