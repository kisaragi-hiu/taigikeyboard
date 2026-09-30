//! FST key families — the `<family>:` prefix every `dictionary.fst` /
//! `syllables.fst` key carries, and the search key built from raw input.
//!
//! Families (frozen: `dictionary/build/create_fst.py` writes the same bytes):
//! - `tl:` / `poj:` / `tps:` — the phonetic family of each romanization
//! - `tl-abbrev:` / `poj-abbrev:` / `tps-abbrev:` — the per-syllable
//!   abbreviation (acronym) face of each family, kept out of the phonetic
//!   range so a prefix scan over `tl:` never meets an acronym key
//!   ([`abbrev_family_key`])
//! - `hanzi:` — hanji ([`HANJI_KEY_PREFIX`])

use unicode_normalization::UnicodeNormalization;

use crate::api::{tl_display_to_poj_display, InputMode};
use crate::normalization::{is_combining_tone_mark, normalize_input};
use crate::syllable::normalize_to_poj;
use crate::tps::{normalize_tps_tone8_scalar, tps_notone_from_tl};

/// A phonetic key family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyFamily {
    Tl,
    Poj,
    Tps,
}

impl KeyFamily {
    /// The family an input mode's keys live in. English shares the TL family:
    /// English buffers have no inventory of their own.
    pub fn for_input_mode(mode: InputMode) -> Self {
        match mode {
            InputMode::Poj => Self::Poj,
            InputMode::Tps => Self::Tps,
            InputMode::Tl | InputMode::English => Self::Tl,
        }
    }

    /// Bare family tag (`"tl"`), also the `family` column of the custom
    /// dictionary search-key table.
    pub fn tag(self) -> &'static str {
        match self {
            Self::Tl => "tl",
            Self::Poj => "poj",
            Self::Tps => "tps",
        }
    }

    /// Key prefix, colon included (`"tl:"`).
    pub fn prefix(self) -> &'static str {
        match self {
            Self::Tl => "tl:",
            Self::Poj => "poj:",
            Self::Tps => "tps:",
        }
    }

    /// The family whose [`prefix`](Self::prefix) is exactly `prefix`; `None`
    /// for `hanzi:`, an abbreviation family or anything else.
    pub fn from_prefix(prefix: &str) -> Option<Self> {
        [Self::Tl, Self::Poj, Self::Tps]
            .into_iter()
            .find(|family| family.prefix() == prefix)
    }

    /// The prefix-qualified lookup key for raw user input (`tl:gua2`,
    /// `tps:ㄍㄨㄚˋ`). TL / POJ run through [`normalize_input`] so diacritic
    /// and numeric tone forms meet the same stored key; TPS keeps Bopomofo
    /// literal ([`tps_key_body`]).
    pub fn search_key(self, input: &str) -> String {
        let body = match self {
            Self::Tl | Self::Poj => normalize_input(input),
            Self::Tps => tps_key_body(input),
        };
        format!("{}{body}", self.prefix())
    }

    /// A TL reading's toneless key body in this family — the `tl_notone` /
    /// `poj_notone` / `tps_notone` column `create_fst.py` indexes the row
    /// under, reconstructed at runtime from the reading alone (`dict.bin`
    /// carries only `tl`). TPS returns the primary face; its or→er dialect
    /// variant is [`crate::tps_notone_or_variant`].
    pub fn toneless_face(self, reading_tl: &str) -> String {
        match self {
            Self::Tl => tl_toneless_face(reading_tl),
            Self::Poj => poj_notone_of_display(&tl_display_to_poj_display(reading_tl)),
            Self::Tps => tps_notone_from_tl(reading_tl),
        }
    }
}

/// `normalize_input` yields a per-syllable numeric-tone form
/// (`gín-á-lâng` → `gin2a2lang5`), so every ASCII tone digit is dropped to
/// reach the stored fused surface (`remove_tone(to_numeric_tone(tl)) ==
/// tl_notone` holds for every `dictionary.csv` row;
/// `lexicon/tests/roman_num_face_parity.rs`).
fn tl_toneless_face(reading_tl: &str) -> String {
    normalize_input(reading_tl)
        .chars()
        .filter(|c| !c.is_ascii_digit())
        .collect()
}

