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

/// The due test and the stamp as one step: run INSIDE one locked
/// `settings.json` update, it is the claim that lets exactly one of two
/// launches racing for the same due window fetch (the Windows scheduled
/// task and a window opening; the IBus engine and the Fcitx5 addon both
/// activating). Answers whether the caller won.
pub fn claim_due_check(document: &mut SettingsDocument, now_ms: i64) -> bool {
    if !is_due(document, now_ms) {
        return false;
    }
    stamp_next_check(document, now_ms);
    true
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

    #[test]
    fn one_claim_per_due_window() {
        let mut document = SettingsDocument::default();
        assert!(claim_due_check(&mut document, 1_000));
        assert!(
            !claim_due_check(&mut document, 1_001),
            "the second launch loses"
        );
        assert!(claim_due_check(&mut document, 1_000 + CHECK_INTERVAL_MS));
    }
}
