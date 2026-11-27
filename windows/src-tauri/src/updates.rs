//! Update rules for the Windows shell: stable versions only, the daily check cache, skip and
//! reminder throttling, the opt-outs, the idle rule and the bounded log. No network lives
//! here: the signed request and the verified install belong to `tauri-plugin-updater`.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Background checks and reminders for one version happen at most once a day.
pub const DAY: f64 = 24.0 * 3600.0;
/// The only identifying header of an update request.
pub const USER_AGENT: &str = "goodnight-update-check";
/// The background update log is one file, capped at this size.
pub const MAX_LOG_BYTES: usize = 256 * 1024;
/// An opt-in install waits until the user has been away from keyboard and mouse this long.
pub const IDLE_SECS: u64 = 10 * 60;

/// `updates.json`, next to settings.json. The two switches live in settings.json; this file
/// only remembers what the checks found.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Cache {
    /// Unix seconds of the last check, successful or not.
    pub last_check: Option<f64>,
    /// Cause of the last failed check or automatic install, shown in Settings.
    pub last_error: Option<String>,
    /// "Skip This Version": no background prompt for it until a newer release appears.
    pub skipped: Option<String>,
    /// The version last prompted for, and when.
    pub shown_version: Option<String>,
    pub shown_at: Option<f64>,
}

impl Cache {
    pub fn load(path: &Path) -> Self {
        std::fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    /// Written to a sibling and renamed over the old file, so a crash never leaves half a file.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        let mut file = std::fs::OpenOptions::new();
        file.write(true).create(true).truncate(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut file, 0o600);
        std::io::Write::write_all(&mut file.open(&tmp)?, &serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(&tmp, path)
    }

    /// A background check runs once a day, also after a failure: never in a loop.
    pub fn check_due(&self, at: f64) -> bool {
        self.last_check.is_none_or(|t| at - t >= DAY || at < t)
    }

    /// Whether a found release opens the prompt. "Check Now" always shows a newer release;
    /// a background check stays quiet for a skipped version and repeats one at most daily.
    pub fn should_prompt(&self, version: &str, current: &str, at: f64, manual: bool) -> bool {
        if !is_newer(version, current) {
            return false;
        }
        if manual {
            return true;
        }
        let skipped = self.skipped.as_deref() == Some(version);
        let shown_today = self.shown_version.as_deref() == Some(version) && self.shown_at.is_some_and(|t| at - t < DAY);
        !skipped && !shown_today
    }
}

pub fn now() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

/// Stable `x.y.z` only. Prereleases, build tags and anything unparseable are never newer.
pub fn parse_version(s: &str) -> Option<(u64, u64, u64)> {
    let mut parts = s.trim().split('.').map(|p| if !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()) { p.parse().ok() } else { None });
    let v = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(v)
}

pub fn is_newer(candidate: &str, current: &str) -> bool {
    matches!((parse_version(candidate), parse_version(current)), (Some(a), Some(b)) if a > b)
}

pub fn notice_text(app: &str, new: &str, current: &str) -> String {
    format!("{app} {new} is available — you have {current}.")
}

/// Background checks are off under CI or with `GOODNIGHT_NO_UPDATE_CHECK=1`; "Check Now" still works.
pub fn background_allowed(var: impl Fn(&str) -> Option<String>) -> bool {
    let set = |key| var(key).is_some_and(|v| !v.is_empty() && v != "0" && !v.eq_ignore_ascii_case("false"));
    !set("GOODNIGHT_NO_UPDATE_CHECK") && !set("CI")
}

/// An opt-in install never interrupts: no Good Night window is open and the user is away.
pub fn idle_enough(idle_secs: Option<u64>, window_open: bool) -> bool {
    !window_open && idle_secs.is_some_and(|s| s >= IDLE_SECS)
}

/// Appends one line, keeping the newest whole lines within `MAX_LOG_BYTES`.
pub fn append_log(path: &Path, at: f64, line: &str) -> std::io::Result<()> {
    let line: String = line.chars().take(500).collect();
    let mut bytes = std::fs::read(path).unwrap_or_default();
    bytes.extend_from_slice(format!("{at:.0} {line}\n").as_bytes());
    if bytes.len() > MAX_LOG_BYTES {
        let cut = bytes.len() - MAX_LOG_BYTES;
        let start = bytes[cut..].iter().position(|b| *b == b'\n').map_or(bytes.len(), |i| cut + i + 1);
        bytes.drain(..start);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, bytes)
}

