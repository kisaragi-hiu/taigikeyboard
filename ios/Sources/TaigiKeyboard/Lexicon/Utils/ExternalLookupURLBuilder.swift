import Foundation

/// Builds external dictionary lookup URLs from Taiwanese TL forms.
///
/// Third-party dictionaries (Chhoe Taigi, MOE) expect *digit-toned* TL
/// syllables (e.g. `tai7-tsi3`), while our search results carry the
/// diacritic display form (`tāi-tsì`). The engine owns that conversion
/// (`RustEngineBridge.externalLookupDigitForm`); this helper keeps the
/// percent-encoding so the lookup call sites stay trivial.
///
/// Platform-owned by design — which query parameters each dictionary takes
/// and the percent-encoding rules belong with the platform that knows about
/// `URLQueryAllowed` / `URLEncoder`. Mirror at
/// `android/.../ime/dictionary/ExternalLookupURLBuilder.kt`.
enum ExternalLookupURLBuilder {
    /// Chhoe Taigi dictionary lookup URL for the given TL display form.
    static func chhoeURL(forTL tl: String) -> URL? {
        guard let encoded = encodedTLDigit(tl) else { return nil }
        return URL(string: "https://chhoe.taigi.info/s?s=su&f=e&lmjf=ki&lmj=\(encoded)")
    }

    /// MOE Sutian dictionary lookup URL for the given TL display form.
    static func moeURL(forTL tl: String) -> URL? {
        guard let encoded = encodedTLDigit(tl) else { return nil }
        return URL(string: "https://sutian.moe.edu.tw/zh-hant/tshiau/?lui=tai_su&tsha=\(encoded)")
    }

    private static func encodedTLDigit(_ tl: String) -> String? {
        let digit = RustEngineBridge.externalLookupDigitForm(tl)
        guard !digit.isEmpty else { return nil }
        return digit.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed)
    }
}
