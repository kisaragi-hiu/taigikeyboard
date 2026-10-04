//! The two web dictionaries a search result can be looked up in — the MOE dictionary and
//! ChhoeTaigi's Taigi dictionary. The digit-tone spelling of a TL reading their query
//! strings take is the engine's (`external_lookup_digit_form`); the URLs are this
//! platform's. Twin of iOS `ExternalLookupURLBuilder.swift`.

use super::phonetics::external_lookup_digit_form;

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
    // An engine failure looks the reading up as written (logged by the bridge).
    let digit_tone = external_lookup_digit_form(tl).unwrap_or_else(|| tl.to_owned());
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
/// hyphens after the digit-tone fold, where the two agree.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_urls_carry_the_fixed_query_and_the_engines_digit_tone_reading() {
        // trace: engine `external_lookup::digit_tone_form` — Tâi-gí →
        // tai5-gi2, tsia̍h → tsiah8 (the full table is pinned there).
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
