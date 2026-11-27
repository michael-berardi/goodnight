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
pub const TEMP_NAMES: [&str; 2] = ["settings.json.tmp", "update-check.json.tmp"];

/// The Windows settings file. It holds the user's own choices and never expires.
pub const SETTINGS_FILE_NAME: &str = "settings.json";

/// The Windows update-check record: last check time plus, if one failed, its cause.
pub const CHECK_RECORD_FILE_NAME: &str = "update-check.json";

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
    } else if name == CHECK_RECORD_FILE_NAME {
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
    fn a_file_timestamped_in_the_future_is_not_debris() {
        // Clocks move backwards across updates and time zones; saturate instead of expiring.
        assert_eq!(disposition(TEMP_NAMES[0], NOW + 10_000, NOW), Disposition::Keep);
        assert_eq!(disposition(CHECK_RECORD_FILE_NAME, NOW + 10_000, NOW), Disposition::Keep);
    }

    #[test]
    fn the_documented_lifetimes_are_the_ones_enforced() {
        assert_eq!(TEMP_LIFETIME_SECS, 3_600, "PRIVACY.md promises one hour");
        assert_eq!(CHECK_RECORD_LIFETIME_SECS, 2_592_000, "PRIVACY.md promises 30 days");
    }
}
