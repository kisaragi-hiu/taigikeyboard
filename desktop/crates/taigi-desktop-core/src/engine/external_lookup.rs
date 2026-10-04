//! The two web dictionaries a search result can be looked up in — the MOE dictionary and
//! ChhoeTaigi's Taigi dictionary — and the digit-tone spelling of a TL reading their
//! query strings take. Twin of iOS `ExternalLookupURLBuilder.swift`.

use super::phonetics::{nfd_preprocess_for_lookup, strip_tone};

/// The MOE dictionary's search URL for `tl`, or `None` when the reading spells nothing.
pub fn moe_url(tl: &str) -> Option<String> {
    url(
        "https://sutian.moe.edu.tw/zh-hant/tshiau/",
        &[("lui", "tai_su")],
        "tsha",
        tl,
    )
}

/// ChhoeTaigi's search URL for `tl`.
pub fn chhoe_url(tl: &str) -> Option<String> {
    url(
        "https://chhoe.taigi.info/s",
        &[("s", "su"), ("f", "e"), ("lmjf", "ki")],
        "lmj",
        tl,
    )
}

fn url(
    base: &str,
    fixed_query: &[(&str, &str)],
    reading_parameter: &str,
    tl: &str,
) -> Option<String> {
    let digit_tone = digit_tone_form(tl);
    if digit_tone.is_empty() {
        return None;
    }
    let mut query: Vec<String> = fixed_query
        .iter()
        .map(|(name, value)| format!("{name}={}", percent_encode(value)))
        .collect();
    query.push(format!(
        "{reading_parameter}={}",
        percent_encode(&digit_tone)
    ));
    Some(format!("{base}?{}", query.join("&")))
}

/// Percent-encoding for a query value: unreserved characters kept,
/// everything else (including `+`, `/`, `&`, `=` and non-ASCII) encoded as
/// UTF-8. Stricter than Foundation's `URLQueryItem` (which leaves `+` and
/// `/` alone) — the readings this ever carries are letters, digits and
/// hyphens after `digit_tone_form`, where the two agree.
fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// A TL reading in the digit-tone spelling the web dictionaries search by:
/// lowercased, syllable by syllable (empty syllables kept so the hyphens
/// survive), the nasal marks as `nn`, the tone as a trailing digit with
/// tones 1 and 4 omitted (iOS `ExternalLookupURLBuilder.toTLDigit`).
pub fn digit_tone_form(tl: &str) -> String {
    tl.to_lowercase()
        .split('-')
        .map(syllable_in_digit_tone)
        .collect::<Vec<_>>()
        .join("-")
}

fn syllable_in_digit_tone(syllable: &str) -> String {
    if syllable.is_empty() {
        return String::new();
    }
    let with_nasal = syllable.replace(['\u{207F}', '\u{1D3A}'], "nn");
    if let Some(last) = with_nasal.chars().last().filter(char::is_ascii_digit) {
        let normalized = nfd_preprocess_for_lookup(&with_nasal).unwrap_or(with_nasal);
        return if last == '1' || last == '4' {
            normalized[..normalized.len() - 1].to_owned()
        } else {
            normalized
        };
    }
    let Some((bare, tone)) = nfd_preprocess_for_lookup(&with_nasal).and_then(|p| strip_tone(&p))
    else {
        return with_nasal;
    };
    if tone.is_empty() || tone == "1" || tone == "4" {
        bare
    } else {
        format!("{bare}{tone}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digit_tone_form_omits_tones_one_and_four_and_keeps_hyphens() {
        // trace: tâi-gí → tai5-gi2; tsia̍h → tsiah8; kau (tone 1) → kau;
        // ah4 (numeric) → ah; a leading empty syllable keeps its hyphen.
        assert_eq!(digit_tone_form("Tâi-gí"), "tai5-gi2");
        assert_eq!(digit_tone_form("tsia̍h"), "tsiah8");
        assert_eq!(digit_tone_form("kau"), "kau");
        assert_eq!(digit_tone_form("ah4"), "ah");
        assert_eq!(digit_tone_form("--ah"), "--ah");
        assert_eq!(digit_tone_form("tiⁿ"), "tinn");
    }

    /// Characterization of this per-syllable fold before it moved to the
    /// engine — the same table iOS `ExternalLookupURLBuilderTests` records.
    #[test]
    fn digit_tone_form_characterization() {
        // trace: diacritic path = nfd_preprocess_for_lookup (ⁿ→nn, NFD,
        // U+0358→o) then strip_tone (first combining tone mark, bare NFC);
        // tones 1 / 4 / none omitted.
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
            // trace: digit path = last char ASCII digit, then
            // nfd_preprocess_for_lookup (o͘ folded, output NFD), 1 / 4 dropped.
            ("ah4", "ah"),
            ("sann1", "sann"),
            ("TSIT8", "tsit8"),
            ("ho\u{358}2", "hoo2"),
            ("t\u{E2}i5", "ta\u{302}i5"),
            ("ta\u{302}i5", "ta\u{302}i5"),
            // A full-width digit is not ASCII: the diacritic path.
            ("ho\u{FF12}", "ho\u{FF12}"),
            ("h\u{F3}\u{FF12}", "ho\u{FF12}2"),
        ] {
            assert_eq!(digit_tone_form(reading), expected, "{reading:?}");
        }
    }

    #[test]
    fn the_two_urls_carry_the_fixed_query_and_the_encoded_reading() {
        assert_eq!(
            moe_url("Tâi-gí").as_deref(),
            Some("https://sutian.moe.edu.tw/zh-hant/tshiau/?lui=tai_su&tsha=tai5-gi2")
        );
        assert_eq!(
            chhoe_url("tsia̍h").as_deref(),
            Some("https://chhoe.taigi.info/s?s=su&f=e&lmjf=ki&lmj=tsiah8")
        );
        assert_eq!(moe_url(""), None);
    }
}
