@testable import TaigiKeyboard
import XCTest

/// The digit-tone reading the external-dictionary URLs query by
/// (`ExternalLookupURLBuilder.toTLDigit`). Compared scalar by scalar: Swift
/// `String` equality is canonical, so `==` cannot tell NFC from NFD.
final class ExternalLookupURLBuilderTests: XCTestCase {
    private func assertDigitForm(
        _ reading: String,
        _ expected: String,
        file: StaticString = #filePath,
        line: UInt = #line,
    ) {
        XCTAssertEqual(
            Array(ExternalLookupURLBuilder.toTLDigit(reading).unicodeScalars),
            Array(expected.unicodeScalars),
            "\(reading.unicodeScalars.map { String($0.value, radix: 16) })",
            file: file,
            line: line,
        )
    }

    /// Characterization of the Swift per-syllable fold before it moved to the
    /// engine — the same table desktop-core `external_lookup.rs` records.
    func testDigitForm_characterization() {
        // trace: diacritic path = nfdPreprocessForLookup (ⁿ→nn, NFD,
        // U+0358→o) then stripTone (first combining tone mark, bare NFC);
        // tones 1 / 4 / none omitted.
        assertDigitForm("Tâi-gí", "tai5-gi2")
        assertDigitForm("tsi\u{030D}t-ê", "tsit8-e5")
        assertDigitForm("kiaⁿ", "kiann")
        assertDigitForm("kiânn", "kiann5")
        assertDigitForm("ho\u{0301}\u{0358}", "hoo2")
        assertDigitForm("ho\u{0358}\u{0301}", "hoo2")
        assertDigitForm("tâí", "taí5") // only the first tone mark is stripped
        assertDigitForm("iā sī", "ia sī7") // split on `-` only, never on the space
        assertDigitForm("--ah", "--ah")
        assertDigitForm("台語", "台語")
        assertDigitForm("", "")
        // trace: digit path = last Character `isNumber`, ⁿ→nn only, 1 / 4
        // dropped, NO NFD and NO o͘ fold.
        assertDigitForm("ah4", "ah")
        assertDigitForm("sann1", "sann")
        assertDigitForm("TSIT8", "tsit8")
        assertDigitForm("ho\u{0358}2", "ho\u{0358}2")
        assertDigitForm("t\u{E2}i5", "t\u{E2}i5")
        assertDigitForm("ta\u{0302}i5", "ta\u{0302}i5")
        assertDigitForm("ho\u{FF12}", "ho\u{FF12}")
        assertDigitForm("h\u{F3}\u{FF12}", "h\u{F3}\u{FF12}")
    }
}
