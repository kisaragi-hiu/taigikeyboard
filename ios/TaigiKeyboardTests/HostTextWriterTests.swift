@testable import TaigiKeyboard
import UIKit
import XCTest

/// Tests for `HostTextWriter` — the proxy call sequence behind every
/// Taigi-controlled host write (issue #352). A commit over a marked region
/// must replace and unmark it, never `insertText`; inside an event the commit
/// and the literals after it must reach the host as one replacement.
final class HostTextWriterTests: XCTestCase {
    private var writer: HostTextWriter!
    private var proxy: RecordingTextDocumentProxy!

    override func setUp() {
        super.setUp()
        let proxy = RecordingTextDocumentProxy()
        self.proxy = proxy
        writer = HostTextWriter { proxy }
    }

    // MARK: - Outside an event

    func testHostTextWriter_commitWithoutMarkedText_insertsAtCaret() {
        writer.commit("，")

        XCTAssertEqual(proxy.calls, [.insert("，")])
    }

    func testHostTextWriter_commitOverMarkedText_replacesThenUnmarksWithoutInsert() {
        writer.update("Taigi")
        writer.commit("Tâi-gí")

        XCTAssertEqual(proxy.calls, [
            .setMarked("Taigi", NSRange(location: 5, length: 0)),
            .setMarked("Tâi-gí", NSRange(location: 6, length: 0)),
            .unmark,
        ])
        XCTAssertFalse(writer.hasMarkedText)
    }

    func testHostTextWriter_commitEmptyTextOverMarkedText_removesPreedit() {
        writer.update("taigi")
        proxy.calls.removeAll()

        writer.commit("")

        XCTAssertEqual(proxy.calls, [.setMarked("", NSRange(location: 0, length: 0)), .unmark])
    }

    func testHostTextWriter_updateEmptyText_leavesNoMarkedRegion() {
        writer.update("t")
        writer.update("")
        proxy.calls.removeAll()

        writer.commit("，")

        XCTAssertEqual(proxy.calls, [.insert("，")])
    }

    func testHostTextWriter_clear_removesMarkedTextWithoutCommitting() {
        writer.update("taigi")
        proxy.calls.removeAll()

        writer.clear()
        writer.commit("，")

        XCTAssertEqual(proxy.calls, [
            .setMarked("", NSRange(location: 0, length: 0)),
            .unmark,
            .insert("，"),
        ])
    }

    func testHostTextWriter_updateCaret_countsUTF16Units() {
        // trace: "𪜶" is U+2A736 — one Character, two UTF-16 units.
        writer.update("𪜶")

        XCTAssertEqual(proxy.calls, [.setMarked("𪜶", NSRange(location: 2, length: 0))])
    }

    // MARK: - Inside an event

    // INVARIANT_COMPOSING_HOST_COMMIT_ONE_WRITE_PER_EVENT (behavioral-invariants.md §13, issue #352)

    func testHostTextWriter_commitThenLiteralInEvent_writesOneReplacement() {
        // (preedit, commit, literals written after it, expected replacement)
        let cases: [(String, String, [String], String)] = [
            ("taigi", "台語", [], "台語"), // candidate tap, no auto space
            ("taigi", "taigi", [" "], "taigi "), // Space
            ("taigi", "tâi-gí", [" "], "tâi-gí "), // candidate + auto space
            ("taigi", "taigi", ["，"], "taigi，"), // punctuation while composing
            ("taigi", "taigi😀", [], "taigi😀"), // emoji / symbol (engine joins them)
        ]
        for (preedit, committed, literals, expected) in cases {
            writer.update(preedit)
            proxy.calls.removeAll()

            writer.beginEvent()
            writer.commit(committed)
            literals.forEach { writer.insert($0) }
            XCTAssertEqual(proxy.calls, [], "nothing reaches the host before the event ends: \(expected)")
            writer.endEvent()

            XCTAssertEqual(
                proxy.calls,
                [.setMarked(expected, NSRange(location: expected.utf16.count, length: 0)), .unmark],
                "one marked replacement for \(expected)",
            )
            XCTAssertFalse(writer.hasMarkedText)
            XCTAssertNil(writer.pendingCommit)
        }
    }

