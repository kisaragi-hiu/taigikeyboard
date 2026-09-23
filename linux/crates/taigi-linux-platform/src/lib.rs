//! The few Linux-specific pieces the IBus engine and the settings window
//! share, all pure (no D-Bus, no GTK): where the user's data lives (XDG),
//! where the install put the dictionaries, how a framework key event becomes the
//! desktop core's [`taigi_desktop_core::keys::KeyEventSnapshot`], how the
//! engine opens the settings window, and which language the UI draws in.
//!
//! Counterpart of `windows/crates/taigi-windows-platform`, minus everything
//! that needed a Win32 handle. Design record:
//! `docs/architecture/linux-roadmap.md` (L5, L7, L8).

pub mod key_translation;
pub mod launcher;
pub mod locale;
pub mod paths;

pub use key_translation::{snapshot, KeyState, RawKeyEvent};
pub use launcher::{open_settings, open_url};
pub use locale::system_locale;
pub use paths::{
    config_directory, data_directory, dictionaries_directory, install_prefix, settings_binary,
    InstallLayout, UserDirectories,
};
