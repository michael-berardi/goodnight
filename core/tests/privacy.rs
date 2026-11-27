//! Privacy regressions: no analytics/crash-reporting packages or endpoints in shipped sources,
//! and one anonymous daily update check by default in either platform's configuration.

use std::fs;
use std::path::{Path, PathBuf};

use carapace::Engine;
use goodnight_core::{Config, GoodNight, Settings};

/// Packages, SDKs and endpoints that would send usage data off the machine.
const BANNED: &[&str] = &[
    "analytics",
    "telemetry",
    "sentry",
    "posthog",
    "mixpanel",
    "amplitude",
    "bugsnag",
    "crashlytics",
    "appcenter",
    "googletagmanager",
    "google-analytics",
    "datadog",
    "hotjar",
    "countly",
    "opentelemetry",
    "newrelic",
    "firebase",
    "appsflyer",
    "segment.com",
    "segmentio",
    "plausible.io",
    "umami",
    "matomo",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("core/ lives in the repository")
        .to_path_buf()
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if !matches!(&*name, ".git" | "target" | ".build" | "dist" | "gen" | "Core") {
                collect(&path, out);
            }
        } else if name != "privacy.rs"
            && matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("rs" | "toml" | "swift" | "js" | "html" | "json" | "plist" | "lock")
            )
        {
            out.push(path);
        }
    }
}

#[test]
fn no_telemetry_packages_or_endpoints_in_shipped_sources() {
    let mut files = Vec::new();
    collect(&root(), &mut files);
    assert!(files.len() > 10, "the source scan must see the repository");
    for file in files {
        let text = fs::read_to_string(&file).unwrap_or_default().to_ascii_lowercase();
        for token in BANNED {
            assert!(
                !text.contains(token),
                "{token} reintroduced in {}",
                file.display()
            );
        }
    }
}

#[test]
fn default_config_allows_a_daily_anonymous_check_but_not_installation() {
    assert!(Settings::default().auto_update, "daily checks must be on by default");
    let config = Config {
        time_zone: "UTC".into(),
        fixed_now: Some(0.0),
        ..Config::default()
    };
    assert!(config.settings.auto_update);
    let (engine, _) = Engine::<GoodNight>::start(config);
    let state: serde_json::Value = serde_json::from_str(engine.snapshot()).unwrap();
    assert_eq!(state["autoUpdate"], true);
    // The engine performs no I/O; both shells gate their updater on this flag and on opt-outs.
}

#[test]
fn macos_sparkle_defaults_to_daily_checks_and_no_auto_install() {
    let root = root();
    let plist = fs::read_to_string(root.join("macos/Resources/Info.plist")).unwrap();
    assert!(
        plist.contains("<key>SUEnableAutomaticChecks</key>\n\t<true/>"),
        "Sparkle must schedule the anonymous daily check"
    );
    assert!(
        plist.contains("<key>SUAutomaticallyUpdate</key>\n\t<false/>"),
        "Sparkle must never install without the user's opt-in"
    );
    assert!(plist.contains("<integer>86400</integer>"));
    assert!(
        plist.contains("<key>SUSendProfileInfo</key>\n\t<false/>"),
        "Sparkle must not send the system profile with the check"
    );
    let app = fs::read_to_string(root.join("macos/Sources/GoodNight/GoodNightApp.swift")).unwrap();
    assert!(
        app.contains("GoodNight-Update-Check/1.0"),
        "the User-Agent must be fixed, with no machine or OS identifiers"
    );
    assert!(app.contains("updater.userAgentString = Self.userAgent"));
    assert!(
        app.contains("startingUpdater: false"),
        "set the User-Agent before Sparkle's first scheduled check"
    );
    assert!(app.contains("try updater.updater.start()"));
    assert!(
        app.contains("automaticallyChecksForUpdates ?? true"),
        "the Settings toggle must read on when no preference was saved"
    );
    assert!(app.contains("GOODNIGHT_NO_UPDATE_CHECK"), "env opt-out");
    assert!(app.contains("env[\"CI\"] != nil"), "CI must disable checks");
    let generated = fs::read_to_string(root.join("macos/Sources/GoodNight/GoodNightEngine.swift")).unwrap();
    assert!(
        generated.contains("autoUpdate: Bool = true"),
        "generated Swift bindings must match the core's checks-on default"
    );
}

