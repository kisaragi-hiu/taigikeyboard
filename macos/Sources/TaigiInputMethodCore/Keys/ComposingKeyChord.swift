// One recordable key combination, as desktop-core hands it out.

/// A key plus its modifiers, as a composing action can be bound to it.
///
/// A value desktop-core hands out (`desktop/crates/taigi-desktop-core/src/keys/chord.rs`,
/// through `KeyRules`) and never built from its parts here: the gate that
/// refuses the keys a syllable is typed with, the case fold, the reserved
/// keys and the stored spelling are the core's, under the Mac's grammar.
struct ComposingKeyChord: Hashable, Sendable {
    /// The stored form, `"<modifiers>|<scalars>"` — modifier letters `d` ⌘
    /// `c` ⌃ `o` ⌥ `s` ⇧, then the key's scalars in hex. What `SettingsStore`
    /// writes, and the chord's identity: the form is canonical, so two chords
    /// are one exactly when their raw values are.
    let rawValue: String
    /// How the chord reads on screen: `⇧↩`, `⌃⌘R`, `Space`.
    let display: String

    init(_ chord: Taigi_DesktopShell_Chord) {
        rawValue = chord.raw
        display = chord.display
    }

    static func == (lhs: Self, rhs: Self) -> Bool {
        lhs.rawValue == rhs.rawValue
    }

    func hash(into hasher: inout Hasher) {
        hasher.combine(rawValue)
    }

    /// Why a key could not be recorded, so the recorder can say so rather than
    /// silently doing nothing.
    enum Rejection: Error, Equatable, Sendable {
        /// A letter, a digit, the hyphen or `;` with no ⌘/⌃/⌥ held — every
        /// key one of the two tone schemes types with or picks a candidate
        /// with. Refused whichever scheme is live, so a chord recorded under
        /// one cannot go inert when the user switches to the other.
        case typesRomanization
        /// Backspace, Escape, the arrows or the paging keys, which the input
        /// method reserves whatever modifiers are held.
        case reservedKey
        /// An event carrying no character to bind.
        case noKey
        /// A chord the system already answers to. Global-tier only: a Carbon
        /// hotkey never gets a chord the window server has taken first, so
        /// recording one would leave a row that reads as bound and does
        /// nothing (`KeyboardShortcuts.Shortcut.isTakenBySystem`).
        case takenBySystem
        /// A key press the Carbon registry cannot name. Global-tier only: a
        /// global row stores a key CODE, and a press that yields none has
        /// nothing to store.
        case notAGlobalKey
        /// A bare ⌘ combination. Global-tier only: ⌘ plus a key is the shape a
        /// Mac application puts its own menu commands on, and a global hotkey
        /// takes that key from whatever the user is typing into for as long as
        /// this input source is selected — ⌘, would cost them their app's own
        /// settings command. Composing rows never see this: they are read
        /// inside a composition, not registered process-wide.
        case belongsToHost

        /// The core's reason, or nil for one this side does not know — a seam
        /// failure, never a reason to show.
        init?(_ wire: Taigi_DesktopShell_ChordRejection) {
            switch wire {
            case .typesRomanization: self = .typesRomanization
            case .reservedKey: self = .reservedKey
            case .noKey: self = .noKey
            case .takenBySystem: self = .takenBySystem
            case .notAGlobalKey: self = .notAGlobalKey
            case .belongsToHost: self = .belongsToHost
            case .unspecified, .UNRECOGNIZED: return nil
            }
        }
    }
}
