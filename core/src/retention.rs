//! Retention bounds for the files the platform shells write to disk.
//!
//! This module is pure policy: the engine still performs no I/O. The shells ask these functions
//! what a file on disk deserves when they start, so nothing Good Night writes can sit past its
//! documented lifetime and stay behind as debris. `PRIVACY.md` is the user-facing version of
//! this list, and its lifetimes must match the constants here.

/// A temporary file is live only for the instant between create and rename. A copy still on
/// disk after an hour belongs to an interrupted write and is removed at the next launch.
pub const TEMP_LIFETIME_SECS: u64 = 60 * 60;

/// The last update check is recorded so the app checks at most once a day. Once checks have
/// stopped for this long (the setting is off, or the app has not been launched), the record is
/// dead weight and is removed. Losing it only means the next check is due, which an old record
/// already allows, so the sweep changes no behaviour.
pub const CHECK_RECORD_LIFETIME_SECS: u64 = 30 * 24 * 60 * 60;

/// The temporary names the shells write before renaming a file into place. Only these are ever
/// swept, so the sweeper can never remove a file Good Night did not write.
pub const TEMP_NAMES: [&str; 3] = ["settings.json.tmp", "update-check.json.tmp", "updates.json.tmp"];

/// The Windows settings file. It holds the user's own choices and never expires.
pub const SETTINGS_FILE_NAME: &str = "settings.json";

/// The Windows update-check record used by earlier privacy builds: last check time plus,
/// if one failed, its cause. A leftover copy is still swept by the merged shell.
pub const CHECK_RECORD_FILE_NAME: &str = "update-check.json";

/// The current Windows update-cache record: last check time, failure cause, skipped version
/// and the version last shown. It has the same 30-day bound as the earlier record name.
pub const UPDATE_CACHE_FILE_NAME: &str = "updates.json";

/// The Windows updater (tauri-plugin-updater 2.x) writes the downloaded installer to
/// `%TEMP%\<product>-<version>-updater-<random>\<product>-<version>-installer.exe`, keeps that
/// folder, then exits the app to run the installer. `updates.rs` removes that installer and its
/// empty folder at the next launch; this module only recognises which names are ours.

/// The product name the updater uses in those temporary names (`productName` in tauri.conf.json).
pub const UPDATER_PRODUCT_NAME: &str = "Good Night";

/// Is `dir_name` an updater staging folder this app created (`Good Night-<version>-updater-<random>`)?
pub fn is_updater_staging_dir(dir_name: &str) -> bool {
    let Some(rest) = dir_name.strip_prefix(UPDATER_PRODUCT_NAME).and_then(|r| r.strip_prefix('-')) else {
        return false;
    };
    let Some((version, random)) = rest.split_once("-updater-") else { return false };
    is_version(version) && !random.is_empty() && random.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Is `file_name` the installer the updater wrote into such a folder (`Good Night-<version>-installer.exe`)?
pub fn is_updater_installer(file_name: &str) -> bool {
    file_name
        .strip_prefix(UPDATER_PRODUCT_NAME)
        .and_then(|r| r.strip_prefix('-'))
        .and_then(|r| r.strip_suffix("-installer.exe"))
        .is_some_and(is_version)
}

fn is_version(v: &str) -> bool {
    !v.is_empty()
        && v.split('.').count() >= 2
        && v.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '+')
        && v.split('.').next().is_some_and(|major| major.chars().all(|c| c.is_ascii_digit()) && !major.is_empty())
}

/// What a file on disk deserves the next time the app starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    /// User data, or still inside its lifetime: leave it alone.
    Keep,
    /// Past its documented lifetime: remove it.
    Remove,
}

