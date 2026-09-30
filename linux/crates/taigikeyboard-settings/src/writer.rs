//! How the Linux window opens the shared `taigi_desktop_storage::SettingsWriter`:
//! over the user's XDG configuration directory, or read-only on the shipped
//! defaults when there is none (roadmap L7).

use std::rc::Rc;
use taigi_desktop_storage::{created, LiveSettings, SettingsFileStore, SettingsWriter};
use taigi_linux_platform::UserDirectories;

/// What the banner says when there is no user directory at all: the
/// variables the XDG lookup wanted.
const NO_DIRECTORIES_DETAIL: &str = "HOME / XDG_CONFIG_HOME";

/// Over the user's configuration directory, or read-only when there is none
/// or it cannot be created — with the real reason as the banner's detail.
pub fn at_launch() -> SettingsWriter {
    let Some(directories) = UserDirectories::resolve() else {
        log::error!("settings.no_user_directories");
        return SettingsWriter::read_only(NO_DIRECTORIES_DETAIL.to_owned());
    };
    match created(directories.config) {
        Ok(directory) => SettingsWriter::new(Rc::new(LiveSettings::new(SettingsFileStore::new(
            &directory,
        )))),
        Err(error) => {
            log::error!("settings.no_config_directory error={error}");
            SettingsWriter::read_only(error.to_string())
        }
    }
}
