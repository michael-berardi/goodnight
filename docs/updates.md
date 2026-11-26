<!-- update-contract: v1 -->
# Updates

Good Night tells you when a newer stable release exists and installs it in one step. Checking
is on by default; installing automatically is an opt-in and off by default.

## The prompt

When a check finds a newer release, Good Night shows "Good Night 0.4.0 is available — you have
0.3.0." with the release notes and three choices:

- **Install Update** downloads, verifies and installs it, then starts the new version.
- **Remind Me Later** hides it until the next daily check.
- **Skip This Version** hides it until a newer release appears. **Check Now** still shows it.

Below the notes, **Automatically download and install updates in the future** is unchecked
until you choose it.

On macOS this is Sparkle's standard window. If Good Night runs without a Dock icon (menu bar
only), a scheduled check never opens a window over your work: the menu bar popover shows
"Good Night 0.4.0 is available" with **Review**, which opens the prompt. On Windows the prompt
appears at the top of the Good Night window, and the tray flyout shows the same one-line
banner.

## Settings › Updates

- **Check for updates automatically**: on by default.
- **Install updates automatically**: off by default; needs automatic checks.
- The version on the left and **Check Now** on the right. On Windows, a failed check adds
  "last check failed" to the version; hover for the cause.

On macOS, **Good Night › Check for Updates…** also checks.

## Checks

One anonymous request to the signed feed on GitHub Releases (`appcast.xml` on macOS,
`latest.json` on Windows), at most once a day, in the background, starting a minute after
launch. The feed lists only the latest stable release, and the app also ignores any version
that is not a plain `x.y.z` newer than the running one. `GOODNIGHT_NO_UPDATE_CHECK=1` or `CI`
turns background checks off. What is sent and stored: [PRIVACY.md](../PRIVACY.md).

## Automatic installs

With **Install updates automatically** on:

- macOS: Sparkle downloads the update in the background and installs it when Good Night quits.
- Windows: the update waits until no Good Night window is open and you have been away from
  keyboard and mouse for 10 minutes. Then the installer replaces the app and starts the new
  version. Good Night is installed for all users, so Windows asks for administrator approval.

If an automatic install fails, the current version keeps running, the cause is recorded, and
the prompt appears instead.

## Safety

- macOS: the download must match the EdDSA signature in the appcast (public key in the app's
  `Info.plist`) and pass code signing and Gatekeeper. Sparkle replaces the app only after
  that, and keeps the old one if the replacement fails.
- Windows: the installer must match the minisign signature in `latest.json` (public key in
  `tauri.conf.json`) before it is run. A mismatch stops the update and nothing changes.
- Good Night is distributed only as these direct downloads; there is no package-manager install
  to defer to. Earlier versions stay on the Releases page if you need to go back.

## No debris

- Windows keeps `updates.json` and a background log `updates.log` capped at 256 KB, next to
  `settings.json`. Turning automatic checks off or uninstalling deletes both.
- The Windows updater stages the installer in the temp folder and runs it after Good Night
  quits; the next launch deletes that folder.
- macOS writes no update files of its own: Sparkle keeps its settings in the app's preferences
  and removes each download after installing.
- No rollback copies are kept on either platform.

## Testing locally

On macOS, `GOODNIGHT_UPDATE_FEED=<url>` points the updater at a test appcast, and
`"Good Night.app/Contents/MacOS/GoodNight" --update-probe` checks it once without any window.
It prints the result and exits 0 when up to date, 10 when an update is available, and 1 when the
check failed. An install from a test feed still needs a valid signature.

Tests (no network beyond a local test server):

```sh
cargo test --manifest-path core/Cargo.toml
cargo test --manifest-path windows/src-tauri/Cargo.toml
node --test windows/ui/tests/updates.test.js
cd macos && swift test
```
