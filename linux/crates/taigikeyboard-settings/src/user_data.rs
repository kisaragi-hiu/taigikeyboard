//! The user-data stores the window reads and writes (自訂詞庫, the three
//! learning tables), opened once at launch under `$XDG_DATA_HOME/taigikeyboard`
//! (roadmap L7). Port of the Windows `user_data.rs`: the launch migrations
//! run on a background thread off the open; no directory means no stores
//! and the pages that need them say so.

use std::sync::Arc;
use taigi_desktop_storage::{created, UserDataStores};
use taigi_linux_platform::UserDirectories;

pub fn open_at_launch() -> Option<UserDataStores> {
    let directories = UserDirectories::resolve()?;
    let directory = match created(directories.data) {
        Ok(directory) => directory,
        Err(error) => {
            log::error!("user_data.no_data_directory error={error}");
            return None;
        }
    };
    let stores = UserDataStores::new(directory);
    stores.open();
    let custom_dictionary = Arc::clone(&stores.custom_dictionary);
    std::thread::Builder::new()
        .name("taigi-custom-dictionary-launch".into())
        .spawn(move || {
            if let Err(error) = custom_dictionary.rederive_search_keys_if_needed() {
                log::error!("custom_dictionary.rederive_failed error={error}");
            }
            if let Err(error) = custom_dictionary.seed_if_empty() {
                log::error!("custom_dictionary.seed_failed error={error}");
            }
        })
        .ok();
    Some(stores)
}
