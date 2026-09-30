//! The Linux half of `taigi_desktop_core::settings::presentation`: the
//! machine's locale the shared resolver is asked with, read from the
//! environment at every call.

use taigi_desktop_core::settings::{presentation, SettingsDocument};
use taigi_desktop_core::strings::{DisplayLanguage, StringResolver};

/// The display language the document asks for, `system` resolved against
/// the machine (`DisplayLanguageStore.syncFromSettings`).
pub fn display_language_of(document: &SettingsDocument) -> DisplayLanguage {
    document.effective_display_language(&taigi_linux_platform::system_locale())
}

pub fn strings_for(document: &SettingsDocument) -> StringResolver {
    presentation::strings_for(document, &taigi_linux_platform::system_locale())
}
