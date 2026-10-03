// Which punctuation attaches to the auto space a commit left.

@testable import TaigiInputMethodCore
import XCTest

/// The Swift roster the core back end reads before it asks the client whether
/// a swap can happen (`ComposingBackend.panel`). desktop-core holds its own
/// copy (`policies/auto_space.rs`); this keeps the Swift one from drifting.
final class AutoSpacePunctuationTests: XCTestCase {
    func testAttachingSet_matchesTheCrossPlatformRoster() {
        // trace: the 19 glyphs of ios/Sources/TaigiKeyboard/Input/AutoSpacePunctuation.swift
        let attaching = ["。", "！", "？", ".", "!", "?", "，", ",", "、", "；", ";", "：", ":", ")", "）", "]", "】", "」", "』"]
        for glyph in attaching {
            XCTAssertTrue(AutoSpacePunctuation.isAttaching(glyph), "\(glyph) must attach")
        }
    }

    func testOpeningBracketsAndAmbiguousQuotes_doNotAttach() {
        for glyph in ["(", "（", "[", "「", "『", "\"", "'"] {
            XCTAssertFalse(AutoSpacePunctuation.isAttaching(glyph), "\(glyph) must not attach")
        }
    }

    func testNonPunctuationAndMultiCharacterText_doNotAttach() {
        for text in ["a", "台", " ", "", "?!", "guá?"] {
            XCTAssertFalse(AutoSpacePunctuation.isAttaching(text), "'\(text)' must not attach")
        }
    }
}
