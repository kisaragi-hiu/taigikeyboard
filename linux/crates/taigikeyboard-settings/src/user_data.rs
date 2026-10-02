//! Opening the engine's user-data stores at launch (Custom Dictionary, the
//! three learning tables) under `$XDG_DATA_HOME/taigikeyboard` (roadmap L7).
//! Port of the Windows `user_data.rs`: the launch migrations run on the
//! engine's own thread off the open; no directory means no stores, and the
//! pages that need them say so.

use taigi_desktop_core::engine::user_data;
use taigi_desktop_storage::created;
use taigi_linux_platform::{UserDirectories, DESKTOP_PLATFORM};

/// Answers the failure the window's banner shows — the data directory
/// could not be created, or there is no user directory at all — or `None`
/// once the stores are opening.
pub fn open_at_launch() -> Option<String> {
    let Some(directories) = UserDirectories::resolve() else {
        return Some("HOME / XDG_DATA_HOME".to_owned());
    };
    let directory = match created(directories.data) {
        Ok(directory) => directory,
        Err(error) => {
            log::error!("user_data.no_data_directory error={error}");
            return Some(error.to_string());
        }
    };
    // A refused open is logged by the bridge; the pages mount and say so
    // on their first request, as they did when a store failed to open.
    user_data::open(&directory, DESKTOP_PLATFORM);
    None
}
