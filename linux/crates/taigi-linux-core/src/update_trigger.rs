//! When the automatic daily update check runs (roadmap L10). Linux has no
//! scheduled task; the input method is the one process that lives for the
//! whole session, so an activation (IBus `Enable` / `FocusIn`, Fcitx5
//! `activate`) past the due time spawns the settings binary with
//! `--check-updates` and no window. The child claims the due window under the
//! settings lock (`update_schedule::claim_due_check`), so the IBus engine and
//! the Fcitx5 addon activating together still fetch once. Nothing here links
//! a network stack.

use crate::runtime::Runtime;
use std::sync::atomic::{AtomicI64, Ordering};
use taigi_desktop_core::composing::{Clock, SystemClock};
use taigi_desktop_core::settings::{keys, update_schedule};
use taigi_linux_platform::check_for_updates_in_background;

/// The next time this process asks — one per process, as the runtime is —
/// cached so an activation, every focus change, costs one clock read and one
/// compare rather than a settings `stat`. [`UNREAD`] until the first
/// activation reads the file. Not re-read after: a manual check or the other
/// framework's child moving the file's time on costs at most one extra spawn
/// a day, which finds the window claimed and exits.
static NEXT_CHECK_MS: AtomicI64 = AtomicI64::new(UNREAD);

const UNREAD: i64 = i64::MIN;

/// Moves the cached time a full interval on BEFORE the spawn, so a binary
/// that cannot start is not respawned on every focus; the child stamps the
/// file itself. One winner when two engine objects activate at once.
/// Answers whether this call fired.
fn fire_if_due(
    next_check_ms: &AtomicI64,
    stored_next_ms: impl FnOnce() -> i64,
    now_ms: i64,
) -> bool {
    let mut next = next_check_ms.load(Ordering::Acquire);
    if next == UNREAD {
        // Published once: a second first activation that read the file at
        // the same time takes the value already there instead.
        next = match next_check_ms.compare_exchange(
            UNREAD,
            stored_next_ms(),
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => next_check_ms.load(Ordering::Acquire),
            Err(published) => published,
        };
    }
    if now_ms < next {
        return false;
    }
    next_check_ms
        .compare_exchange(
            next,
            now_ms + update_schedule::CHECK_INTERVAL_MS,
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .is_ok()
}

/// An input context activated: spawn the background check when it is due.
/// Never in the e2e test build: a scenario run must not reach the network
/// or put a second process beside the one it measures.
pub fn on_activate(runtime: &Runtime) {
    if cfg!(any(feature = "e2e-trace", feature = "disable-updates")) {
        return;
    }
    let fired = fire_if_due(
        &NEXT_CHECK_MS,
        || runtime.settings.current().i64(&keys::UPDATE_NEXT_CHECK_MS),
        SystemClock.now_ms(),
    );
    if fired {
        log::info!("update.check_spawned");
        check_for_updates_in_background();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = update_schedule::CHECK_INTERVAL_MS;

    #[test]
    fn fires_once_per_due_window_and_reads_the_file_once() {
        // trace: stored next = 5_000; at 4_999 not due; at 5_000 fires and
        // moves the cache to 5_000 + DAY; a second activation at 5_001 does
        // not fire; the file is read on the first call only.
        let trigger = AtomicI64::new(UNREAD);
        let mut reads = 0;
        assert!(!fire_if_due(
            &trigger,
            || {
                reads += 1;
                5_000
            },
            4_999
        ));
        assert!(fire_if_due(&trigger, || unreachable!("cached"), 5_000));
        assert!(!fire_if_due(&trigger, || unreachable!("cached"), 5_001));
        assert!(fire_if_due(
            &trigger,
            || unreachable!("cached"),
            5_000 + DAY
        ));
        assert_eq!(reads, 1);
    }

    #[test]
    fn a_fresh_install_checks_on_its_first_activation() {
        // `updateNextCheckMs` defaults to 0: never checked is overdue.
        let trigger = AtomicI64::new(UNREAD);
        assert!(fire_if_due(&trigger, || 0, 1_000));
    }
}
