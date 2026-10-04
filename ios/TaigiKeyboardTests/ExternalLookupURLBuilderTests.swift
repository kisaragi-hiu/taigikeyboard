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
        // trace: engine `digit_tone_form` — first tone mark → digit, tones
        // 1 / 4 / none omitted; `ho͘2` is the parity-correction against the
        // Swift copy (it left the digit path unfolded).
        assertDigitForm("Tâi-gí", "tai5-gi2")
        assertDigitForm("ho\u{0358}2", "hoo2")
        assertDigitForm("", "")
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
