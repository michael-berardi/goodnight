//! Privacy regressions: no analytics/crash-reporting packages or endpoints in shipped sources,
//! and no automatic update request in either platform's default configuration.

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
fn default_config_makes_no_update_request() {
    assert!(!Settings::default().auto_update, "engine default must be offline");
    let config = Config {
        time_zone: "UTC".into(),
        fixed_now: Some(0.0),
        ..Config::default()
    };
    assert!(!config.settings.auto_update);
    let (engine, _) = Engine::<GoodNight>::start(config);
    let state: serde_json::Value = serde_json::from_str(engine.snapshot()).unwrap();
    assert_eq!(state["autoUpdate"], false);
    // The engine performs no I/O; both shells gate their updater on this flag.
}

#[test]
fn macos_automatic_checks_default_to_off_in_shell_and_info_plist() {
    let root = root();
    let plist = fs::read_to_string(root.join("macos/Resources/Info.plist")).unwrap();
    assert!(
        plist.contains("<key>SUEnableAutomaticChecks</key>\n\t<false/>"),
        "Sparkle must not schedule checks on launch"
    );
    let app = fs::read_to_string(root.join("macos/Sources/GoodNight/GoodNightApp.swift")).unwrap();
    assert!(
        app.contains("automaticallyChecksForUpdates ?? false"),
        "the Settings toggle must read off when no preference was saved"
    );
}

#[test]
fn windows_periodic_check_is_gated_on_the_user_setting() {
    let main = fs::read_to_string(root().join("windows/src-tauri/src/main.rs")).unwrap();
    assert!(
        main.contains("core_state().auto_update"),
        "the Windows timer must check the user setting before contacting the feed"
    );
}