#[test]
fn windows_periodic_check_is_gated_on_setting_opt_out_and_daily_cache() {
    let main = fs::read_to_string(root().join("windows/src-tauri/src/main.rs")).unwrap();
    let gate = main
        .find("let enabled = st.core_state().auto_update")
        .expect("the Windows timer must check the user setting before contacting the feed");
    let body = &main[gate..];
    let call = body.find("check_update(handle.clone())").expect("timer calls check_update");
    let close = body.find("\n                }\n            });").expect("timer gate block closes");
    assert!(call < close, "check_update must sit inside the gated block");
    assert_eq!(main.matches("check_update(handle.clone())").count(), 1, "no ungated timer check");
    assert!(main.contains("vec![\"--autostart\", \"--hidden\"]"), "login launches hidden but marked as a real user");
    assert!(main.contains("checks_allowed(hidden, autostart, std::env::var_os(\"CI\").is_some(), env_opt_out())"));
    assert!(main.contains("GOODNIGHT_NO_UPDATE_CHECK"));
    assert!(main.contains("check_due(&st.cache_path)"), "at most one check per day");
    assert!(main.contains("const CHECK_TIMEOUT: Duration = Duration::from_secs(4)"), "short timeout");
    assert!(main.contains("old_default = raw.get(\"autoUpdate\")"));
    assert!(!main.contains("settings.auto_update = false"), "no forced-off default");
}

#[test]
fn docs_and_changelog_describe_the_v1_contract() {
    let root = root();
    let docs = fs::read_to_string(root.join("docs/updates.md")).unwrap();
    assert!(docs.contains("<!-- update-contract: v1 -->"));
    assert!(docs.contains("GOODNIGHT_NO_UPDATE_CHECK=1"));
    assert!(
        docs.contains("no 4-second timeout on macOS"),
        "the macOS framework timeout limitation must be explicit"
    );
    assert!(docs.contains("updateCheckInterval"), "schedule is not a request timeout");
    let readme = fs::read_to_string(root.join("README.md")).unwrap();
    assert!(readme.contains("[docs/updates.md](docs/updates.md)"));
    assert!(readme.contains("on by default"));
    let changelog = fs::read_to_string(root.join("CHANGELOG.md")).unwrap();
    assert!(changelog.contains("[0.5.2]"), "the branch must sit above the published v0.5.0");
    assert!(changelog.contains("v0.5.0"), "the published release must be named");
}

#[test]
fn a_windows_login_launch_checks_but_a_hidden_agent_launch_does_not() {
    let main = fs::read_to_string(root().join("windows/src-tauri/src/main.rs")).unwrap();
    let gate = main.find("fn checks_allowed(").expect("the gate");
    let body = &main[gate..gate + 400];
    assert!(body.contains("(!hidden || autostart)"));
    assert!(main.contains("assert!(checks_allowed(true, true, false, false))"));
    assert!(main.contains("assert!(!checks_allowed(true, false, false, false)"));
}

#[test]
fn disabled_check_now_is_not_reported_as_up_to_date() {
    let main = fs::read_to_string(root().join("windows/src-tauri/src/main.rs")).unwrap();
    assert!(main.contains("checks_disabled: env_opt_out()"));
    assert!(main.contains("Ok(CheckOutcome { version: None, disabled: true })"));
    let js = fs::read_to_string(root().join("windows/ui/app.js")).unwrap();
    assert!(js.contains("Checks disabled"));
    let disabled = js.find("r.disabled").expect("the disabled branch");
    let up_to_date = js.find("Up to date").expect("the ordinary no-update result");
    assert!(disabled < up_to_date, "the disabled branch must precede and guard 'Up to date'");
    let html = fs::read_to_string(root().join("windows/ui/index.html")).unwrap();
    assert!(
        !html.contains("id=\"update-caption\""),
        "the switch label and docs explain the check; no redundant caption"
    );
    assert!(!js.contains("update-caption"), "no orphaned caption renderer");
}

