// What each write does to the client document.

import InputMethodKit
@testable import TaigiInputMethodCore
import XCTest

@MainActor
final class ClientWriterTests: XCTestCase {
    func testSetMarkedText_marksTheTextUnderlinedWithTheCaretAtTheEnd() {
        let client = RecordingTextInputClient()

        ClientWriter(client: client).setMarkedText("tâi", caretUTF16: 3)

        XCTAssertEqual(
            client.writes,
            [.setMarkedText("tâi", selectionLocation: 3)],
            "the composition must be marked, not inserted, and the caret sits after it",
        )
        XCTAssertEqual(
            client.lastMarkedTextAttributes[.underlineStyle] as? Int,
            NSUnderlineStyle.single.rawValue,
            "an unconverted composition is underlined — without it the user cannot tell it is provisional",
        )
        XCTAssertEqual(
            client.lastMarkedTextAttributes[.markedClauseSegment] as? Int,
            0,
            "the composition is one clause, not a run of unrelated characters",
        )
    }

    /// The engine's caret is where the client's insertion point goes — not
    /// the end of the marked text.
    func testSetMarkedText_putsTheCaretWhereTheEngineSaysItIs() {
        let client = RecordingTextInputClient()

        ClientWriter(client: client).setMarkedText("khá", caretUTF16: 2)

        XCTAssertEqual(client.writes, [.setMarkedText("khá", selectionLocation: 2)])
    }

    func testInsertText_writesOnceAndDoesNotClearTheMarkedRegionFirst() {
        let client = RecordingTextInputClient()
        let writer = ClientWriter(client: client)

        writer.setMarkedText("tâi", caretUTF16: 3)
        writer.insertText("台")

        XCTAssertEqual(
            client.writes,
            [.setMarkedText("tâi", selectionLocation: 3), .insertText("台")],
            """
            `insertText` already replaces the marked region, so committing must be one \
            document mutation — an extra clearing write flickers and undoes in two steps
            """,
        )
    }

    func testClearMarkedText_removesTheMarkedRegionWithoutWritingText() {
        let client = RecordingTextInputClient()

        ClientWriter(client: client).clearMarkedText()

        XCTAssertEqual(
            client.writes,
            [.setMarkedText("", selectionLocation: 0)],
            "an abort must leave the document untouched",
        )
    }
}
