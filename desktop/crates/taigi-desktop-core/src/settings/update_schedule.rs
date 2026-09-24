//! When the automatic update check is due, over `updateNextCheckMs`. Port
//! of `UpdateChecker`'s schedule — stamped BEFORE the fetch so a hanging
//! server does not re-check every launch. Here rather than beside the
//! fetch (`taigi-desktop-update`) so a process that only asks "is it due"
//! links no network stack.

use super::{keys, SettingsDocument};

/// `UpdateChecker.checkInterval`: daily.
pub const CHECK_INTERVAL_MS: i64 = 24 * 60 * 60 * 1000;

/// Whether the daily check is due (`checkAutomatically`'s guard).
pub fn is_due(document: &SettingsDocument, now_ms: i64) -> bool {
    now_ms >= document.i64(&keys::UPDATE_NEXT_CHECK_MS)
}

/// Stamped before the fetch, whatever it answers.
pub fn stamp_next_check(document: &mut SettingsDocument, now_ms: i64) {
    document.set_i64(&keys::UPDATE_NEXT_CHECK_MS, now_ms + CHECK_INTERVAL_MS);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_schedule_is_due_again_one_interval_after_the_stamp() {
        let mut document = SettingsDocument::default();
        assert!(is_due(&document, 0));
        stamp_next_check(&mut document, 1_000);
        assert!(!is_due(&document, 1_000 + CHECK_INTERVAL_MS - 1));
        assert!(is_due(&document, 1_000 + CHECK_INTERVAL_MS));
    }
}