#[test]
fn download_links_are_published_and_windows_deviations_are_documented() {
    let readme = fs::read_to_string(root().join("README.md")).unwrap();
    assert!(!readme.contains("GoodNight-0.5.2-mac.dmg"), "no unpublished macOS asset link");
    assert!(!readme.contains("GoodNight-0.5.2-windows-x64-setup.exe"), "no unpublished Windows asset link");
    assert!(!readme.contains("releases/download/v0.2.0/"), "no stale v0.2.0 asset links");
    assert!(readme.contains("releases/download/v0.5.0/GoodNight-0.5.0-mac.dmg"));
    assert!(readme.contains("releases/download/v0.5.0/GoodNight-0.5.0-windows-x64-setup.exe"));
    assert!(readme.contains("latest published release, v0.5.0"));
    let docs = fs::read_to_string(root().join("docs/updates.md")).unwrap();
    assert!(docs.contains("**no Install-updates-automatically toggle**"));
    assert!(docs.contains("**no Remind Me Later and no Skip This Version buttons**"));
    assert!(docs.contains("including 0.5.2"), "the Windows deviation stays tied to the branch version");
}

#[test]
fn updater_feeds_use_the_published_latest_release_urls() {
    let config = fs::read_to_string(root().join("windows/src-tauri/tauri.conf.json")).unwrap();
    assert!(
        config.contains("https://github.com/michael-berardi/goodnight/releases/latest/download/latest.json"),
        "the Windows plugin must read the published feed"
    );
    let plist = fs::read_to_string(root().join("macos/Resources/Info.plist")).unwrap();
    assert!(
        plist.contains("https://github.com/michael-berardi/goodnight/releases/latest/download/appcast.xml"),
        "Sparkle must read the published feed"
    );
}

#[test]
fn branch_version_is_bumped_in_every_manifest() {
    for path in [
        "core/Cargo.toml",
        "core/Cargo.lock",
        "windows/src-tauri/Cargo.toml",
        "windows/src-tauri/Cargo.lock",
        "windows/src-tauri/tauri.conf.json",
        "macos/Resources/Info.plist",
    ] {
        let text = fs::read_to_string(root().join(path)).unwrap();
        assert!(text.contains("0.5.2"), "{path} must carry 0.5.2");
        assert!(!text.contains("0.4.2"), "{path} must not carry the old branch version");
    }
    let plist = fs::read_to_string(root().join("macos/Resources/Info.plist")).unwrap();
    assert!(
        plist.contains("<key>CFBundleVersion</key>\n\t<string>7</string>"),
        "the macOS build number must stay above published v0.5.0"
    );
}

/// The README's Privacy section, from its heading to the next one.
fn readme_privacy_section() -> String {
    let readme = fs::read_to_string(root().join("README.md")).unwrap();
    let start = readme.find("\n## Privacy\n").expect("the README has a Privacy section");
    let body = &readme[start + 1..];
    let end = body.find("\n## ").expect("the Privacy section ends at the next heading");
    body[..end].to_string()
}

#[test]
fn readme_privacy_is_a_short_summary_that_links_the_full_disclosure() {
    let section = readme_privacy_section();
    let lines: Vec<&str> = section.lines().filter(|l| !l.trim().is_empty()).collect();
    assert!(
        (4..=7).contains(&lines.len()),
        "README Privacy must be the heading plus 3-6 short lines, found {} lines",
        lines.len()
    );
    assert!(lines[0].starts_with("## Privacy"));
    assert!(
        section.contains("[PRIVACY.md](PRIVACY.md)"),
        "the short form must link the full disclosure instead of carrying it"
    );
    for detail in ["update-check.json", "settings.json.tmp", "~/Library/Preferences", "GdiIcmGammaRange"] {
        assert!(!section.contains(detail), "{detail} is detail for PRIVACY.md, not the README summary");
    }
}

#[test]
fn every_local_write_is_inventoried_with_a_lifetime() {
    let privacy = fs::read_to_string(root().join("PRIVACY.md")).unwrap();
    for required in [
        "Lifetime",
        // settings, both platforms
        "~/Library/Preferences/com.libertydesignstudio.goodnight.plist",
        r"%APPDATA%\com.libertydesignstudio.goodnight\settings.json",
        // the Windows update-check record and the atomic-write temp files
        r"%LOCALAPPDATA%\com.libertydesignstudio.goodnight\update-check.json",
        "settings.json.tmp",
        "update-check.json.tmp",
        "1 hour",
        "30 days",
        // logs, install leftovers and the way out
        "no log files",
        "GdiIcmGammaRange",
        "Start at login",
        "defaults delete com.libertydesignstudio.goodnight",
        "uninstall",
    ] {
        assert!(privacy.contains(required), "PRIVACY.md must document {required}");
    }
    let readme = fs::read_to_string(root().join("README.md")).unwrap();
    assert!(readme.contains("[PRIVACY.md](PRIVACY.md)"), "the README must point at it");
    let docs = fs::read_to_string(root().join("docs/updates.md")).unwrap();
    assert!(docs.contains("[PRIVACY.md](../PRIVACY.md)"), "the update docs must point at it too");
}

