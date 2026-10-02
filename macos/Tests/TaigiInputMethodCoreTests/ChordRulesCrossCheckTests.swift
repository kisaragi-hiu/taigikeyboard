// The Shortcuts pane's chord rules against the table desktop-core is held to.

import AppKit
@testable import TaigiInputMethodCore
import XCTest

/// The Swift half of the chord-rule cross-check: every row of
/// `macos/crates/taigi-macos-ffi/fixtures/chord_rules.tsv` read by the Swift
/// codec, matcher and resolver the Shortcuts pane uses, against the same
/// hand-written expectations the core's `chord_rules.rs` meets. A row an open
/// parity item splits (E6, E7) holds the Mac to its third column and the core
/// to its fourth, so the difference is listed, not skipped. Retired when the
/// pane reads its rules from the core (roadmap P14).
@MainActor
final class ChordRulesCrossCheckTests: XCTestCase {
    func testTheSwiftRulesAnswerEveryRowOfTheMacChordTable() throws {
        let table = try String(contentsOf: TestFixtures.chordRulesURL, encoding: .utf8)
        let rows = table.split(separator: "\n")
            .filter { !$0.isEmpty && !$0.hasPrefix("#") }
            .map { $0.split(separator: "\t", omittingEmptySubsequences: false).map(String.init) }
        var kinds: [String: Int] = [:]
        for row in rows {
            kinds[row[0], default: 0] += 1
            switch row[0] {
            case "default":
                let action = try XCTUnwrap(ComposingAction(rawValue: row[1]))
                XCTAssertEqual(action.defaultChord.rawValue, row[2], "\(row)")
            case "parse":
                XCTAssertEqual(ComposingKeyChord(rawValue: row[1])?.rawValue ?? "refused", row[2], "\(row)")
            case "match":
                let chord = try XCTUnwrap(ComposingKeyChord(rawValue: row[1]))
                let characters = try Self.scalars(row[2])
                let event = KeyEventSnapshot(
                    characters: characters,
                    modifiers: Self.modifiers(row[3]),
                    isNamedSpecialKey: false,
                )
                XCTAssertEqual(chord.matches(event) ? "yes" : "no", row[4], "\(row)")
            case "resolve":
                XCTAssertEqual(try bindings(row[1]), row[2], "\(row)")
            default:
                XCTFail("unknown row kind \(row[0])")
            }
        }
        XCTAssertEqual(kinds["default"], ComposingAction.allCases.count, "every action has its default row")
        XCTAssertEqual(kinds.count, 4)
    }

    /// The rows as `SettingsStore.composingKeyBindings` resolves them from a
    /// defaults domain holding `stored`.
    private func bindings(_ stored: String) throws -> String {
        let store = try makeScratchSettingsStore()
        if stored != "-" {
            for assignment in stored.split(separator: ";") {
                let parts = assignment.split(separator: "=", maxSplits: 1, omittingEmptySubsequences: false)
                let action = try XCTUnwrap(ComposingAction(rawValue: String(parts[0])))
                store.userDefaults.set(String(parts[1]), forKey: action.settingsKeyName)
            }
        }
        let bindings = store.composingKeyBindings
        return ComposingAction.allCases
            .map { "\($0.rawValue)=\(bindings.chord(for: $0)?.rawValue ?? "-")" }
            .joined(separator: ";")
    }

    private static func modifiers(_ letters: String) -> NSEvent.ModifierFlags {
        var flags: NSEvent.ModifierFlags = []
        if letters.contains("d") {
            flags.insert(.command)
        }
        if letters.contains("c") {
            flags.insert(.control)
        }
        if letters.contains("o") {
            flags.insert(.option)
        }
        if letters.contains("s") {
            flags.insert(.shift)
        }
        return flags
    }

    private static func scalars(_ hex: String) throws -> String {
        var text = ""
        for field in hex.split(separator: ",") {
            let value = try XCTUnwrap(UInt32(field, radix: 16))
            try text.unicodeScalars.append(XCTUnwrap(UnicodeScalar(value)))
        }
        return text
    }
}
