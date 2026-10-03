// Writes into one IMK client: marked text in, committed text out.

import InputMethodKit

/// Writes the composition and its commits into the client that is currently
/// focused.
///
/// The composition lives entirely in the client's marked region until a
/// commit, which is what makes an abort free: nothing was ever written to the
/// document, so nothing has to be taken back.
@MainActor
struct ClientWriter {
    /// "Wherever the insertion point is." Both IMK text calls read a location of
    /// `NSNotFound` as "use the current selection" and ignore the range
    /// entirely for clients without `TSMDocumentAccess`
    /// (`IMKInputSession.h:66-72` for `insertText`, `:85-87` for
    /// `setMarkedText`), which is exactly the behaviour an inline composition
    /// wants — the client owns caret placement, we never move it.
    private static let atInsertionPoint = NSRange(location: NSNotFound, length: NSNotFound)

    private let client: IMKTextInput

    init(client: IMKTextInput) {
        self.client = client
    }

    /// The composition, marked.
    func setMarkedText(_ text: String, caretUTF16: Int) {
        client.setMarkedText(
            NSAttributedString(string: text, attributes: Self.markedTextAttributes),
            // Collapsed where the engine says the caret is — the end unless
            // the user stepped it back. Length 0, and one underline style over
            // the whole region: in vChewing's experience IMK shows a caret
            // inside marked text only under both
            // (`InputSession_HandleStates.swift:117-129`). UTF-16 because that
            // is the unit `NSRange` counts in, and what the engine sends.
            selectionRange: NSRange(location: caretUTF16, length: 0),
            replacementRange: Self.atInsertionPoint,
        )
    }

    /// An empty marked string is how IMK is told the composition ended
    /// without producing text; the client drops the marked region.
    func clearMarkedText() {
        client.setMarkedText(
            "",
            selectionRange: NSRange(location: 0, length: 0),
            replacementRange: Self.atInsertionPoint,
        )
    }

    /// No `setMarkedText("")` first: `insertText` already replaces the active
    /// marked region (`IMKInputSession.h:63-74`). Clearing first would be a
    /// second document mutation for one keystroke, which clients render as a
    /// visible flicker and undo as two steps.
    func insertText(_ text: String) {
        client.insertText(text, replacementRange: Self.atInsertionPoint)
    }

    /// Underlined, single clause. Mirrors the marked-text styling every
    /// mainstream macOS IME uses for an unconverted composition — McBopomofo
    /// `references/McBopomofo/Source/InputState.swift:340-345`. Passing a plain
    /// `String` would also underline it, but only with IMK's default styling,
    /// and the clause segment is what tells the client this is one unit rather
    /// than a run of unrelated characters. Hoisted because the styling never
    /// varies, and this runs once per keystroke.
    private static let markedTextAttributes: [NSAttributedString.Key: Any] = [
        .underlineStyle: NSUnderlineStyle.single.rawValue,
        .markedClauseSegment: 0,
    ]
}
