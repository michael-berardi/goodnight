//! Local debris housekeeping.
//!
//! Good Night writes exactly three files on Windows: `settings.json` in the app config folder,
//! `update-check.json` in the app cache folder, and a one-line `.tmp` next to either while an
//! atomic write is in flight. The retention rules live in `goodnight_core::retention` (pure and
//! tested there); this module is the only code that deletes anything, it deletes files only, and
//! it never touches the update-check logic: an expired record is simply a record whose lifetime
//! ran out, and a missing record only means the next check is due.
//!
//! Lifetimes are documented in `PRIVACY.md`.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use goodnight_core::retention::{self, Disposition};

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn modified_secs(path: &Path) -> Option<u64> {
    std::fs::metadata(path).ok()?.modified().ok()?.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())
}

/// Remove this app's own expired files from one folder, as of `now`. Files the app did not write,
/// folders and anything still inside its lifetime are left alone. Returns what it removed.
pub fn sweep_at(dir: &Path, now: u64) -> Vec<PathBuf> {
    let mut removed = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return removed };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else { continue };
        if !kind.is_file() {
            continue;
        }
        let path = entry.path();
        let Some(modified) = modified_secs(&path) else { continue };
        if retention::disposition(&entry.file_name().to_string_lossy(), modified, now) == Disposition::Remove
            && std::fs::remove_file(&path).is_ok()
        {
            removed.push(path);
        }
    }
    removed
}

/// Sweep one folder against the current clock. Called once at startup, off the settings and
/// check paths, so a crash that left a file behind cannot leave it behind for good.
pub fn sweep(dir: &Path) -> Vec<PathBuf> {
    sweep_at(dir, now_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A private scratch folder under the crate's (gitignored) target directory.
    fn scratch(name: &str) -> PathBuf {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target").join("test-housekeeping").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn a_fresh_install_is_left_untouched() {
        let dir = scratch("fresh");
        let settings = write(&dir, retention::SETTINGS_FILE_NAME, "{}");
        let record = write(&dir, retention::CHECK_RECORD_FILE_NAME, "{}");
        let temp = write(&dir, retention::TEMP_NAMES[1], "{}");
        let removed = sweep(&dir);
        assert!(removed.is_empty(), "nothing written a moment ago is debris");
        for path in [settings, record, temp] {
            assert!(path.exists(), "{} must survive", path.display());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_interrupted_write_is_removed_but_settings_are_kept() {
        let dir = scratch("interrupted");
        let settings = write(&dir, retention::SETTINGS_FILE_NAME, r#"{"enabled":true}"#);
        let record = write(&dir, retention::CHECK_RECORD_FILE_NAME, r#"{"lastCheckedAt":1}"#);
        let temp = write(&dir, retention::TEMP_NAMES[1], "{\"lastChe");
        let now = now_secs() + retention::TEMP_LIFETIME_SECS + 1;
        let removed = sweep_at(&dir, now);
        assert_eq!(removed, vec![temp.clone()], "only the abandoned temp file goes");
        assert!(!temp.exists());
        assert!(settings.exists(), "the user's settings are not debris");
        assert!(record.exists(), "a day-old check record is still within its month");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_expired_check_record_is_removed_and_a_fresh_one_kept() {
        let dir = scratch("record");
        let settings = write(&dir, retention::SETTINGS_FILE_NAME, "{}");
        let record = write(&dir, retention::CHECK_RECORD_FILE_NAME, r#"{"lastCheckedAt":1,"failure":null}"#);
        let after_a_week = sweep_at(&dir, now_secs() + 7 * 24 * 3600);
        assert!(after_a_week.is_empty(), "a weekly record is inside its lifetime");
        assert!(record.exists());
        let after_a_month = sweep_at(&dir, now_secs() + retention::CHECK_RECORD_LIFETIME_SECS + 1);
        assert_eq!(after_a_month, vec![record.clone()]);
        assert!(!record.exists());
        assert!(settings.exists(), "settings never expire");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn foreign_files_and_folders_are_never_touched() {
        let dir = scratch("foreign");
        let foreign = write(&dir, "installer-notes.txt", "keep");
        let lookalike = write(&dir, "settings.json.bak", "keep");
        let nested = dir.join("logs");
        std::fs::create_dir_all(&nested).unwrap();
        let removed = sweep_at(&dir, u64::MAX / 2);
        assert!(removed.is_empty(), "the sweeper only removes the app's own temp and cache files");
        for path in [foreign, lookalike, nested] {
            assert!(path.exists(), "{} must survive", path.display());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_folder_is_not_an_error() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target").join("test-housekeeping").join("absent");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(sweep(&dir).is_empty(), "a first launch with no folders yet must not fail");
    }
}
