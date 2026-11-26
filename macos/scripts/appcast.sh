#!/bin/sh
# Writes dist/appcast.xml, the Sparkle update feed, for a notarized DMG.
#   scripts/appcast.sh dist/GoodNight-<version>-mac.dmg
# Signs with the EdDSA key in the login keychain (account $SPARKLE_ACCOUNT, default "goodnight")
# or with $SPARKLE_KEY_FILE. Set GOODNIGHT_DOWNLOAD_BASE to test against another server.
set -eu
cd "$(dirname "$0")/.."
dmg="$1"
plist=Resources/Info.plist
version=$(/usr/libexec/PlistBuddy -c 'Print CFBundleShortVersionString' "$plist")
build=$(/usr/libexec/PlistBuddy -c 'Print CFBundleVersion' "$plist")
repo="${GOODNIGHT_REPO:-michael-berardi/goodnight}"
base="${GOODNIGHT_DOWNLOAD_BASE:-https://github.com/$repo/releases/download/v$version}"
bin=.build/artifacts/sparkle/Sparkle/bin
# This version's CHANGELOG section becomes the short release notes in the update prompt.
notes=$(awk -v v="[$version]" '/^## \[/ { p = index($0, v) > 0; next } p' ../CHANGELOG.md)
[ -n "$notes" ] || { echo "CHANGELOG.md has no section for $version" >&2; exit 1; }
if [ -n "${SPARKLE_KEY_FILE:-}" ]; then
  sig=$("$bin/sign_update" --ed-key-file "$SPARKLE_KEY_FILE" "$dmg")
else
  sig=$("$bin/sign_update" --account "${SPARKLE_ACCOUNT:-goodnight}" "$dmg")
fi
cat > "$(dirname "$dmg")/appcast.xml" <<XML
<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0" xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle">
  <channel>
    <title>Good Night</title>
    <item>
      <title>Good Night $version</title>
      <pubDate>$(LC_ALL=C date -u "+%a, %d %b %Y %H:%M:%S +0000")</pubDate>
      <sparkle:version>$build</sparkle:version>
      <sparkle:shortVersionString>$version</sparkle:shortVersionString>
      <sparkle:minimumSystemVersion>13.0</sparkle:minimumSystemVersion>
      <description sparkle:format="markdown"><![CDATA[$notes]]></description>
      <enclosure url="$base/$(basename "$dmg")" type="application/octet-stream" $sig />
    </item>
  </channel>
</rss>
XML
echo "Wrote $(dirname "$dmg")/appcast.xml"
