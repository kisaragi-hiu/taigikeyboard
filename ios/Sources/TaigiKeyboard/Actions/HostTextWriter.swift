// Single writer of the keyboard's own host text: the marked composition, commits, literal inserts and deletes.

import UIKit

/// The keyboard's own proxy writes go through here (KeyboardKit's
/// fall-through actions, which run after an event has finished, and the
/// UIResponder text-input overrides still write directly). Two host
/// behaviours shape it (issue #352, `behavioral-invariants.md`
/// `INVARIANT_composing_host_commit_one_write_per_event`):
///
/// - A commit over the marked region uses Apple's documented flow — replace
///   the marked text, then `unmarkText()` — never `insertText`: Flutter
///   before flutter/flutter#191062 inserts at the caret and keeps the marked
///   text as committed text.
/// - Observed on iOS 27 (not documented by Apple): the proxy calls made in one
///   main-thread turn reach the host merged — all inserted text, then only the
///   last `setMarkedText`, then the unmark. So inside an input event the
///   commit is held, the literals written after it are appended to it, and
///   `endEvent()` writes the whole thing as one marked replacement.
final class HostTextWriter {
    /// How long after a commit the host's `textDidChange` echo of it is
    /// expected. Observed ~50–70 ms on the iOS 27 simulator.
    static let commitEchoWindow: TimeInterval = 0.5

    private let proxy: () -> UITextDocumentProxy
    private let now: () -> TimeInterval
    private let logger = DebugLogger(category: "HostTextWriter")

    /// A stale `true` only turns the next commit into replace-and-unmark at
    /// the caret (or over the selection), which inserts the same text; a stale
    /// `false` brings #352 back. So only the writer's own calls clear it — a
    /// field switch does not: the engine's generation bump already drops the
    /// old composition, and the field-switch detector is not trusted to fire
    /// only on switches.
    private(set) var hasMarkedText = false

    private(set) var isInEvent = false

    /// The commit made over the marked region during the current event, plus
    /// every literal written after it. The marked region stays on screen
    /// until `endEvent()` replaces it with this text.
    private var pendingCommit: String?

    /// What the host should report right after the last marked replacement,
    /// until one text-change callback has been checked against it.
    private var expectedEcho: (committed: String, textAfterCaret: String?, writtenAt: TimeInterval)?

    init(
        proxy: @escaping () -> UITextDocumentProxy,
        now: @escaping () -> TimeInterval = { ProcessInfo.processInfo.systemUptime },
    ) {
        self.proxy = proxy
        self.now = now
    }

    // MARK: - Event scope

    func beginEvent() {
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
    /// commit rather than a real edit. UIKit hosts report `unmarkText()` of
    /// non-empty marked text as a text change; the commit it replaced
    /// (`insertText`) raised none. An echo arrives within
    /// `commitEchoWindow`, with the commit right before the caret and the
    /// text after the caret untouched — the before/after match azooKey's
    /// `ExpectedEditTracker` makes. Only the first callback after a commit is
    /// checked, so a host that never echoes (Flutter) leaves nothing behind.
    func isEchoOfOwnCommit() -> Bool {
        guard let expected = expectedEcho else { return false }
        expectedEcho = nil
        guard now() - expected.writtenAt <= Self.commitEchoWindow else { return false }
        let proxy = proxy()
        return proxy.documentContextBeforeInput?.hasSuffix(expected.committed) == true
            && proxy.documentContextAfterInput == expected.textAfterCaret
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

    /// A held commit only takes trailing inserts; no event commits and then
    /// re-marks, clears or deletes. Should one start to, the assertion
    /// catches it in tests; release writes the commit first.
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
        expectedEcho = text.isEmpty ? nil : (text, proxy.documentContextAfterInput, now())
    }
}
