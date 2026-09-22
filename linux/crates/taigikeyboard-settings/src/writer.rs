//! The window's copy of `settings.json` and the one way it changes: one
//! atomic edit (lock, load, mutate, save) through the same store the engine
//! writes, then this copy follows the file. Port of the Windows
//! `settings_writer.rs`; the read-only mode is the same named rule (roadmap
//! W2 / L7): no per-user directory ⇒ defaults shown, every write refused
//! with a banner, never a file the engine would not read.

use std::sync::Arc;
use std::time::Duration;
use taigi_desktop_core::settings::SettingsDocument;
use taigi_desktop_core::strings::StringResolver;
use taigi_desktop_storage::{created, LiveSettings, SettingsFileStore};
use taigi_linux_platform::UserDirectories;

/// How long an idle window waits before reading the file again: one `stat`
/// a second is nothing, and a chord's effect showing within a second reads
/// as live (roadmap L9).
pub const REFRESH_INTERVAL: Duration = Duration::from_secs(1);

/// What the banner says when there is no `$HOME` / `$XDG_CONFIG_HOME`: the
/// missing variable's name is the whole diagnosis.
const READ_ONLY_DETAIL: &str = "XDG_CONFIG_HOME";

pub struct SettingsWriter {
    live: LiveSettings,
    is_read_only: bool,
    document: Arc<SettingsDocument>,
    write_failure: Option<String>,
}

impl SettingsWriter {
    /// Over the user's configuration directory, or read-only over a
    /// temporary one when there is none.
    pub fn at_launch() -> Self {
        let directory = UserDirectories::resolve().and_then(|directories| {
            created(directories.config)
                .map_err(|error| {
                    log::error!("settings.no_config_directory error={error}");
                })
                .ok()
        });
        match directory {
            Some(directory) => Self::new(SettingsFileStore::new(&directory), false),
            None => Self::new(SettingsFileStore::new(&std::env::temp_dir()), true),
        }
    }

    pub fn new(store: SettingsFileStore, is_read_only: bool) -> Self {
        let live = LiveSettings::new(store);
        let document = live.refresh_if_changed();
        Self {
            live,
            is_read_only,
            document,
            write_failure: is_read_only.then(|| READ_ONLY_DETAIL.to_owned()),
        }
    }

    pub fn document(&self) -> &SettingsDocument {
        &self.document
    }

    pub fn strings(&self) -> StringResolver {
        crate::presentation::strings_for(&self.document)
    }

    pub fn is_read_only(&self) -> bool {
        self.is_read_only
    }

    /// The last write that failed, shown as a banner until a write
    /// succeeds: a control that snaps back with no word looks broken.
    pub fn write_failure(&self) -> Option<&str> {
        self.write_failure.as_deref()
    }

    /// Re-reads the file if its fingerprint moved, so a change made outside
    /// — the engine's own write for a chord or a menu row — shows without
    /// a restart. Answers whether the document changed.
    pub fn refresh(&mut self) -> bool {
        let document = self.live.refresh_if_changed();
        let changed = !Arc::ptr_eq(&document, &self.document);
        self.document = document;
        changed
    }

    /// One atomic edit, then this copy follows the file. A failed write is
    /// reported, not swallowed; a read-only window refuses without touching
    /// its banner.
    pub fn update(&mut self, mutate: impl FnOnce(&mut SettingsDocument)) {
        if self.is_read_only {
            return;
        }
        match self.live.store().update(mutate) {
            Ok(_) => self.write_failure = None,
            Err(error) => {
                log::error!("settings.update_failed error={error}");
                self.write_failure = Some(error.to_string());
            }
        }
        self.refresh();
    }
}