/// The updater (tauri-plugin-updater 2.x) stages each installer as
/// `<temp>\<app>-<version>-updater-<random>\<app>-<version>-installer.exe` and runs it after this
/// process exits, so that run cannot delete it. The background task calls this a minute after
/// the next launch: it deletes that installer, then the folder if nothing else is in it, and
/// returns how many folders it removed. Only this app's own names are opened (the strict
/// patterns in `goodnight_core::retention`); another app's folder, or a file of ours sharing a
/// folder with anything else, keeps the folder. An installer still running is locked by
/// Windows and simply stays until a later launch. This is the app's only %TEMP% sweep (the
/// uninstaller has its own); the startup housekeeping handles the app's own config folder.
pub fn remove_staged_installers(temp: &Path, app: &str) -> usize {
    use goodnight_core::retention::{is_updater_installer, is_updater_staging_dir};
    let prefix = format!("{app}-");
    let Ok(entries) = std::fs::read_dir(temp) else { return 0 };
    let mut folders = 0;
    for entry in entries.flatten() {
        // Names first: %TEMP% can be large, and a name check costs no file system call.
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with(&prefix) || !is_updater_staging_dir(&name) {
            continue;
        }
        if !entry.file_type().is_ok_and(|k| k.is_dir()) {
            continue;
        }
        if let Ok(inner) = std::fs::read_dir(entry.path()) {
            for item in inner.flatten() {
                let is_file = item.file_type().is_ok_and(|k| k.is_file());
                if is_file && is_updater_installer(&item.file_name().to_string_lossy()) {
                    let _ = std::fs::remove_file(item.path());
                }
            }
        }
        // remove_dir (not remove_dir_all) refuses a folder that still holds anything else.
        if std::fs::remove_dir(entry.path()).is_ok() {
            folders += 1;
        }
    }
    folders
}

