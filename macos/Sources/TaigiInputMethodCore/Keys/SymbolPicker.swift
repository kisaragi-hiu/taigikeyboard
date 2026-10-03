// What a key means while the symbol picker is up.

/// The picker's reading of one key event, decided before any window is asked
/// anything — by desktop-core (`keys/symbol_picker.rs`, through
/// `KeyRules.symbolPickerIntent(for:in:)`), in the order the candidate bar
/// reads keys: the fixed navigation keys, the slot keys of the live tone
/// scheme, then whatever the user put on the paging and confirm rows. Its own
/// reading rather than the composition's: the picker runs with no
/// composition, and its keys pick from a list the engine never fetched.
enum SymbolPickerIntent: Equatable, Sendable {
    /// Take the picker down and swallow the key. Escape.
    case close
    /// Move the selection the way the window's layout reads `direction`.
    case navigate(CandidateNavigation)
    /// Pick the cell the `slot`-th selection key addresses on the visible
    /// page — the same keys that pick a candidate.
    case pickSlot(Int)
    /// Pick the highlighted cell: the confirm row, and the Hanji/romanization
    /// row, which has no other script to write here.
    case confirm
    /// Take the picker down and let the key go on to do its job: the picker
    /// is a list to pick from, not a mode, so a letter typed over it starts
    /// the composition it would have started anyway. The literal-commit row
    /// too — there is no literal to write.
    case closeAndPassThrough
}
