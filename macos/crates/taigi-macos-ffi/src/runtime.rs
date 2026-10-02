//! The process's desktop runtime behind the seam
//! (docs/architecture/macos-desktop-core-roadmap.md D3): `Configure` builds
//! it once, `Settings` replaces the key-path settings snapshot, `Prepare`
//! brings the engine up. One [`Shell`] serves the bridge; tests build their
//! own.
//!
//! Launch, not first key (inventory C6): Swift sends `Prepare` from
//! `applicationDidFinishLaunching`, so the lexicon is installed and the user
//! data opened at launch as before. `settings_store` is `None`: nothing here
//! writes settings, and the core's launch reconciliation of the Windows /
//! Linux shortcut registries never runs on the Mac, whose global shortcuts
//! stay in Swift (roadmap D5).

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use taigi_desktop_core::platform::DesktopPlatform;
use taigi_desktop_core::runtime::{DesktopRuntime, RuntimeParts};

use crate::proto::{
    ConfigureReply, ConfigureRequest, LexiconStats, PrepareReply, SettingsReply, SettingsRequest,
};
use crate::settings::{self, SnapshotSettings};

/// Which desktop this is: every desktop-core rule that differs per desktop
/// is handed it (roadmap D6) — `platform_id = macos` on every engine
/// request, the `DELETE` user-data journal, the Mac chord grammar.
pub(crate) const DESKTOP_PLATFORM: DesktopPlatform = DesktopPlatform::MacOS;

/// Why a request was refused; the seam answers FAIL_INVARIANT.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Refusal {
    AlreadyConfigured,
    NotConfigured,
    Settings(settings::SettingsRefusal),
}

#[derive(Default)]
pub(crate) struct Shell {
    /// Exists before `Configure`, so a `Settings` may arrive first.
    settings: Arc<SnapshotSettings>,
    runtime: OnceLock<DesktopRuntime>,
}

impl Shell {
    /// Builds the runtime once. Building one has no side effect, so a
    /// second (or a racing) `Configure` is refused whole when it cannot be
    /// published.
    pub(crate) fn configure(&self, request: ConfigureRequest) -> Result<ConfigureReply, Refusal> {
        let runtime = DesktopRuntime::new(self.runtime_parts(request));
        self.runtime
            .set(runtime)
            .map_err(|_| Refusal::AlreadyConfigured)?;
        Ok(ConfigureReply {
            settings: settings::descriptors(),
        })
    }

    /// What `Configure` builds the runtime from. Each directory is
    /// optional on its own, as at launch before: no data directory skips
    /// the user-data open, no dictionaries directory skips the lexicon.
    fn runtime_parts(&self, request: ConfigureRequest) -> RuntimeParts {
        let dictionaries = request.dictionaries_directory.map(PathBuf::from);
        let system_locale = request.system_locale;
        RuntimeParts {
            settings: Arc::clone(&self.settings) as _,
            settings_store: None,
            data_directory: request.data_directory.map(PathBuf::from),
            dictionaries: Box::new(move || dictionaries.clone()),
            dictionary_version: request.dictionary_stamp,
            system_locale: Box::new(move || system_locale.clone()),
            platform: DESKTOP_PLATFORM,
        }
    }

    /// Replaces the settings snapshot, or keeps the last one when any
    /// entry is refused.
    pub(crate) fn settings(&self, request: &SettingsRequest) -> Result<SettingsReply, Refusal> {
        let document = settings::document_from(&request.entries).map_err(Refusal::Settings)?;
        self.settings.replace(document);
        Ok(SettingsReply {})
    }

    /// The lexicon installed and the user data opened — once; a repeat
    /// answers the first result without touching the engine.
    pub(crate) fn prepare(&self) -> Result<PrepareReply, Refusal> {
        let runtime = self.runtime.get().ok_or(Refusal::NotConfigured)?;
        let lexicon = runtime
            .prepare_for_first_key()
            .lexicon
            .map(|stats| LexiconStats {
                dictionary_record_count: stats.dictionary_record_count,
                prefix_index_entry_count: stats.prefix_index_entry_count,
            });
        Ok(PrepareReply { lexicon })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{configure_request, text};
    use std::path::Path;
    use taigi_desktop_core::settings::{InputMode, SettingChoice, SettingsProvider};

    /// trace: the request's fields reach RuntimeParts as given; the platform
    /// is MacOS (engine `platform_id = macos` — desktop-core
    /// `engine/bridge.rs` `wire_platform`; journal DELETE —
    /// `engine/user_data.rs` `each_desktop_opens_its_stores_under_its_own_journal`).
    #[test]
    fn configure_builds_the_runtime_from_the_request() {
        let shell = Shell::default();
        let parts = shell.runtime_parts(configure_request(
            Some(Path::new("/data")),
            Some(Path::new("/Resources")),
        ));
        assert_eq!(parts.platform, DesktopPlatform::MacOS);
        assert_eq!(parts.dictionary_version, 30613);
        assert_eq!(parts.data_directory, Some(PathBuf::from("/data")));
        assert_eq!((parts.dictionaries)(), Some(PathBuf::from("/Resources")));
        assert_eq!((parts.system_locale)(), "zh-Hant-TW");
        assert!(parts.settings_store.is_none(), "nothing writes settings");

        let parts = shell.runtime_parts(configure_request(None, None));
        assert_eq!(parts.data_directory, None);
        assert_eq!((parts.dictionaries)(), None);
    }

    #[test]
    fn configure_is_once_only() {
        let shell = Shell::default();
        let reply = shell.configure(configure_request(None, None)).unwrap();
        assert_eq!(reply.settings, settings::descriptors());
        assert_eq!(
            shell.configure(configure_request(None, None)),
            Err(Refusal::AlreadyConfigured)
        );
    }

    #[test]
    fn prepare_needs_configure() {
        assert_eq!(Shell::default().prepare(), Err(Refusal::NotConfigured));
    }

    /// No directories: Prepare installs nothing and opens nothing, every
    /// time — and never reaches the engine.
    #[test]
    fn prepare_without_directories_brings_nothing_up() {
        let shell = Shell::default();
        shell.configure(configure_request(None, None)).unwrap();
        assert_eq!(shell.prepare(), Ok(PrepareReply { lexicon: None }));
        assert_eq!(shell.prepare(), Ok(PrepareReply { lexicon: None }));
    }

    /// The runtime reads the pushed snapshot — pushed before or after
    /// Configure — and a refused push keeps the last one.
    #[test]
    fn the_runtime_reads_the_pushed_settings() {
        let shell = Shell::default();
        let poj = SettingsRequest {
            entries: vec![text("inputMode", "poj")],
        };
        shell.settings(&poj).unwrap();
        shell.configure(configure_request(None, None)).unwrap();
        let input_mode = || {
            let runtime = shell.runtime.get().unwrap();
            runtime.settings.current().engine_settings().input_mode
        };
        assert_eq!(input_mode(), InputMode::Poj);

        let mut refused = poj.clone();
        refused.entries.push(text("notASetting", ""));
        assert!(matches!(
            shell.settings(&refused),
            Err(Refusal::Settings(_))
        ));
        assert_eq!(
            input_mode(),
            InputMode::Poj,
            "a refused push keeps the last"
        );

        shell
            .settings(&SettingsRequest { entries: vec![] })
            .unwrap();
        assert_eq!(
            input_mode(),
            InputMode::DEFAULT,
            "a removed key reads as its default"
        );
    }
}