/// The `poj_notone` surface of a POJ display form, byte-for-byte the way
/// `dictionary/build/merge_csv.py:226-240` derives it:
///   1. split on both `-` AND space (`record.tl` carries multi-syllable
///      readings as `gín-á-lâng` or `iā sī`), skipping empty tokens
///      (`-tiong-tàu`, `thàu-tiong-tàu-`);
///   2. per token: NFD-walk, drop the 8 combining tone marks, lowercase,
///      apply [`normalize_to_poj`] (`ou→oo`, `o\u{0358}→oo`, `\u{207f}→nn`,
///      `\u{1d3a}→nn`), strip ASCII digits;
///   3. concatenate.
///
/// Per-token normalization is load-bearing: `ou → oo` must not fire across
/// a hyphen boundary (`tó-uī` → `toui`, not `tooi`). Encoding-only — no
/// phonotactic gate — because the build has none (`hehⁿ` → `hehnn` is
/// indexed). `lexicon/tests/poj_notone_parity.rs` pins this against every
/// row of `dictionary/output/dictionary.csv`.
fn poj_notone_of_display(poj_display: &str) -> String {
    let mut out = String::with_capacity(poj_display.len());
    for token in poj_display.split(['-', ' ']) {
        if token.is_empty() {
            continue;
        }
        let mut token_buf = String::with_capacity(token.len());
        for ch in token.nfd() {
            if is_combining_tone_mark(ch) {
                continue;
            }
            for lower_ch in ch.to_lowercase() {
                token_buf.push(lower_ch);
            }
        }
        for c in normalize_to_poj(&token_buf).chars() {
            if !c.is_ascii_digit() {
                out.push(c);
            }
        }
    }
    out
}

/// Key prefix of the hanji family.
pub const HANJI_KEY_PREFIX: &str = "hanzi:";

/// Suffix that turns a phonetic family prefix into its abbreviation family
/// (`tl:` → `tl-abbrev:`). Mirrors `dictionary/build/create_fst.py`.
const ABBREV_FAMILY_SUFFIX: &str = "-abbrev";

/// The abbreviation-family twin of a phonetic-family key: `tl:ss` →
/// `tl-abbrev:ss`, `tps:ㄙㄒ` → `tps-abbrev:ㄙㄒ`. `None` for a key with no
/// `<family>:` prefix or one that is already an abbreviation key.
pub fn abbrev_family_key(key: &str) -> Option<String> {
    let (family, body) = key.split_once(':')?;
    if family.ends_with(ABBREV_FAMILY_SUFFIX) {
        return None;
    }
    Some(format!("{family}{ABBREV_FAMILY_SUFFIX}:{body}"))
}

