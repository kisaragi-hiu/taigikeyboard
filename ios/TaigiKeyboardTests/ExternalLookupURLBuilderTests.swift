@testable import TaigiKeyboard
import XCTest

/// The digit-tone reading the external-dictionary URLs query by, through the
/// engine (`RustEngineBridge.externalLookupDigitForm`; the full table is
/// pinned in engine `phonetics/src/external_lookup.rs`). Compared scalar by
/// scalar: Swift `String` equality is canonical, so `==` cannot tell NFC
/// from NFD.
final class ExternalLookupURLBuilderTests: XCTestCase {
    private func assertDigitForm(
        _ reading: String,
        _ expected: String,
        file: StaticString = #filePath,
        line: UInt = #line,
    ) {
        XCTAssertEqual(
            Array(RustEngineBridge.externalLookupDigitForm(reading).unicodeScalars),
            Array(expected.unicodeScalars),
            "\(reading.unicodeScalars.map { String($0.value, radix: 16) })",
            file: file,
            line: line,
        )
    }

    func testDigitForm_throughTheEngine() {
        // trace: engine `digit_tone_form` — base form (ⁿ→nn, NFD, U+0358→o),
        // first tone mark → digit, tones 1 / 4 / none omitted, NFC.
        assertDigitForm("Tâi-gí", "tai5-gi2")
        assertDigitForm("ho\u{0301}\u{0358}", "hoo2")
        assertDigitForm("iā sī", "ia sī7")
        assertDigitForm("sann1", "sann")
        assertDigitForm("", "")
    }

    /// Parity-corrections against the Swift copy (characterized before the
    /// move): the digit path now folds `o͘`, answers NFC, and takes only an
    /// ASCII digit as a tone.
    func testDigitForm_parityCorrections() {
        assertDigitForm("ho\u{0358}2", "hoo2") // was ho͘2
        assertDigitForm("ta\u{0302}i5", "t\u{E2}i5") // was NFD, passed through
        assertDigitForm("h\u{F3}\u{FF12}", "ho\u{FF12}2") // was hó２
    }

    func testURLs_carryTheEncodedDigitForm() {
        XCTAssertEqual(
            ExternalLookupURLBuilder.moeURL(forTL: "Tâi-gí")?.absoluteString,
            "https://sutian.moe.edu.tw/zh-hant/tshiau/?lui=tai_su&tsha=tai5-gi2",
        )
        XCTAssertEqual(
            ExternalLookupURLBuilder.chhoeURL(forTL: "tsia\u{030D}h")?.absoluteString,
            "https://chhoe.taigi.info/s?s=su&f=e&lmjf=ki&lmj=tsiah8",
        )
        XCTAssertNil(ExternalLookupURLBuilder.moeURL(forTL: ""))
    }
}
