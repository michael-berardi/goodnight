//! Privacy regressions: no analytics/crash-reporting packages or endpoints in shipped sources,
//! private settings files on Windows, and a short README Privacy section whose details live in
//! PRIVACY.md. Update-check behaviour is tested in the shells (`updates.rs`, UpdateKit).

use std::fs;
use std::path::{Path, PathBuf};

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
            assert!(!text.contains(token), "{token} reintroduced in {}", file.display());
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