#[test]
fn retention_bounds_are_pure_documented_and_applied_at_launch() {
    use goodnight_core::retention::{
        disposition, Disposition, CHECK_RECORD_LIFETIME_SECS, SETTINGS_FILE_NAME, TEMP_LIFETIME_SECS,
    };
    let now = 1_800_000_000u64;
    assert_eq!(TEMP_LIFETIME_SECS, 3_600, "PRIVACY.md promises one hour");
    assert_eq!(CHECK_RECORD_LIFETIME_SECS, 30 * 24 * 3_600, "PRIVACY.md promises 30 days");
    assert_eq!(disposition("settings.json.tmp", now - TEMP_LIFETIME_SECS, now), Disposition::Remove);
    assert_eq!(disposition("update-check.json.tmp", now - 1, now), Disposition::Keep, "a live write");
    assert_eq!(disposition("update-check.json", now - CHECK_RECORD_LIFETIME_SECS, now), Disposition::Remove);
    assert_eq!(disposition("update-check.json", now - 3_600, now), Disposition::Keep);
    assert_eq!(
        disposition(SETTINGS_FILE_NAME, now - 10 * CHECK_RECORD_LIFETIME_SECS, now),
        Disposition::Keep,
        "settings are the user's data, not debris"
    );
    assert_eq!(disposition("installer-notes.txt", 0, now), Disposition::Keep, "only this app's files");

    let privacy = fs::read_to_string(root().join("PRIVACY.md")).unwrap();
    assert!(
        privacy.contains("core/src/retention.rs") && privacy.contains("windows/src-tauri/src/housekeeping.rs"),
        "PRIVACY.md must name the code that enforces the bounds"
    );

    let main = fs::read_to_string(root().join("windows/src-tauri/src/main.rs")).unwrap();
    assert!(main.contains("mod housekeeping;"), "the shell must own the sweep");
    assert_eq!(main.matches("housekeeping::sweep(").count(), 1, "one sweep, at startup");
    let sweep = main.find("housekeeping::sweep(").unwrap();
    let read = main.find("std::fs::read(&path)").expect("settings are read at startup");
    assert!(sweep < read, "debris is cleared before the settings file is read");
    assert_eq!(
        main.matches("housekeeping::sweep_updater_leftovers(&std::env::temp_dir())").count(),
        1,
        "the updater's abandoned installer in %TEMP% is swept once at startup"
    );

    let housekeeping = fs::read_to_string(root().join("windows/src-tauri/src/housekeeping.rs")).unwrap();
    assert!(housekeeping.contains("goodnight_core::retention"), "the bounds are the shared policy");
    for forbidden in ["check_due", "record_check", "check_update", "install_update"] {
        assert!(
            !housekeeping.contains(forbidden),
            "housekeeping must not drive or duplicate the update check: {forbidden}"
        );
    }
    // Only the (test-only) scratch cleanup may remove folders; the sweeper itself deletes files.
    let shipping = housekeeping.split("#[cfg(test)]").next().unwrap();
    assert!(!shipping.contains("remove_dir_all"), "the sweeper removes files, never folders");
    assert!(shipping.contains("remove_file"), "and it does remove the expired ones");
}