    func testHostTextWriter_insertInEventWithoutCommit_writesImmediately() {
        writer.beginEvent()
        writer.insert(" ")
        writer.endEvent()

        XCTAssertEqual(proxy.calls, [.insert(" ")])
    }

    func testHostTextWriter_commitWithoutMarkedTextInEvent_insertsImmediately() {
        // A leading hyphen from Idle: the engine commits with no preedit,
        // then composition begins — the insert must land before the mark.
        writer.beginEvent()
        writer.commit("-")
        writer.update("a")
        writer.endEvent()

        XCTAssertEqual(proxy.calls, [.insert("-"), .setMarked("a", NSRange(location: 1, length: 0))])
    }

    func testHostTextWriter_endEventWithoutCommit_writesNothing() {
        writer.update("taigi")
        proxy.calls.removeAll()

        writer.beginEvent()
        writer.endEvent()

        XCTAssertEqual(proxy.calls, [])
        XCTAssertTrue(writer.hasMarkedText)
    }

    // MARK: - Echo of the keyboard's own commit

    func testHostTextWriter_echoOfOwnCommit_matchesOnceWhenCommitPrecedesCaret() {
        writer.update("gua")
        writer.beginEvent()
        writer.commit("我")
        writer.endEvent()

        XCTAssertFalse(writer.isEchoOfOwnCommit(documentContextBeforeInput: nil), "nothing before the caret")
        XCTAssertFalse(writer.isEchoOfOwnCommit(documentContextBeforeInput: "你"), "caret moved elsewhere")
        XCTAssertTrue(writer.isEchoOfOwnCommit(documentContextBeforeInput: "台語我"), "the commit's own echo")
        XCTAssertFalse(writer.isEchoOfOwnCommit(documentContextBeforeInput: "台語我"), "an echo is consumed once")
    }

    func testHostTextWriter_echoOfOwnCommit_neverMatchesWithoutAMarkedReplacement() {
        // (step, document text before the caret) — none of these unmarks text.
        let steps: [(String, () -> Void)] = [
            ("insert", { self.writer.insert("我") }),
            ("commit without marked text", { self.writer.commit("我") }),
            ("clear", { self.writer.update("gua"); self.writer.clear() }),
        ]
        for (step, write) in steps {
            write()
            XCTAssertFalse(writer.isEchoOfOwnCommit(documentContextBeforeInput: "我"), step)
        }
    }

    func testHostTextWriter_insertAfterEvent_goesStraightToHost() {
        writer.update("taigi")
        writer.beginEvent()
        writer.commit("台語")
        writer.endEvent()
        proxy.calls.removeAll()

        writer.insert("，")

        XCTAssertEqual(proxy.calls, [.insert("，")])
    }
}

private final class RecordingTextDocumentProxy: NSObject, UITextDocumentProxy {
    enum Call: Equatable {
        case setMarked(String, NSRange)
        case unmark
        case insert(String)
        case deleteBackward
    }

    var calls: [Call] = []

    var documentContextBeforeInput: String? {
        nil
    }

    var documentContextAfterInput: String? {
        nil
    }

    var selectedText: String? {
        nil
    }

    var documentInputMode: UITextInputMode? {
        nil
    }

    var documentIdentifier: UUID {
        UUID()
    }

    var hasText: Bool {
        false
    }

    func adjustTextPosition(byCharacterOffset _: Int) {}

    func setMarkedText(_ markedText: String, selectedRange: NSRange) {
        calls.append(.setMarked(markedText, selectedRange))
    }

    func unmarkText() {
        calls.append(.unmark)
    }

    func insertText(_ text: String) {
        calls.append(.insert(text))
    }

    func deleteBackward() {
        calls.append(.deleteBackward)
    }
}
