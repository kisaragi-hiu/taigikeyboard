//! Opening the engine's user-data stores at launch (once, from
//! `winui::window::run`).

use std::path::PathBuf;
use taigi_desktop_core::engine::user_data;
use taigi_windows_platform::DESKTOP_PLATFORM;

/// Opens the stores for this process unless the window is read-only (no
/// `%APPDATA%`: it shows the defaults and writes nothing, so it opens
/// nothing either).
///
/// What the DLL does on its first consumed key is done here too: a fresh
/// install whose first visitor is this window still gets its seeds, and an
/// older dictionary its re-derived keys. The engine finishes that on a
/// thread of its own, so a large dictionary cannot hold the window closed;
/// a page's first request waits for it.
pub fn open_at_launch(directory: PathBuf, is_read_only: bool) {
    if is_read_only {
        return;
    }
    // A refused open is logged by the bridge; the pages say so on their
    // first request.
    user_data::open(&directory, DESKTOP_PLATFORM);
}