#[test]
fn uninstalling_leaves_no_install_leftovers() {
    let nsh = fs::read_to_string(root().join("windows/src-tauri/installer-hooks.nsh")).unwrap();
    let value = r#"HKLM "SOFTWARE\Microsoft\Windows NT\CurrentVersion\ICM" "GdiIcmGammaRange""#;
    assert!(nsh.contains("!macro NSIS_HOOK_PREUNINSTALL"), "the uninstaller must clean up");
    assert!(nsh.contains(&format!(r#"WriteRegDWORD {value} 256"#)), "the install-side value");
    assert!(nsh.contains(&format!(r#"ReadRegDWORD $0 {value}"#)), "read before deleting");
    assert!(nsh.contains("IntCmp $0 256"), "only a value this installer wrote is removed");
    assert!(nsh.contains(&format!("DeleteRegValue {value}")));
    assert!(nsh.contains(r#"RMDir /r "$APPDATA\com.libertydesignstudio.goodnight""#));
    assert!(nsh.contains(r#"RMDir /r "$LOCALAPPDATA\com.libertydesignstudio.goodnight""#));
}

/// Tauri's NSIS template sets `$UpdateMode` from the updater's `/UPDATE` flag in `un.onInit`.
/// Only a real uninstall may reach any cleanup in our hook, and profile paths must be current-user
/// paths even though the installer itself is perMachine.
#[test]
fn uninstall_cleanup_is_guarded_against_updates_and_uses_the_current_profile() {
    let nsh = fs::read_to_string(root().join("windows/src-tauri/installer-hooks.nsh")).unwrap();
    let macro_start = nsh.find("!macro NSIS_HOOK_PREUNINSTALL").expect("the pre-uninstall hook");
    let after_macro = &nsh[macro_start..];
    let macro_end = after_macro.find("!macroend").expect("the hook ends");
    let body = &after_macro[..macro_end];

    let guard = body.find("${If} $UpdateMode <> 1").expect("skip cleanup during an in-app update");
    let close = body.rfind("${EndIf}").expect("the guard must close");
    let first_cleanup = body.find("DeleteRegValue").expect("registry cleanup");
    let last_cleanup = body.rfind("RMDir /r").expect("folder cleanup");
    assert!(guard < first_cleanup && guard < last_cleanup, "guard must precede every deletion");
    assert!(close > first_cleanup && close > last_cleanup, "guard must contain every deletion");
    assert!(!body[..guard].contains("DeleteRegValue") && !body[..guard].contains("RMDir /r"));
    assert!(!body[close..].contains("DeleteRegValue") && !body[close..].contains("RMDir /r"));

    let per_user = body.find("SetShellVarContext current").expect("delete the current user's profile");
    let first_folder = body.find(r#"RMDir /r "$APPDATA\com.libertydesignstudio.goodnight""#).unwrap();
    let last_folder = body.rfind(r#"RMDir /r "$LOCALAPPDATA\com.libertydesignstudio.goodnight""#).unwrap();
    assert!(per_user > guard && per_user < first_folder, "switch context before deleting folders");
    assert!(!body[..per_user].contains("RMDir /r"));
    assert!(!body.contains("SetShellVarContext all"), "never delete the all-users folder");
    assert!(
        body[last_folder..].contains("!insertmacro SetContext"),
        "restore the perMachine context for the rest of the uninstall"
    );
}

#[test]
fn privacy_disclosure_is_exact_about_caches_and_cleanup() {
    let privacy = fs::read_to_string(root().join("PRIVACY.md")).unwrap();
    for false_claim in [
        "at most **three** files",
        "nothing Good Night wrote remains",
        "Sparkle downloads to the system temporary folder",
    ] {
        assert!(!privacy.contains(false_claim), "PRIVACY.md must not claim {false_claim}");
    }
    for required in [
        "~/Library/Caches/com.libertydesignstudio.goodnight/org.sparkle-project.Sparkle/PersistentDownloads/",
        "10 days",
        "WebView2",
        "An in-app update does not remove them",
        "Good Night-<version>-installer.exe",
        "`UPDATER_LEFTOVER_LIFETIME_SECS` (1 hour)",
    ] {
        assert!(privacy.contains(required), "PRIVACY.md must state {required}");
    }
    let readme = fs::read_to_string(root().join("README.md")).unwrap();
    let start = readme.find("\n## Privacy\n").expect("the Privacy section");
    let body = &readme[start + 1..];
    let section = &body[..body.find("\n## ").expect("the next section")];
    let lines: Vec<&str> = section.lines().filter(|l| !l.trim().is_empty()).collect();
    assert!((4..=7).contains(&lines.len()), "README Privacy stays 3-6 lines");
}
