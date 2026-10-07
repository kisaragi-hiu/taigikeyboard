// The Shortcuts pane's composing rows, as desktop-core resolves them.

/// The user's composing key contract as one store holds it, resolved by
/// desktop-core (`ComposingKeyBindings::from_document`, through
/// `KeyRules.composingShortcuts(in:)`), and the pane's fixed rows.
///
/// "Resolved" means the core's three rules have run: every chord passed the
/// gate; no chord is on two rows (the one the user recorded keeps it); an
/// emptied commit row has taken back a free default of its pair.
struct ComposingShortcuts: Sendable {
    /// One composing action's row.
    struct Row: Sendable, Equatable {
        /// The action's name (`nextCandidate`).
        let name: String
        /// Where its chord is stored (`composingShortcut.nextCandidate`).
        let settingsKey: String
        let label: StringKey
        /// Its block on the pane: 0 through the candidates, 1 out of the
        /// composition.
        let group: Int
        /// The resolved chord; nil when the row is empty.
        let chord: ComposingKeyChord?
        /// What a fresh install has on it.
        let defaultChord: ComposingKeyChord
    }

    /// Every composing action, in the roster's order.
    let rows: [Row]
    /// The fixed rows, as the pane prints them: the live slot keys bare and
    /// behind ⇧, the navigation keys, the composing caret's chords, the
    /// cancel key.
    let slotKeys: String
    let shiftedSlotKeys: String
    let navigationKeys: String
    let caretChords: String
    let cancelKey: String

    /// No rows and no labels — what the pane draws when the core could not
    /// answer (`KeyRules`, logged there).
    static let unavailable = ComposingShortcuts(
        rows: [],
        slotKeys: "",
        shiftedSlotKeys: "",
        navigationKeys: "",
        caretChords: "",
        cancelKey: "",
    )

    /// The rows of one block, in roster order.
    func rows(inGroup group: Int) -> [Row] {
        rows.filter { $0.group == group }
    }

    /// The rows holding `chord`, other than `excluded` — the pure half of
    /// recording: the pane empties these before it writes, so the row that
    /// loses a chord empties in front of the user. The cross-registry scan
    /// asks without an exclusion: the global tier is nobody's row here.
    func rows(holding chord: ComposingKeyChord, excluding excluded: Row? = nil) -> [Row] {
        rows.filter { $0.chord == chord && $0.name != excluded?.name }
    }
}
