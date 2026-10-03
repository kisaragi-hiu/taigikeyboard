//! The Windows half of `taigi_desktop_core::settings::presentation`: the
//! machine's UI language the shared resolver is asked with, and the
//! browser hand-off.

use std::sync::OnceLock;
use taigi_desktop_core::settings::presentation::{self, PageMessage};
use taigi_desktop_core::settings::SettingsDocument;
use taigi_desktop_core::strings::{StringKey, StringResolver};

/// The machine's UI language, read once. Windows requires a sign-out to
/// change it, so a running window can hold the answer instead of asking
/// `GetUserPreferredUILanguages` again for every redraw.
fn system_locale() -> &'static str {
    static LOCALE: OnceLock<String> = OnceLock::new();
    LOCALE.get_or_init(taigi_windows_platform::system_locale)
}

/// The resolver for the document's display language, `system` resolved
/// against the machine (`DisplayLanguageStore.syncFromSettings`).
pub fn strings_for(document: &SettingsDocument) -> StringResolver {
    presentation::strings_for(document, system_locale())
}

/// Opens `url` in the browser, answering the message to show when it
/// refused: a button that silently does nothing is indistinguishable from
/// a broken one (`ExternalLinkButton.open()` on the Mac).
pub fn open_url(url: &str) -> Option<PageMessage> {
    (!taigi_windows_platform::open_url(url))
        .then(|| PageMessage::failure(StringKey::DesktopOpenURLFailed, url))
}
