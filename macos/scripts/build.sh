#!/bin/sh
# Builds a universal, signed "Good Night.app" into ./dist.
#   scripts/build.sh            sign with the first Developer ID identity, else ad-hoc
#   SIGN_ID=- scripts/build.sh  force ad-hoc signing
set -eu
cd "$(dirname "$0")/.."

swift build -c release --arch arm64 --arch x86_64 -Xswiftc -Osize
bin=.build/apple/Products/Release/GoodNight

app="dist/Good Night.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$bin" "$app/Contents/MacOS/GoodNight"
strip -x "$app/Contents/MacOS/GoodNight"
mkdir -p "$app/Contents/Frameworks"
ditto .build/artifacts/sparkle/Sparkle/Sparkle.xcframework/macos-arm64_x86_64/Sparkle.framework "$app/Contents/Frameworks/Sparkle.framework"
cp Resources/Info.plist "$app/Contents/Info.plist"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
swift scripts/icon.swift "$work/icon.png"
set_dir="$work/AppIcon.iconset"
mkdir "$set_dir"
for s in 16 32 128 256 512; do
  sips -z $s $s "$work/icon.png" --out "$set_dir/icon_${s}x${s}.png" >/dev/null
  sips -z $((s * 2)) $((s * 2)) "$work/icon.png" --out "$set_dir/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$set_dir" -o "$app/Contents/Resources/AppIcon.icns"

id="${SIGN_ID:-$(security find-identity -v -p codesigning | awk -F'"' '/Developer ID Application/ {print $2; exit}')}"
# Sign inside out: Sparkle's helpers, the framework, then the app.
sign() {
  if [ -z "$id" ] || [ "$id" = "-" ]; then codesign --force --sign - "$@"
  else codesign --force --options runtime --timestamp --sign "$id" "$@"; fi
}
fw="$app/Contents/Frameworks/Sparkle.framework/Versions/B"
sign "$fw/XPCServices/Installer.xpc"
sign --preserve-metadata=entitlements "$fw/XPCServices/Downloader.xpc"
sign "$fw/Autoupdate"
sign "$fw/Updater.app"
sign "$app/Contents/Frameworks/Sparkle.framework"
sign "$app"
codesign --verify --strict "$app"
echo "Built $app ($(du -sh "$app" | cut -f1))"
