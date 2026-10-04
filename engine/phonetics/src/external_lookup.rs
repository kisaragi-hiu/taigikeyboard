//! The digit-tone spelling of a TL reading that the external web
//! dictionaries (the MOE dictionary, ChhoeTaigi) search by — the one copy
//! every platform asks (`ExternalLookupDigitForm`). Each platform keeps its
//! own URL assembly (hosts, query parameters, percent-encoding).

use unicode_normalization::UnicodeNormalization;

use crate::normalization::taigi_unicode_base_form;
use crate::syllable::strip_tone_mark;

/// `reading` lowercased and folded syllable by syllable, split on `-` only
/// (empty syllables kept, so the hyphens survive; a space stays inside its
/// syllable): the nasal marks as `nn`, POJ `o͘` as `oo`, the tone as a
/// trailing ASCII digit with tones 1 and 4 omitted, NFC throughout.
/// `tāi-tsì` → `tai7-tsi3`.
pub(crate) fn digit_tone_form(reading: &str) -> String {
    reading
        .to_lowercase()
        .split('-')
        .map(syllable_in_digit_tone)
        .collect::<Vec<_>>()
        .join("-")
}

fn syllable_in_digit_tone(syllable: &str) -> String {
    let base = taigi_unicode_base_form(syllable);
    // A syllable typed with its tone digit keeps it — the digit wins over
    // any tone mark also on it.
    let (bare, tone) = match base.chars().last().filter(char::is_ascii_digit) {
        Some(digit) => (base[..base.len() - 1].nfc().collect(), digit.to_string()),
        None => strip_tone_mark(&base),
    };
    if is_omitted_tone(&tone) {
        bare
    } else {
        format!("{bare}{tone}")
    }
}

/// The web dictionaries' query convention: no digit for tone 1 (open) or
/// 4 (checked), nor for a syllable that carries no tone.
fn is_omitted_tone(tone: &str) -> bool {
    matches!(tone, "" | "1" | "4")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The contract, against the three platform copies it replaced
    /// (characterized in iOS `ExternalLookupURLBuilderTests` and desktop-core
    /// `external_lookup.rs` before the move; Android ran the iOS algorithm).
    #[test]
    fn digit_tone_form_matches_the_platform_copies() {
        // trace: base form (ⁿ→nn, NFD, U+0358→o) then strip_tone_mark (first
        // combining tone mark, bare NFC); tones 1 / 4 / none omitted —
        // identical on all three before the move.
        for (reading, expected) in [
            ("Tâi-gí", "tai5-gi2"),
            ("tsi\u{30D}t-ê", "tsit8-e5"),
            ("kiaⁿ", "kiann"),
            ("kiânn", "kiann5"),
            ("ho\u{301}\u{358}", "hoo2"),
            ("ho\u{358}\u{301}", "hoo2"),
            ("tâí", "taí5"),
            ("iā sī", "ia sī7"),
            ("--ah", "--ah"),
            ("台語", "台語"),
            ("", ""),
            ("ah4", "ah"),
            ("sann1", "sann"),
            ("TSIT8", "tsit8"),
            ("ho\u{FF12}", "ho\u{FF12}"),
        ] {
            assert_eq!(digit_tone_form(reading), expected, "{reading:?}");
        }
    }

    /// Parity-corrections: inputs on which the copies disagreed. Only a
    /// mobile custom-dictionary entry can carry them (system rows are
    /// tone-marked TL; desktop custom rows have no lookup).
    #[test]
    fn digit_tone_form_parity_corrections() {
        for (reading, expected, before) in [
            // o͘ folded on the digit path too (desktop did; mobile kept `o͘`).
            ("ho\u{358}2", "hoo2", "mobile ho\u{358}2"),
            // NFC on the digit path (desktop answered NFD, mobile passed
            // decomposed input through).
            ("t\u{E2}i5", "t\u{E2}i5", "desktop ta\u{302}i5"),
            ("ta\u{302}i5", "t\u{E2}i5", "mobile + desktop ta\u{302}i5"),
            // Only an ASCII digit is a tone digit (desktop's rule; iOS
            // `isNumber` / Android `isDigit` also took `２`).
            ("h\u{F3}\u{FF12}", "ho\u{FF12}2", "mobile h\u{F3}\u{FF12}"),
        ] {
            assert_eq!(
                digit_tone_form(reading),
                expected,
                "{reading:?} (was {before})"
            );
        }
    }
}
