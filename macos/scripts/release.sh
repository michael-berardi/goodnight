#!/bin/sh
# Builds, signs, notarizes and staples "Good Night.app" and a DMG into macos/dist.
#   NOTARY_PROFILE=<keychain profile> scripts/release.sh
# Create the profile once with: xcrun notarytool store-credentials <name> ...
set -eu
cd "$(dirname "$0")/.."
: "${NOTARY_PROFILE:?set NOTARY_PROFILE to a notarytool keychain profile}"
version=$(/usr/libexec/PlistBuddy -c 'Print CFBundleShortVersionString' Resources/Info.plist)

scripts/build.sh
app="dist/Good Night.app"
zip="dist/GoodNight-notarize.zip"
ditto -c -k --keepParent "$app" "$zip"
xcrun notarytool submit "$zip" --keychain-profile "$NOTARY_PROFILE" --wait
xcrun stapler staple "$app"
rm -f "$zip"

dmg="dist/GoodNight-$version-mac.dmg"
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
cp -R "$app" "$stage/"
ln -s /Applications "$stage/Applications"
rm -f "$dmg"
hdiutil create -volname "Good Night" -srcfolder "$stage" -fs HFS+ -format UDZO "$dmg" >/dev/null
id=$(security find-identity -v -p codesigning | awk -F'"' '/Developer ID Application/ {print $2; exit}')
codesign --force --timestamp --sign "$id" "$dmg"
xcrun notarytool submit "$dmg" --keychain-profile "$NOTARY_PROFILE" --wait
xcrun stapler staple "$dmg"
spctl --assess --type open --context context:primary-signature -v "$dmg"
spctl --assess --type execute -v "$app"
shasum -a 256 "$dmg"
scripts/appcast.sh "$dmg"
