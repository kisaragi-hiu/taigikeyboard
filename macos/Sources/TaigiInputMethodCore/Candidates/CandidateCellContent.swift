// What one candidate cell shows: the primary script, and the other one beside it.

import Foundation

/// One cell as the window renders it — both scripts, in the order the user's
/// swap setting puts them; or one script alone: the romanization under the
/// romanization-only display, and either script by itself under Hanji with Romanization,
/// where a candidate is two adjacent cells. desktop-core builds the cells
/// (`desktop/crates/taigi-desktop-core/src/composing/presentation.rs`).
///
/// A Taigi candidate is a `(Hanji, romanization)` pair (Core Principle #7), and showing
/// only one of them makes several candidates read identically: two Hanji with
/// the same reading, or one Hanji under two readings. iOS and Android have
/// always shown both — primary text with the other script under it
/// (`ios/Sources/TaigiKeyboard/Autocomplete/Views/CandidateButtonView.swift:65-77`)
/// — and this is the macOS counterpart, rendered as MacishType's annotation
/// column rather than a second line because the window is one row tall.
///
/// Display only. What committing writes into the document is the engine's
/// decision (`engine/composing/src/commit_text.rs`): the two agree on which
/// script leads, while the cell may keep the other one in its own column.
struct CandidateCellContent: Equatable, Sendable {
    /// The script this cell leads with — romanization, or Hanji when swapped.
    let text: String

    /// The other script, shown smaller beside `text`, or nil for a candidate
    /// that has only one (a romanization-only candidate has no Hanji).
    let annotation: String?

    /// An empty annotation is normalized to nil so the cell reserves no width
    /// for it — a producer that emitted `""` for "no Hanji" means the same
    /// thing as omitting it, and the layout must not read the two differently.
    init(text: String, annotation: String?) {
        self.text = text
        self.annotation = (annotation?.isEmpty == false) ? annotation : nil
    }
}
