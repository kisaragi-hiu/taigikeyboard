// Executable spec for the tone scheme: which keys type a tone, and the slot
// set that follows from it.

@testable import TaigiInputMethodCore
import XCTest

/// What these pin: the slot key set is DERIVED from the scheme. Which keys
/// type a tone under Telex is desktop-core's (`keys/intent.rs`, the engine's
/// `TELEX_KEYS`).
final class ToneInputSchemeTests: XCTestCase {
    func testStandard_isTheShippedScheme() {
        XCTAssertEqual(ComposingKeyBindings.default.toneScheme, .standard)
        XCTAssertEqual(ToneInputScheme.allCases.map(\.rawValue), ["standard", "telex"], "the stored spellings")
    }

    func testTheSlotKeySet_followsTheScheme() {
        XCTAssertEqual(ToneInputScheme.standard.slotKeySet, .bareKeys)
        XCTAssertEqual(ToneInputScheme.telex.slotKeySet, .digits)
        XCTAssertEqual(ComposingKeyBindings(toneScheme: .telex).slotKeySet, .digits)
    }
}
