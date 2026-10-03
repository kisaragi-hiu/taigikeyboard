// The key-rule requests from the Swift side: each answer decoded into what
// the recorder, the pane and the picker act on.

import Carbon.HIToolbox
@testable import TaigiInputMethodCore
import XCTest

/// The rules are desktop-core's and pinned there under `MacOS`; these cases
/// hold the Swift half — the event as it is sent, the answer as it is decoded
/// — through the real seam (`KeyRules`).
@MainActor
final class KeyRulesTests: XCTestCase {
    private func key(
        _ characters: String,
        modifiers: NSEvent.ModifierFlags = [],
        keyCode: Int? = nil,
        specialKey: UInt32? = nil,
    ) -> KeyEventSnapshot {
        KeyEventSnapshot(
            characters: characters,
            modifiers: modifiers,
            isNamedSpecialKey: specialKey != nil,
            keyCode: keyCode.map(UInt16.init),
            specialKeyRawValue: specialKey,
        )
    }

    // MARK: - Press

    /// Every outcome a recording field acts on: a chord, a refusal, the way
    /// out, the blank.
    func testEachPressOutcome_decodes() throws {
        XCTAssertEqual(KeyRules.press(key("]")), try .recorded(TestFixtures.chord("]")))
        XCTAssertEqual(KeyRules.press(key("a")), .refused(.typesRomanization))
        XCTAssertEqual(KeyRules.press(key("\u{1B}", keyCode: kVK_Escape)), .blur)
        XCTAssertEqual(KeyRules.press(key("\u{F728}", specialKey: 0xF728)), .blank)
        XCTAssertEqual(KeyRules.press(key("\u{7F}", specialKey: 0x7F)), .blank)
    }

    // MARK: - Symbol picker

    /// The picker's intents as the controller switches on them, read under
    /// the bindings and tone scheme of the store they are asked with.
    func testEachPickerIntent_decodes() throws {
        let store = try makeScratchSettingsStore()
        _ = try TestFixtures.composingShortcuts(in: store)
        func intent(_ key: KeyEventSnapshot) -> SymbolPickerIntent? {
            KeyRules.symbolPickerIntent(for: key, in: store.userDefaults)
        }

        XCTAssertEqual(intent(key("\u{1B}")), .close)
        XCTAssertEqual(intent(key("\r", specialKey: 0x0D)), .confirm)
        XCTAssertEqual(intent(key(" ")), .confirm, "the Hanji/romanization row has no other script here")
        XCTAssertEqual(intent(key("]")), .navigate(.pageDown))
        XCTAssertEqual(intent(key("\u{F701}", specialKey: 0xF701)), .navigate(.down))
        XCTAssertEqual(intent(key("a")), .closeAndPassThrough)

        store.userDefaults.set("o|000D", forKey: "composingShortcut.pageForward")
        XCTAssertEqual(intent(key("\r", modifiers: .option, specialKey: 0x0D)), .navigate(.pageDown))
        XCTAssertEqual(intent(key("]")), .closeAndPassThrough, "the row moved")
    }

    /// The key the window draws beside slot `i` is the key the core picks
    /// slot `i` with, under each scheme — read through the picker, which asks
    /// the core's slot key set.
    func testEveryDrawnSlotLabel_picksItsSlot() throws {
        for scheme in ToneInputScheme.allCases {
            let store = try makeScratchSettingsStore()
            store.userDefaults.set(scheme.rawValue, forKey: SettingsStore.Keys.toneInputScheme.name)
            _ = try TestFixtures.composingShortcuts(in: store)
            for slot in 0 ..< HorizontalPageLayout.pageSize {
                let label = scheme.slotKeySet.label(forSlot: slot)
                XCTAssertEqual(
                    KeyRules.symbolPickerIntent(for: key(label), in: store.userDefaults),
                    .pickSlot(slot),
                    "\(scheme) slot \(slot) (`\(label)`)",
                )
            }
        }
    }
}