/// Turning update checks off removes every update file; the uninstaller does the same.
pub fn clear(dir: &Path) {
    for name in ["updates.json", "updates.json.tmp", "updates.log"] {
        let _ = std::fs::remove_file(dir.join(name));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AT: f64 = 1_800_000_000.0;

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("goodnight-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn only_strictly_newer_stable_versions_count() {
        assert_eq!(parse_version("0.3.0"), Some((0, 3, 0)));
        assert_eq!(parse_version(" 1.12.3 "), Some((1, 12, 3)));
        for bad in ["v0.3.0", "0.3", "0.3.0-beta.1", "0.3.0+build", "0.3.x", "", "1.2.3.4", "newest"] {
            assert_eq!(parse_version(bad), None, "{bad}");
        }
        assert!(is_newer("0.3.0", "0.2.0"));
        assert!(is_newer("0.2.1", "0.2.0"));
        assert!(is_newer("0.10.0", "0.9.9"));
        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(!is_newer("0.1.9", "0.2.0"));
        assert!(!is_newer("0.3.0-rc.1", "0.2.0"));
        assert!(!is_newer("garbage", "0.2.0"));
        assert!(!is_newer("0.3.0", "garbage"));
    }

    #[test]
    fn background_checks_run_once_a_day_even_after_a_failure() {
        let mut cache = Cache::default();
        assert!(cache.check_due(AT));
        cache.last_check = Some(AT);
        cache.last_error = Some("Couldn't check for updates: timed out".into());
        assert!(!cache.check_due(AT + 60.0));
        assert!(!cache.check_due(AT + DAY - 1.0));
        assert!(cache.check_due(AT + DAY));
        assert!(cache.check_due(AT - 60.0), "a clock set back does not block checks for a day");
    }

    #[test]
    fn skip_hides_a_version_until_a_newer_one_and_reminders_are_daily() {
        let mut cache = Cache::default();
        assert!(cache.should_prompt("0.3.0", "0.2.0", AT, false));
        assert!(!cache.should_prompt("0.2.0", "0.2.0", AT, false));
        cache.skipped = Some("0.3.0".into());
        assert!(!cache.should_prompt("0.3.0", "0.2.0", AT, false));
        assert!(cache.should_prompt("0.3.1", "0.2.0", AT, false));
        assert!(cache.should_prompt("0.3.0", "0.2.0", AT, true), "Check Now shows a skipped version");
        cache.skipped = None;
        cache.shown_version = Some("0.3.0".into());
        cache.shown_at = Some(AT);
        assert!(!cache.should_prompt("0.3.0", "0.2.0", AT + 3600.0, false));
        assert!(cache.should_prompt("0.3.0", "0.2.0", AT + DAY, false));
        assert!(cache.should_prompt("0.3.1", "0.2.0", AT + 3600.0, false));
    }

    #[test]
    fn cache_round_trips_atomically_with_private_permissions() {
        let dir = scratch("cache");
        let path = dir.join("updates.json");
        assert_eq!(Cache::load(&path), Cache::default());
        let cache = Cache { last_check: Some(AT), last_error: Some("offline".into()), skipped: Some("0.3.0".into()), ..Cache::default() };
        cache.save(&path).unwrap();
        assert_eq!(Cache::load(&path), cache);
        assert!(!dir.join("updates.json.tmp").exists());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "one cache file and nothing else");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        }
        std::fs::write(&path, b"not json").unwrap();
        assert_eq!(Cache::load(&path), Cache::default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ci_and_the_env_opt_out_silence_background_checks() {
        let env = |pairs: &'static [(&'static str, &'static str)]| move |k: &str| pairs.iter().find(|p| p.0 == k).map(|p| p.1.to_string());
        assert!(background_allowed(env(&[])));
        assert!(!background_allowed(env(&[("CI", "true")])));
        assert!(!background_allowed(env(&[("GOODNIGHT_NO_UPDATE_CHECK", "1")])));
        assert!(background_allowed(env(&[("GOODNIGHT_NO_UPDATE_CHECK", "0"), ("CI", "false")])));
    }

    #[test]
    fn automatic_installs_wait_for_an_idle_user_and_no_open_window() {
        assert!(idle_enough(Some(IDLE_SECS), false));
        assert!(!idle_enough(Some(IDLE_SECS - 1), false));
        assert!(!idle_enough(Some(IDLE_SECS * 6), true));
        assert!(!idle_enough(None, false), "unknown idle time never installs");
    }

    #[test]
    fn notice_names_both_versions() {
        assert_eq!(notice_text("Good Night", "0.3.0", "0.2.0"), "Good Night 0.3.0 is available — you have 0.2.0.");
    }

    #[test]
    fn the_log_stays_under_256_kb_and_keeps_the_newest_line() {
        let dir = scratch("log");
        let log = dir.join("updates.log");
        let long = "x".repeat(450);
        for i in 0..2000 {
            append_log(&log, AT, &format!("event {i} {long}")).unwrap();
            assert!(std::fs::metadata(&log).unwrap().len() <= MAX_LOG_BYTES as u64);
        }
        let text = std::fs::read_to_string(&log).unwrap();
        assert!(text.ends_with(&format!("event 1999 {long}\n")));
        assert!(!text.contains("event 0 "));
        assert!(text.lines().all(|l| l.starts_with("1800000000 event ")), "only whole lines survive");
        append_log(&log, AT, &"y".repeat(MAX_LOG_BYTES * 2)).unwrap();
        assert!(std::fs::metadata(&log).unwrap().len() <= MAX_LOG_BYTES as u64);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn staged_installers_from_an_earlier_run_are_removed() {
        let temp = scratch("staged");
        let staged = temp.join("Good Night-0.3.0-updater-Ab12Cd");
        std::fs::create_dir(&staged).unwrap();
        std::fs::write(staged.join("Good Night-0.3.0-installer.exe"), b"MZ").unwrap();
        std::fs::create_dir(temp.join("Other App-1.0.0-updater-Zz")).unwrap();
        std::fs::write(temp.join("Good Night-notes.txt"), b"not ours").unwrap();
        assert_eq!(remove_staged_installers(&temp, "Good Night"), 1);
        assert!(!staged.exists());
        assert!(temp.join("Other App-1.0.0-updater-Zz").exists());
        assert!(temp.join("Good Night-notes.txt").exists());
        assert_eq!(remove_staged_installers(&temp, "Good Night"), 0);
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn abandoned_installers_leave_foreign_files_and_folders_alone() {
        let temp = scratch("staged-strict");
        let ours = temp.join("Good Night-0.5.2-updater-Ab12cd");
        std::fs::create_dir_all(&ours).unwrap();
        std::fs::write(ours.join("Good Night-0.5.2-installer.exe"), b"MZ").unwrap();
        let shared = temp.join("Good Night-0.5.1-updater-Zz9");
        std::fs::create_dir_all(&shared).unwrap();
        let old_installer = shared.join("Good Night-0.5.1-installer.exe");
        std::fs::write(&old_installer, b"MZ").unwrap();
        let stranger = shared.join("keep.txt");
        std::fs::write(&stranger, b"not ours").unwrap();
        let other = temp.join("Other App-1.0.0-updater-Zz");
        std::fs::create_dir_all(&other).unwrap();
        let other_installer = other.join("Other App-1.0.0-installer.exe");
        std::fs::write(&other_installer, b"MZ").unwrap();

        assert_eq!(remove_staged_installers(&temp, "Good Night"), 1);
        assert!(!ours.exists(), "an emptied staging folder goes too");
        assert!(stranger.exists() && shared.exists(), "a folder holding anything else is kept");
        assert!(!old_installer.exists(), "our installer in that folder is still removed");
        assert!(other_installer.exists(), "another app's updater files are never touched");
        assert_eq!(remove_staged_installers(&temp, "Good Night"), 0);
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn turning_checks_off_removes_all_update_files_and_nothing_else() {
        let dir = scratch("clear");
        for name in ["updates.json", "updates.json.tmp", "updates.log", "settings.json"] {
            std::fs::write(dir.join(name), b"{}").unwrap();
        }
        clear(&dir);
        let left: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(left, ["settings.json"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
