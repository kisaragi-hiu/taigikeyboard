//! A settings window's copy of `settings.json` and the one way it changes:
//! one atomic edit (lock, load, mutate, save) through the same store the
//! input method writes, then this copy follows the file. Both desktop
//! windows own one; each shell only decides how it is opened.
//!
//! Read-only is the named rule of roadmap W2 / L7: no per-user directory ⇒
//! every write refused, the reason on the banner from the first frame. The
//! two shells hold different documents meanwhile — Windows follows the file
//! it was opened over ([`SettingsWriter::read_only_over`]), Linux holds the
//! shipped defaults and reads nothing ([`SettingsWriter::read_only`]).

use crate::LiveSettings;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use taigi_desktop_core::settings::SettingsDocument;

/// How long an idle window waits before reading the file again: one `stat`
/// a second is nothing, and a chord's effect showing within a second reads
/// as live (roadmap W10 / L9 — `@AppStorage`'s job on the Mac).
pub const IDLE_REFRESH_INTERVAL: Duration = Duration::from_secs(1);

/// Where the window's document comes from, and whether it may be written.
/// The `LiveSettings` is shared rather than owned: one per process is the
/// point — it holds the fingerprint that decides whether the file moved —
/// and a caller may hold the same `Rc`.
enum Source {
    Writable(Rc<LiveSettings>),
    /// Followed, never written.
    ReadOnlyOver(Rc<LiveSettings>),
    /// Nothing on disk is read or tracked.
    Defaults,
}

pub struct SettingsWriter {
    source: Source,
    document: Arc<SettingsDocument>,
    write_failure: Option<String>,
}

impl SettingsWriter {
    /// Over the user's settings file, writable.
    pub fn new(live: Rc<LiveSettings>) -> Self {
        Self::over(Source::Writable(live), None)
    }

    /// Follows `live` but refuses every write; `detail` is what the banner
    /// shows after the message, for as long as the window is open.
    pub fn read_only_over(live: Rc<LiveSettings>, detail: impl Into<String>) -> Self {
        Self::over(Source::ReadOnlyOver(live), Some(detail.into()))
    }

    /// The shipped defaults, nothing read, every write refused; `detail` is
    /// what the banner shows after the message.
    pub fn read_only(detail: impl Into<String>) -> Self {
        Self {
            source: Source::Defaults,
            document: Arc::new(SettingsDocument::default()),
            write_failure: Some(detail.into()),
        }
    }

    fn over(source: Source, write_failure: Option<String>) -> Self {
        let mut writer = Self {
            source,
            document: Arc::default(),
            write_failure,
        };
        writer.refresh();
        writer
    }

    fn live(&self) -> Option<&Rc<LiveSettings>> {
        match &self.source {
            Source::Writable(live) | Source::ReadOnlyOver(live) => Some(live),
            Source::Defaults => None,
        }
    }

    pub fn document(&self) -> &SettingsDocument {
        &self.document
    }

    pub fn is_read_only(&self) -> bool {
        !matches!(self.source, Source::Writable(_))
    }

    /// The last write that failed, shown as a banner until a write
    /// succeeds: a control that snaps back with no word looks broken.
    pub fn write_failure(&self) -> Option<&str> {
        self.write_failure.as_deref()
    }

    /// Re-reads the file if its fingerprint moved, so a change made outside
    /// — the input method's own write for a chord or a menu row — shows
    /// without a restart. Answers whether the document changed.
    pub fn refresh(&mut self) -> bool {
        let Some(live) = self.live() else {
            return false;
        };
        let document = live.refresh_if_changed();
        let changed = !Arc::ptr_eq(&document, &self.document);
        self.document = document;
        changed
    }

