//! What the system's input indicator shows for this input method — the
//! Windows taskbar button, the Fcitx5 tray icon, the IBus panel symbol: the
//! script being typed and what a commit writes, one glyph each, the way Mozc
//! shows あ / _A (Discord report 2026-10-05). macOS draws one static input
//! source icon (D5, `macos-roadmap.md`) and reports changes through its mode
//! flash instead.

use crate::keys::LanguageMode;
use crate::settings::{EngineSettings, InputMode};

/// One indicator state. The output half is the EFFECTIVE swap
/// (`EngineSettings::is_hanji_first`, invariants §42): Hanji with
/// Romanization always commits the hanji, Romanization Only never does, and
/// side by side follows the stored swap. TPS has one state — the swap and
/// the display mode are inert there (`ShortcutAction::is_inert_under`) —
/// and English (Windows' Shift tap) overrides everything.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ModeIndicator {
    TlHanji,
    TlRomanization,
    PojHanji,
    PojRomanization,
    Tps,
    English,
}

impl ModeIndicator {
    pub const ALL: [Self; 6] = [
        Self::TlHanji,
        Self::TlRomanization,
        Self::PojHanji,
        Self::PojRomanization,
        Self::Tps,
        Self::English,
    ];

    pub fn of(settings: &EngineSettings, language: LanguageMode) -> Self {
        if language.is_english() {
            return Self::English;
        }
        let hanji = settings.is_hanji_first;
        match (settings.input_mode, hanji) {
            (InputMode::Tps, _) => Self::Tps,
            (InputMode::Tl, true) => Self::TlHanji,
            (InputMode::Tl, false) => Self::TlRomanization,
            (InputMode::Poj, true) => Self::PojHanji,
            (InputMode::Poj, false) => Self::PojRomanization,
        }
    }

    /// The indicator text: a hanji when a commit writes hanji, the
    /// romanization's own spelling of /ts/ when it writes romanization. Not
    /// an i18n string — it names the script, so it reads the same in every
    /// UI language. One or two characters: GNOME Shell shows an IBus
    /// `InputMode` symbol only at that length (`js/ui/status/keyboard.js`).
    pub fn symbol(self) -> &'static str {
        match self {
            Self::TlHanji => "台",
            Self::TlRomanization => "Ts",
            Self::PojHanji => "白",
            Self::PojRomanization => "Ch",
            Self::Tps => "方",
            Self::English => "英",
        }
    }

    /// The icon's file stem, the same on every platform that ships one:
    /// the Linux hicolor theme name and the Windows `.ico` file name, drawn
    /// by `tools/desktop/make-app-icon.swift` from [`Self::symbol`].
    pub fn icon_name(self) -> &'static str {
        match self {
            Self::TlHanji => "taigikeyboard-tl-hanji",
            Self::TlRomanization => "taigikeyboard-tl-romanization",
            Self::PojHanji => "taigikeyboard-poj-hanji",
            Self::PojRomanization => "taigikeyboard-poj-romanization",
            Self::Tps => "taigikeyboard-tps",
            Self::English => "taigikeyboard-english",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{keys, CandidateDisplayMode, SettingsDocument};
    use std::collections::HashSet;

    fn indicator(
        input_mode: InputMode,
        display_mode: CandidateDisplayMode,
        stored_swap: bool,
        language: LanguageMode,
    ) -> ModeIndicator {
        let mut document = SettingsDocument::default();
        document.set_choice(&keys::INPUT_MODE, input_mode);
        document.set_choice(&keys::CANDIDATE_DISPLAY_MODE, display_mode);
        document.set_bool(&keys::IS_HANJI_FIRST, stored_swap);
        ModeIndicator::of(&document.engine_settings(), language)
    }

    #[test]
    fn invariant_desktop_mode_indicator_every_setting_combination_maps_to_its_indicator() {
        use CandidateDisplayMode::{Combined, RomanOnly, SideBySide};
        use InputMode::{Poj, Tl, Tps};
        use ModeIndicator as M;
        // trace: effective_hanji_first = Combined || (stored && display != RomanOnly)
        let table = [
            (Tl, SideBySide, true, M::TlHanji),
            (Tl, SideBySide, false, M::TlRomanization),
            (Tl, Combined, true, M::TlHanji),
            (Tl, Combined, false, M::TlHanji),
            (Tl, RomanOnly, true, M::TlRomanization),
            (Tl, RomanOnly, false, M::TlRomanization),
            (Poj, SideBySide, true, M::PojHanji),
            (Poj, SideBySide, false, M::PojRomanization),
            (Poj, Combined, true, M::PojHanji),
            (Poj, Combined, false, M::PojHanji),
            (Poj, RomanOnly, true, M::PojRomanization),
            (Poj, RomanOnly, false, M::PojRomanization),
        ];
        for (input_mode, display_mode, swap, expected) in table {
            assert_eq!(
                indicator(input_mode, display_mode, swap, LanguageMode::Taigi),
                expected,
                "{input_mode:?} {display_mode:?} swap={swap}"
            );
        }
        for display_mode in [SideBySide, Combined, RomanOnly] {
            for swap in [true, false] {
                assert_eq!(
                    indicator(Tps, display_mode, swap, LanguageMode::Taigi),
                    M::Tps
                );
            }
        }
    }

    #[test]
    fn english_overrides_every_setting() {
        for input_mode in [InputMode::Tl, InputMode::Poj, InputMode::Tps] {
            for swap in [true, false] {
                assert_eq!(
                    indicator(
                        input_mode,
                        CandidateDisplayMode::SideBySide,
                        swap,
                        LanguageMode::English
                    ),
                    ModeIndicator::English
                );
            }
        }
    }

    #[test]
    fn symbols_fit_the_gnome_indicator_and_are_distinct() {
        let symbols: HashSet<_> = ModeIndicator::ALL.iter().map(|m| m.symbol()).collect();
        assert_eq!(symbols.len(), ModeIndicator::ALL.len());
        for mode in ModeIndicator::ALL {
            let length = mode.symbol().chars().count();
            assert!((1..=2).contains(&length), "{mode:?}");
        }
    }

    #[test]
    fn icon_names_are_distinct() {
        let names: HashSet<_> = ModeIndicator::ALL.iter().map(|m| m.icon_name()).collect();
        assert_eq!(names.len(), ModeIndicator::ALL.len());
    }

    /// The generator (`tools/desktop/make-app-icon.swift` `modeIcons`) keeps
    /// its own copy of the name table; a variant it does not draw has no
    /// file, and this is where that shows.
    #[test]
    fn icon_assets_exist_for_every_indicator() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        for mode in ModeIndicator::ALL {
            let name = mode.icon_name();
            let windows = root.join(format!("windows/resources/mode/{name}.ico"));
            assert!(windows.is_file(), "{}", windows.display());
            // Linux has no English mode (`chrome::mode_indicator`).
            if mode == ModeIndicator::English {
                continue;
            }
            for size in [16, 22, 24, 32, 48, 64] {
                let linux = root.join(format!(
                    "linux/data/icons/hicolor/{size}x{size}/apps/{name}.png"
                ));
                assert!(linux.is_file(), "{}", linux.display());
            }
        }
    }
}
