// Sole writer of Taigi-controlled host text: the marked composition, commits, literal inserts and deletes.

import UIKit

/// Every proxy write the keyboard makes on its own behalf goes through here
/// (KeyboardKit's fall-through actions still write directly, after an event
/// has finished). Two host behaviours shape it (issue #352):
///
/// - A commit over the marked region uses Apple's documented flow — replace
///   the marked text, then `unmarkText()` — never `insertText`: Flutter
///   before flutter/flutter#191062 inserts at the caret and keeps the marked
///   text as committed text.
/// - Observed on iOS 27 (not documented by Apple): the proxy calls made in one
///   main-thread turn reach the host merged — all inserted text, then only the
///   last `setMarkedText`, then the unmark. A commit followed by a literal in
///   the same turn therefore lands out of order (UIKit puts the Space before
///   the word, Flutter duplicates the word). So inside an input event the
///   commit is held, the literals written after it are appended to it, and
///   `endEvent()` writes the whole thing as one marked replacement.
final class HostTextWriter {
    private let proxy: () -> UITextDocumentProxy
    private let logger = DebugLogger(category: "HostTextWriter")

    /// A stale `true` only turns the next commit into replace-and-unmark at
    /// the caret, which inserts the same text; a stale `false` brings #352
    /// back. So only the writer's own calls clear it — a field switch does
    /// not: the engine's generation bump already drops the old composition,
    /// and the field-switch detector is not trusted to fire only on switches.
    private(set) var hasMarkedText = false

    /// The commit made over the marked region during the current event, plus
    /// every literal written after it. The marked region stays on screen
    /// until `endEvent()` replaces it with this text.
    private(set) var pendingCommit: String?

    /// The text the last marked replacement committed, until the host's
    /// echo of it (`isEchoOfOwnCommit`) has been seen.
    private var unechoedCommit: String?

    private var isInEvent = false

    init(proxy: @escaping () -> UITextDocumentProxy) {
        self.proxy = proxy
    }

    // MARK: - Event scope

    func beginEvent() {
        assert(!isInEvent, "HostTextWriter.beginEvent: event already open")
        isInEvent = true
    }

    /// Writes the held commit, if any, as one marked replacement.
    func endEvent() {
        isInEvent = false
        guard let text = pendingCommit else { return }
        pendingCommit = nil
        replaceMarkedText(with: text)
    }

    /// Whether a host text-change callback is the echo of the keyboard's own
    /// commit rather than the user editing. UIKit hosts report `unmarkText()`
    /// of non-empty marked text as a text change (~50 ms later, observed iOS
    /// 27); the echo leaves the committed text right before the caret. Seen
    /// once, the echo is consumed. azooKey matches its own edits the same way
    /// (`ExpectedEditTracker`), with full before/after snapshots.
    func isEchoOfOwnCommit(documentContextBeforeInput: String?) -> Bool {
        guard let committed = unechoedCommit,
              documentContextBeforeInput?.hasSuffix(committed) == true
        else { return false }
        unechoedCommit = nil
        return true
    }

    // MARK: - Composition

    /// Show `text` as the whole marked region, caret at its end. **Model B**:
    /// `text` is the whole composition (`Σ nailed.display_text` + derived
    /// pending tail); the host renders it as one marked region until a hard
    /// finalize.
    func update(_ text: String) {
        writeHeldCommitBeforeUnsupportedWrite("update")
        proxy().setMarkedText(text, selectedRange: NSRange(location: text.utf16.count, length: 0))
        hasMarkedText = !text.isEmpty
    }

    /// Remove the whole marked region (nailed + pending) without committing
    /// it — nailed segments were never literal document text. Two steps are
    /// required by UITextInput.
    func clear() {
        writeHeldCommitBeforeUnsupportedWrite("clear")
        let proxy = proxy()
        proxy.setMarkedText("", selectedRange: NSRange(location: 0, length: 0))
        proxy.unmarkText()
        hasMarkedText = false
    }

    /// Commit `text` in place of the marked region, or insert it at the caret
    /// when there is none. Inside an event the replacement is held until
    /// `endEvent()`.
    func commit(_ text: String) {
        if pendingCommit != nil || !hasMarkedText {
            insert(text)
        } else if isInEvent {
            pendingCommit = text
        } else {
            replaceMarkedText(with: text)
        }
    }

    // MARK: - Literal text

    /// Insert `text` at the caret — appended to a held commit, so the commit
    /// and the literal reach the host as one write.
    func insert(_ text: String) {
        if let held = pendingCommit {
            pendingCommit = held + text
        } else {
            proxy().insertText(text)
        }
    }

    func deleteBackward() {
        writeHeldCommitBeforeUnsupportedWrite("deleteBackward")
        proxy().deleteBackward()
    }

    // MARK: - Private

    /// A held commit only takes trailing inserts: no event commits and then
    /// re-marks, clears or deletes. Should one start to, writing the commit
    /// first keeps UIKit hosts correct, but the host merges the two writes
    /// (see the type comment), so the assertion catches it in tests first.
    private func writeHeldCommitBeforeUnsupportedWrite(_ operation: String) {
        guard let text = pendingCommit else { return }
        assertionFailure("HostTextWriter.\(operation) while a commit is held")
        logger.error("[HOST] event=held_commit_unsupported_write op=\(operation)")
        pendingCommit = nil
        replaceMarkedText(with: text)
    }

    private func replaceMarkedText(with text: String) {
        let proxy = proxy()
        proxy.setMarkedText(text, selectedRange: NSRange(location: text.utf16.count, length: 0))
        proxy.unmarkText()
        hasMarkedText = false
        unechoedCommit = text.isEmpty ? nil : text
    }
}