/// Decide what happens to `name`, last written at `modified_secs`, when the clock reads
/// `now_secs`. Unknown names are always kept: the sweeper holds no opinion about other files.
pub fn disposition(name: &str, modified_secs: u64, now_secs: u64) -> Disposition {
    let age = now_secs.saturating_sub(modified_secs);
    let past = |lifetime: u64| if age >= lifetime { Disposition::Remove } else { Disposition::Keep };
    if TEMP_NAMES.contains(&name) {
        past(TEMP_LIFETIME_SECS)
    } else if name == CHECK_RECORD_FILE_NAME || name == UPDATE_CACHE_FILE_NAME {
        past(CHECK_RECORD_LIFETIME_SECS)
    } else {
        // Settings last until the user deletes them or uninstalls the app; everything else is
        // not ours to touch.
        Disposition::Keep
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_800_000_000;

    #[test]
    fn an_interrupted_write_is_swept_after_an_hour() {
        for name in TEMP_NAMES {
            assert_eq!(disposition(name, NOW - 5, NOW), Disposition::Keep, "{name} may be mid-write");
            assert_eq!(
                disposition(name, NOW - TEMP_LIFETIME_SECS + 1, NOW),
                Disposition::Keep,
                "{name} is still inside its hour"
            );
            assert_eq!(
                disposition(name, NOW - TEMP_LIFETIME_SECS, NOW),
                Disposition::Remove,
                "{name} must not survive its lifetime"
            );
            assert_eq!(disposition(name, NOW - 400 * 24 * 3600, NOW), Disposition::Remove);
        }
    }

    #[test]
    fn the_check_record_is_swept_once_checks_have_been_off_for_a_month() {
        let name = CHECK_RECORD_FILE_NAME;
        assert_eq!(disposition(name, NOW - 24 * 3600, NOW), Disposition::Keep, "a daily record stays");
        assert_eq!(
            disposition(name, NOW - CHECK_RECORD_LIFETIME_SECS + 1, NOW),
            Disposition::Keep,
            "inside its month"
        );
        assert_eq!(
            disposition(name, NOW - CHECK_RECORD_LIFETIME_SECS, NOW),
            Disposition::Remove,
            "a month-old record is debris"
        );
    }

    #[test]
    fn settings_and_foreign_files_are_never_swept() {
        for name in [SETTINGS_FILE_NAME, "notes.json", "uninstall.exe", "settings.json.bak"] {
            assert_eq!(disposition(name, 0, NOW), Disposition::Keep, "{name} is not debris");
        }
    }

    #[test]
    fn the_windows_update_cache_has_the_same_bound_as_the_earlier_record() {
        let name = UPDATE_CACHE_FILE_NAME;
        assert_eq!(disposition(name, NOW - 24 * 3600, NOW), Disposition::Keep);
        assert_eq!(disposition(name, NOW - CHECK_RECORD_LIFETIME_SECS, NOW), Disposition::Remove);
        assert_eq!(disposition("updates.json.tmp", NOW - TEMP_LIFETIME_SECS, NOW), Disposition::Remove);
    }

    #[test]
    fn a_file_timestamped_in_the_future_is_not_debris() {
        // Clocks move backwards across updates and time zones; saturate instead of expiring.
        assert_eq!(disposition(TEMP_NAMES[0], NOW + 10_000, NOW), Disposition::Keep);
        assert_eq!(disposition(CHECK_RECORD_FILE_NAME, NOW + 10_000, NOW), Disposition::Keep);
    }

    #[test]
    fn only_this_apps_updater_leftovers_are_recognised() {
        assert!(is_updater_staging_dir("Good Night-0.5.2-updater-Ab12cd"));
        assert!(is_updater_installer("Good Night-0.5.2-installer.exe"));
        for other in [
            "Good Night-updater-Ab12cd",
            "Good Night-0.5.2-updater-",
            "Good Night-x.y-updater-Ab12cd",
            "Other App-0.5.2-updater-Ab12cd",
            "Good Nightly-0.5.2-updater-Ab12cd",
            "Good Night-0.5.2-updater-../x",
        ] {
            assert!(!is_updater_staging_dir(other), "{other} is not ours");
        }
        for other in ["Good Night-0.5.2-installer.msi", "notes.exe", "Good Night-installer.exe"] {
            assert!(!is_updater_installer(other), "{other} is not ours");
        }
    }

    #[test]
    fn the_documented_lifetimes_are_the_ones_enforced() {
        assert_eq!(TEMP_LIFETIME_SECS, 3_600, "PRIVACY.md promises one hour");
        assert_eq!(CHECK_RECORD_LIFETIME_SECS, 2_592_000, "PRIVACY.md promises 30 days");
    }
}