/// Normalize raw TPS input into the body of a `tps:` FST key.
///
/// The build pipeline emits fused Bopomofo with no separators and tone-8
/// as combining dot above `U+0307` (see `dictionary/build/merge_csv.py`
/// `convert_tl_to_tps_strict(...).replace(" ", "")` and
/// `taigi-converter/src/tables.js` `ZHUYIN_TONES`). Platform keyboards
/// type the standalone modifier-letter dot `U+02D9` (see iOS
/// `TaigiLayouts.swift` + Android `tps.json`), so substitute one for the
/// other and drop the separators `-`, space and tab.
pub(crate) fn tps_key_body(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '-' | ' ' | '\t' => {}
            _ => out.push(normalize_tps_tone8_scalar(ch)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_shares_the_tl_family() {
        assert_eq!(KeyFamily::for_input_mode(InputMode::English), KeyFamily::Tl);
        assert_eq!(KeyFamily::for_input_mode(InputMode::Tl), KeyFamily::Tl);
        assert_eq!(KeyFamily::for_input_mode(InputMode::Poj), KeyFamily::Poj);
        assert_eq!(KeyFamily::for_input_mode(InputMode::Tps), KeyFamily::Tps);
    }

    #[test]
    fn prefix_is_tag_plus_colon() {
        for family in [KeyFamily::Tl, KeyFamily::Poj, KeyFamily::Tps] {
            assert_eq!(family.prefix(), format!("{}:", family.tag()));
            assert_eq!(KeyFamily::from_prefix(family.prefix()), Some(family));
        }
        assert_eq!(HANJI_KEY_PREFIX, "hanzi:");
        assert_eq!(KeyFamily::from_prefix(HANJI_KEY_PREFIX), None);
        assert_eq!(KeyFamily::from_prefix("tl-abbrev:"), None);
    }

    #[test]
    fn tl_search_key_uses_tl_prefix() {
        assert_eq!(KeyFamily::Tl.search_key("gua2"), "tl:gua2");
    }

    #[test]
    fn poj_search_key_uses_poj_prefix() {
        assert_eq!(KeyFamily::Poj.search_key("goa2"), "poj:goa2");
    }

    #[test]
    fn tps_search_key_uses_tps_prefix_for_bopomofo_with_tone() {
        // ㆤˊ — Bopomofo `ㆤ` (U+3124) + modifier-letter tone-2 `ˊ` (U+02CA).
        // Matches `tps_num` emitted by the build pipeline for row `ê`.
        assert_eq!(
            KeyFamily::Tps.search_key("\u{3124}\u{02CA}"),
            "tps:\u{3124}\u{02CA}"
        );
    }

    #[test]
    fn tps_search_key_strips_hyphens_and_spaces() {
        // User-visible separators `-` and ` ` are stripped so the key
        // matches the fused Bopomofo emitted by the pipeline.
        assert_eq!(
            KeyFamily::Tps.search_key("\u{3110}\u{3127}-\u{3124}"),
            "tps:\u{3110}\u{3127}\u{3124}"
        );
        assert_eq!(
            KeyFamily::Tps.search_key("\u{3110}\u{3127} \u{3124}"),
            "tps:\u{3110}\u{3127}\u{3124}"
        );
    }

    #[test]
    fn tps_search_key_substitutes_standalone_tone8_dot() {
        // Keyboard layouts emit modifier-letter dot `˙` (U+02D9) for tone 8;
        // build pipeline emits combining dot `̇` (U+0307). Substitute so the
        // FST exact-lookup hits.
        assert_eq!(
            KeyFamily::Tps.search_key("\u{3110}\u{3127}\u{02D9}"),
            "tps:\u{3110}\u{3127}\u{0307}"
        );
    }

    #[test]
    fn tps_search_key_preserves_combining_dot() {
        // Input already in U+0307 form (e.g. internal callers passing the
        // pipeline form) round-trips unchanged.
        assert_eq!(
            KeyFamily::Tps.search_key("\u{3110}\u{3127}\u{0307}"),
            "tps:\u{3110}\u{3127}\u{0307}"
        );
    }

    #[test]
    fn toneless_face_is_the_family_notone_column() {
        // trace: normalize_input → "gin2a2lang5", digits dropped.
        assert_eq!(KeyFamily::Tl.toneless_face("gín-á-lâng"), "ginalang");
        // trace: POJ display "chia̍h", tone mark dropped.
        assert_eq!(KeyFamily::Poj.toneless_face("tsia̍h"), "chiah");
        // Per-token: `ou → oo` never fires across the hyphen.
        assert_eq!(KeyFamily::Poj.toneless_face("tó-uī"), "toui");
        // Space-separated reading splits like a hyphenated one.
        assert_eq!(KeyFamily::Poj.toneless_face("iā sī"), "iasi");
        assert_eq!(
            KeyFamily::Tps.toneless_face("tsia̍h"),
            crate::tps_notone_from_tl("tsia̍h")
        );
    }

    #[test]
    fn abbrev_twin_of_a_phonetic_key() {
        assert_eq!(abbrev_family_key("tl:ss").as_deref(), Some("tl-abbrev:ss"));
        assert_eq!(abbrev_family_key("tl-abbrev:ss"), None);
        assert_eq!(abbrev_family_key("ss"), None);
    }
}