    /// One atomic edit, then this copy follows the file — the same path the
    /// input method's own writes take, so two writers cannot lose each
    /// other's change. A failed write is reported, not swallowed; a
    /// read-only window refuses without running `mutate` or touching its
    /// banner.
    pub fn update(&mut self, mutate: impl FnOnce(&mut SettingsDocument)) {
        let Source::Writable(live) = &self.source else {
            return;
        };
        match live.store().update(mutate) {
            Ok(_) => self.write_failure = None,
            Err(error) => {
                log::error!("settings.update_failed error={error}");
                self.write_failure = Some(error.to_string());
            }
        }
        self.refresh();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SettingsFileStore;
    use std::path::Path;
    use taigi_desktop_core::settings::keys;

    fn live_over(directory: &Path) -> Rc<LiveSettings> {
        Rc::new(LiveSettings::new(SettingsFileStore::new(directory)))
    }

    /// Another process's write: the input method, or a hand edit.
    fn write_outside(directory: &Path, is_auto_space_enabled: bool) {
        let mut document = SettingsDocument::default();
        document.set_bool(&keys::IS_AUTO_SPACE_ENABLED, is_auto_space_enabled);
        SettingsFileStore::new(directory).save(&document).unwrap();
    }

    #[test]
    fn a_writable_window_writes_and_follows_the_file() {
        let directory = tempfile::tempdir().unwrap();
        let mut writer = SettingsWriter::new(live_over(directory.path()));
        assert!(!writer.is_read_only());
        assert_eq!(writer.write_failure(), None);
        writer.update(|document| document.set_bool(&keys::IS_AUTO_SPACE_ENABLED, true));
        assert!(writer.document().bool(&keys::IS_AUTO_SPACE_ENABLED));
        assert!(SettingsFileStore::new(directory.path())
            .load()
            .unwrap()
            .bool(&keys::IS_AUTO_SPACE_ENABLED));
        // Our own write is already followed: nothing new on the next beat.
        assert!(!writer.refresh());
        write_outside(directory.path(), false);
        assert!(writer.refresh(), "an outside write is a change");
        assert!(!writer.document().bool(&keys::IS_AUTO_SPACE_ENABLED));
        assert!(!writer.refresh(), "and only once");
    }

    #[test]
    fn a_failed_write_stays_on_the_banner_until_a_write_succeeds() {
        let directory = tempfile::tempdir().unwrap();
        let mut writer = SettingsWriter::new(live_over(directory.path()));
        // Another writer holds the lock for longer than `update` waits.
        let lock = SettingsFileStore::new(directory.path())
            .path()
            .with_extension("json.lock");
        std::fs::write(&lock, "").unwrap();
        writer.update(|document| document.set_bool(&keys::IS_AUTO_SPACE_ENABLED, true));
        assert!(writer.write_failure().is_some());
        assert!(!writer.document().bool(&keys::IS_AUTO_SPACE_ENABLED));
        std::fs::remove_file(&lock).unwrap();
        // An outside write followed is not a write of ours.
        write_outside(directory.path(), false);
        assert!(writer.refresh());
        assert!(writer.write_failure().is_some());
        // Any update that answers Ok clears it — one that changed nothing too.
        writer.update(|_| {});
        assert_eq!(writer.write_failure(), None);
    }

    #[test]
    fn a_read_only_window_over_a_file_follows_it_and_refuses_every_write() {
        let directory = tempfile::tempdir().unwrap();
        write_outside(directory.path(), true);
        let mut writer = SettingsWriter::read_only_over(live_over(directory.path()), "APPDATA");
        assert!(writer.is_read_only());
        assert!(
            writer.document().bool(&keys::IS_AUTO_SPACE_ENABLED),
            "the first frame shows the file it was opened over"
        );
        assert_eq!(writer.write_failure(), Some("APPDATA"));
        let mut ran = false;
        writer.update(|_| ran = true);
        assert!(!ran, "a refused write never runs the mutation");
        write_outside(directory.path(), false);
        assert!(writer.refresh(), "it still follows the file");
        assert!(!writer.document().bool(&keys::IS_AUTO_SPACE_ENABLED));
        assert_eq!(writer.write_failure(), Some("APPDATA"));
    }

    #[test]
    fn a_read_only_window_over_nothing_holds_the_defaults() {
        let mut writer = SettingsWriter::read_only("HOME / XDG_CONFIG_HOME");
        assert!(writer.is_read_only());
        assert_eq!(writer.document(), &SettingsDocument::default());
        let mut ran = false;
        writer.update(|_| ran = true);
        assert!(!ran);
        assert!(!writer.refresh());
        assert_eq!(writer.write_failure(), Some("HOME / XDG_CONFIG_HOME"));
    }
}
