//! Privacy regressions: no analytics/crash-reporting packages or endpoints in shipped sources,
//! private settings files on Windows, one anonymous daily update check by default and gated by
//! its opt-outs, bounded local files whose lifetimes match PRIVACY.md, an uninstaller that cleans
//! up, and a short README Privacy section whose details live in PRIVACY.md. Update-check
//! behaviour itself is tested in the shells (`updates.rs`, UpdateKit).

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

fn read(path: &str) -> String {
    fs::read_to_string(root().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
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
fn windows_settings_are_written_owner_only() {
    let main = read("windows/src-tauri/src/main.rs");
    let body = &main[main.find("fn write_settings(").expect("one private settings writer")..];
    let body = &body[..body.find("\n}\n").unwrap()];
    assert!(body.contains("from_mode(0o700)"), "settings folder 0700 on Unix");
    assert!(body.contains("mode(&mut options, 0o600)"), "settings file created 0600 on Unix");
    assert!(main.contains("write_settings(&self.path, s)"), "State::save uses the private writer");
    assert!(!main.contains("std::fs::write(&self.path"), "no world-readable settings write");
}

#[test]
fn readme_privacy_is_short_and_details_live_in_privacy_md() {
    let readme = read("README.md");
    let start = readme.find("\n## Privacy\n").expect("README has a Privacy section") + "\n## Privacy\n".len();
    let section = &readme[start..];
    let section = &section[..section.find("\n## ").unwrap_or(section.len())];
    let lines: Vec<&str> = section.lines().filter(|l| !l.trim().is_empty()).collect();
    assert!(lines.len() <= 6, "README Privacy must stay at most 6 lines, has {}", lines.len());
    let section = section.to_lowercase();
    assert!(section.contains("no telemetry"));
    assert!(section.contains("one anonymous request a day"), "the update check, not 'no network'");
    assert!(section.contains("[privacy.md](privacy.md)"));
    assert!(!section.contains("no update check") && !section.contains("no network"));

    let privacy = read("PRIVACY.md");
    for detail in [
        "no account, no analytics, no telemetry, no crash reporting",
        "install ID or machine fingerprint",
        "GitHub sees your IP address",
        "goodnight-update-check",
        "GOODNIGHT_NO_UPDATE_CHECK=1",
        "~/Library/Preferences/com.libertydesignstudio.goodnight.plist",
        "`settings.json`",
        "no prompts, files, credentials or location",
        "user-only",
        "0600",
        "system fonts",
        "defaults delete com.libertydesignstudio.goodnight",
    ] {
        assert!(privacy.contains(detail), "PRIVACY.md must state: {detail}");
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
    assert_eq!(state["installUpdatesAutomatically"], false);
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
        !plist.contains("<key>SUSendProfileInfo</key>\n\t<true/>"),
        "Sparkle's system profile is off by default and must stay off"
    );
    let kit = fs::read_to_string(root.join("macos/Sources/UpdateKit/UpdateKit.swift")).unwrap();
    assert!(
        kit.contains("public static let userAgent = \"goodnight-update-check\""),
        "the User-Agent must be fixed, with no machine or OS identifiers"
    );
    let app = fs::read_to_string(root.join("macos/Sources/GoodNight/GoodNightApp.swift")).unwrap();
    assert!(app.contains("controller.updater.userAgentString = UpdatePolicy.userAgent"));
    assert!(app.contains("controller.updater.httpHeaders = UpdatePolicy.headers"));
    assert!(
        app.contains("startingUpdater: false"),
        "set the User-Agent before Sparkle's first scheduled check"
    );
    assert!(app.contains("try updater.start()"));
    assert!(
        app.contains("automaticallyChecksForUpdates ?? true"),
        "the Settings toggle must read on when no preference was saved"
    );
    assert!(
        app.contains("!UpdatePolicy.backgroundChecksAllowed(environment: ProcessInfo.processInfo.environment)"),
        "background checks are refused under the env opt-out and CI"
    );
    assert!(kit.contains("!set(\"GOODNIGHT_NO_UPDATE_CHECK\") && !set(\"CI\")"), "env opt-out and CI");
    let generated = fs::read_to_string(root.join("macos/Sources/GoodNight/GoodNightEngine.swift")).unwrap();
    assert!(
        generated.contains("autoUpdate: Bool = true"),
        "generated Swift bindings must match the core's checks-on default"
    );
}

#[test]
fn windows_periodic_check_is_gated_on_setting_opt_out_and_daily_cache() {
    let main = read("windows/src-tauri/src/main.rs");
    let gate = main
        .find("if background && handle.state::<State>().core_state().auto_update {")
        .expect("the Windows timer must check the opt-outs and the user setting before contacting the feed");
    let body = &main[gate..];
    let call = body.find("check(&handle, false).await").expect("timer calls the background check");
    let close = body.find("\n                    }\n").expect("timer gate block closes");
    assert!(call < close, "the background check must sit inside the gated block");
    assert_eq!(main.matches("check(&handle, false).await").count(), 1, "no ungated timer check");
    assert!(main.contains("let background = updates::background_allowed(|k| std::env::var(k).ok());"));
    assert!(main.contains("if !manual && !cache.check_due(at) {"), "at most one background check per day");
    assert!(main.contains("b.timeout(Duration::from_secs(4))"), "short check timeout");
    let updates = read("windows/src-tauri/src/updates.rs");
    assert!(updates.contains("!set(\"GOODNIGHT_NO_UPDATE_CHECK\") && !set(\"CI\")"), "env opt-out and CI");
}

#[test]
fn docs_and_changelog_describe_the_v1_contract() {
    let docs = read("docs/updates.md");
    assert!(docs.contains("<!-- update-contract: v1 -->"));
    assert!(docs.contains("GOODNIGHT_NO_UPDATE_CHECK=1"));
    assert!(docs.contains("**Check for updates automatically**: on by default"), "the opt-out switch");
    for false_claim in ["no update check", "never checks for updates", "no network feature"] {
        assert!(!docs.to_lowercase().contains(false_claim), "docs/updates.md must not claim {false_claim}");
    }
    let readme = read("README.md");
    assert!(readme.contains("[docs/updates.md](docs/updates.md)"));
    assert!(readme.contains("on by default"));
    let changelog = read("CHANGELOG.md");
    let first = changelog.find("\n## [").expect("a release section");
    assert!(changelog[first..].starts_with("\n## [0.5.3] - 2026-11-28\n"), "one new entry, at the top");
    let second = changelog[first + 1..].find("\n## [").map(|k| first + 1 + k).expect("earlier releases");
    assert!(
        changelog[second..].starts_with("\n## [0.5.2] - 2026-11-27\n"),
        "releases stay in order, newest first"
    );
    assert_eq!(changelog.matches("## [0.5.3]").count(), 1);
}

#[test]
fn download_links_name_the_release_being_published() {
    let readme = read("README.md");
    assert!(readme.contains("releases/latest/download/GoodNight-0.5.3-mac.dmg"));
    assert!(readme.contains("releases/latest/download/GoodNight-0.5.3-windows-x64-setup.exe"));
    for stale in ["GoodNight-0.5.0-", "GoodNight-0.2.0-", "releases/download/v0.", "unpublished", "not downloadable"] {
        assert!(!readme.contains(stale), "README must not carry {stale}");
    }
    let docs = read("docs/updates.md");
    assert!(docs.contains("**Install updates automatically**: off by default"), "the Windows UI keeps the toggle");
    assert!(docs.contains("**Remind Me Later**") && docs.contains("**Skip This Version**"), "the prompt keeps both choices");
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
    for (path, needle) in [
        ("core/Cargo.toml", "\nversion = \"0.5.3\"\n"),
        ("core/Cargo.lock", "name = \"goodnight-core\"\nversion = \"0.5.3\"\n"),
        ("windows/src-tauri/Cargo.toml", "\nversion = \"0.5.3\"\n"),
        ("windows/src-tauri/Cargo.lock", "name = \"goodnight\"\nversion = \"0.5.3\"\n"),
        ("windows/src-tauri/Cargo.lock", "name = \"goodnight-core\"\nversion = \"0.5.3\"\n"),
        ("windows/src-tauri/tauri.conf.json", "\"version\": \"0.5.3\""),
        ("macos/Resources/Info.plist", "<key>CFBundleShortVersionString</key>\n\t<string>0.5.3</string>"),
    ] {
        assert!(read(path).contains(needle), "{path} must carry 0.5.3");
    }
    let plist = read("macos/Resources/Info.plist");
    assert!(
        plist.contains("<key>CFBundleVersion</key>\n\t<string>8</string>"),
        "the macOS build number must stay above published v0.5.0 (build 5)"
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
    for detail in ["updates.json", "updates.json.tmp", "~/Library/Preferences", "GdiIcmGammaRange"] {
        assert!(!section.contains(detail), "{detail} is detail for PRIVACY.md, not the README summary");
    }
}

#[test]
fn every_local_write_is_inventoried_with_a_lifetime() {
    let privacy = read("PRIVACY.md");
    for required in [
        "Lifetime",
        // settings, both platforms
        "~/Library/Preferences/com.libertydesignstudio.goodnight.plist",
        r"%APPDATA%\com.libertydesignstudio.goodnight\settings.json",
        // the Windows update cache, its atomic-write temp file and the bounded log
        r"%APPDATA%\com.libertydesignstudio.goodnight\updates.json",
        "updates.json.tmp",
        r"%APPDATA%\com.libertydesignstudio.goodnight\updates.log",
        "1 hour",
        "30 days",
        "256 KB",
        // install leftovers and the way out
        "GdiIcmGammaRange",
        "Start at login",
        "defaults delete com.libertydesignstudio.goodnight",
        "uninstall",
    ] {
        assert!(privacy.contains(required), "PRIVACY.md must document {required}");
    }
    for never_written in ["update-check.json", "settings.json.tmp", "settingsVersion", "no Windows log file"] {
        assert!(!privacy.contains(never_written), "PRIVACY.md must not list {never_written}: the app never writes it");
    }
    assert!(read("README.md").contains("[PRIVACY.md](PRIVACY.md)"), "the README must point at it");
    assert!(read("docs/updates.md").contains("[PRIVACY.md](../PRIVACY.md)"), "the update docs must point at it too");
}

#[test]
fn retention_bounds_are_pure_documented_and_applied_at_launch() {
    use goodnight_core::retention::{
        disposition, Disposition, CHECK_RECORD_LIFETIME_SECS, SETTINGS_FILE_NAME, TEMP_LIFETIME_SECS,
        UPDATER_PRODUCT_NAME,
    };
    let now = 1_800_000_000u64;
    assert_eq!(TEMP_LIFETIME_SECS, 3_600, "PRIVACY.md promises one hour");
    assert_eq!(CHECK_RECORD_LIFETIME_SECS, 30 * 24 * 3_600, "PRIVACY.md promises 30 days");
    assert_eq!(disposition("updates.json.tmp", now - TEMP_LIFETIME_SECS, now), Disposition::Remove);
    assert_eq!(disposition("updates.json.tmp", now - 1, now), Disposition::Keep, "a live write");
    assert_eq!(disposition("updates.json", now - CHECK_RECORD_LIFETIME_SECS, now), Disposition::Remove);
    assert_eq!(disposition("updates.json", now - 24 * 3_600, now), Disposition::Keep, "a daily check keeps it");
    assert_eq!(disposition("updates.log", 0, now), Disposition::Keep, "the log is bounded by size");
    assert_eq!(
        disposition(SETTINGS_FILE_NAME, now - 10 * CHECK_RECORD_LIFETIME_SECS, now),
        Disposition::Keep,
        "settings are the user's data, not debris"
    );
    assert_eq!(disposition("installer-notes.txt", 0, now), Disposition::Keep, "only this app's files");

    // The files the policy names are the ones the Windows shell really writes.
    let updates = read("windows/src-tauri/src/updates.rs");
    assert!(updates.contains("path.with_extension(\"json.tmp\")"), "Cache::save writes updates.json.tmp");
    let config = read("windows/src-tauri/tauri.conf.json");
    assert!(
        config.contains(&format!("\"productName\": \"{UPDATER_PRODUCT_NAME}\"")),
        "the %TEMP% sweep recognises the updater's names only if they use the real product name"
    );

    let privacy = read("PRIVACY.md");
    assert!(
        privacy.contains("core/src/retention.rs") && privacy.contains("windows/src-tauri/src/housekeeping.rs"),
        "PRIVACY.md must name the code that enforces the bounds"
    );

    let main = read("windows/src-tauri/src/main.rs");
    assert!(main.contains("mod housekeeping;"), "the shell must own the sweep");
    assert_eq!(main.matches("housekeeping::sweep(").count(), 1, "one sweep, at startup");
    let sweep = main.find("housekeeping::sweep(&dir)").expect("the config folder is swept");
    let cache = main.find("updates::Cache::load(").expect("the update cache is read at startup");
    let read_settings = main.find("std::fs::read(&path)").expect("settings are read at startup");
    assert!(sweep < cache && sweep < read_settings, "debris is cleared before anything is read");

    assert_eq!(
        main.matches("updates::remove_staged_installers(&std::env::temp_dir(),").count(),
        1,
        "the updater's abandoned installer in %TEMP% is swept once per launch"
    );
    let staged = main.find("updates::remove_staged_installers(").unwrap();
    let delay = main.find("tokio_sleep(Duration::from_secs(60)).await;").expect("the background task waits a minute");
    let checks = main.find("if background && handle.state::<State>()").unwrap();
    assert!(delay < staged && staged < checks, "a minute after launch, before the first check");
    assert!(
        !read("windows/src-tauri/src/housekeeping.rs").contains("temp_dir"),
        "one %TEMP% sweep, not two"
    );

    let housekeeping = read("windows/src-tauri/src/housekeeping.rs");
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
    let staged_fn = &updates[updates.find("pub fn remove_staged_installers(").unwrap()..];
    let staged_fn = &staged_fn[..staged_fn.find("\n}\n").unwrap()];
    assert!(!staged_fn.contains("remove_dir_all("), "the %TEMP% sweep never removes a folder with other files");
    assert!(staged_fn.contains("std::fs::remove_dir(entry.path())"), "only an emptied folder goes");
    assert!(staged_fn.contains("is_updater_staging_dir") && staged_fn.contains("is_updater_installer"));
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

/// Tauri's running-app check runs after our hook and stops only `goodnight.exe`, so a WebView2
/// browser process keeps writing into the profile folders while they are removed. The hook must
/// stop the whole process tree first, then wait for the folders to come free (J-4813, J-4889).
#[test]
fn uninstalling_while_running_stops_good_night_before_removing_its_data() {
    let nsh = fs::read_to_string(root().join("windows/src-tauri/installer-hooks.nsh")).unwrap();
    let macro_start = nsh.find("!macro NSIS_HOOK_PREUNINSTALL").expect("the pre-uninstall hook");
    let after_macro = &nsh[macro_start..];
    let body = &after_macro[..after_macro.find("!macroend").expect("the hook ends")];

    let stop = body.find("taskkill.exe").expect("the hook must stop Good Night first");
    let line = &body[stop..];
    assert!(
        line[..line.find('\n').unwrap_or(line.len())].contains("/T"),
        "stop the WebView2 children too"
    );
    assert!(line.contains("${MAINBINARYNAME}.exe"), "stop the app the installer installed");
    for deletion in [
        body.find(r#"RMDir /r "$APPDATA\com.libertydesignstudio.goodnight""#).unwrap(),
        body.find(r#"RMDir /r "$LOCALAPPDATA\com.libertydesignstudio.goodnight""#).unwrap(),
    ] {
        assert!(stop < deletion, "stopping the app comes before removing its data");
    }
    assert!(body.contains("goodnight_data_retry"), "wait for a browser process that is still exiting");
    assert!(body.contains("Sleep 250"), "the wait is bounded");
    assert!(body.contains("RMDir /r /REBOOTOK"), "a folder still held is removed at the next start");
}

#[test]
fn privacy_disclosure_is_exact_about_caches_and_cleanup() {
    let privacy = read("PRIVACY.md");
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
        "`remove_staged_installers`",
        "a minute after the next launch",
        "256 KB",
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
