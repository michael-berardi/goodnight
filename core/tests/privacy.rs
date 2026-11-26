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
    let app = fs::read_to_string(root.join("macos/Sources/GoodNight/GoodNightApp.swift")).unwrap();
    assert!(
        app.contains("automaticallyChecksForUpdates ?? true"),
        "the Settings toggle must read on when no preference was saved"
    );
    assert!(app.contains("GOODNIGHT_NO_UPDATE_CHECK"), "env opt-out");
    assert!(app.contains("env[\"CI\"] != nil"), "CI must disable checks");
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
    assert!(main.contains("checks_allowed(hidden, std::env::var_os(\"CI\").is_some(), env_opt_out())"));
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
    let readme = fs::read_to_string(root.join("README.md")).unwrap();
    assert!(readme.contains("[docs/updates.md](docs/updates.md)"));
    assert!(readme.contains("on by default"));
    let changelog = fs::read_to_string(root.join("CHANGELOG.md")).unwrap();
    assert!(changelog.contains("[0.4.0]"));
}
