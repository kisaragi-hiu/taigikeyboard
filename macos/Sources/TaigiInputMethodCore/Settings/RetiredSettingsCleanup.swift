// Launch-time removal of persisted state for settings retired from the macOS UI.

import Foundation
import KeyboardShortcuts

/// Clears what older builds persisted for settings this build no longer
/// exposes, so an upgraded install behaves like a fresh one. Idempotent, and
/// run every launch rather than behind a version flag: removing an absent key
/// is free, and re-clearing also catches a manual `defaults write` that would
/// otherwise resurrect hidden state.
///
/// Nothing reads a retired key any more, so every sweep here is hygiene: it
/// keeps the defaults domain honest and stops a later setting that reused a
/// name from inheriting a value nobody chose for it.
@MainActor
enum RetiredSettingsCleanup {
    /// Raw `KeyboardShortcuts.Name`s of the retired hotkey actions, kept so a
    /// stored chord can be cleared: the library persists chords by raw name,
    /// an orphaned chord would spring back to life on any future action that
    /// reused the name, and `ShortcutConflicts` only sees the live roster.
    /// Declared with no `initial:`, or the construction below would seed the
    /// very chord this is here to clear.
    private static let retiredShortcutNames = [
        KeyboardShortcuts.Name("toggleBothScripts"),
        KeyboardShortcuts.Name("toggleLiteralRomanCandidate"),
        // Open Settings, retired 2026-08-24 when every pane got a row of its own:
        // a chord for "whichever pane was last used" is a second key for what
        // General does. Unlike the two above this one shipped, so real installs
        // hold ⌃⇧, — or whatever the user recorded over it.
        //
        // The CONCEPT came back on 2026-08-25 as `openLastSettingsPane` on
        // ⌃⌘S, once the five pane chords below went and took the reason it was
        // redundant with them. Deliberately under a different raw value: this
        // sweep runs every launch, so an action reusing the name would have
        // its chord cleared each time.
        KeyboardShortcuts.Name("openSettings"),
        // The five pane doorways, ⌃⇧1–⌃⇧5, retired 2026-08-25 (USER): five
        // chords is a lot to hold for panes visited about once a day, which
        // the menu bar already lists by name. One ⌃⌘S doorway reopens
        // wherever the user left off. Cleared rather than
        // left: a chord the user recorded on a row that no longer exists is
        // unreachable from the UI and would spring back on any future action
        // that reused the name.
        KeyboardShortcuts.Name("openGeneralPane"),
        KeyboardShortcuts.Name("openAppearancePane"),
        KeyboardShortcuts.Name("openShortcutPane"),
        KeyboardShortcuts.Name("openCustomDictionaryPane"),
        KeyboardShortcuts.Name("openDictionarySourcesPane"),
    ]

    /// Sidebar panes removed from `SettingsPane`. Cleared explicitly rather
    /// than trusting `@AppStorage` to shrug off an unknown raw value — that
    /// fallback is framework behaviour this codebase has not pinned.
    ///
    /// An explicit tombstone list, NOT "any value that is not a live case": an
    /// unknown raw value is not necessarily a retired one, and a build that
    /// cleared everything it did not recognise would reset the selection of
    /// anyone who also runs a newer build against the same defaults.
    private static let retiredPaneRawValues: Set<String> = [
        "dictionarySearch",
        "frequencyData",
        "associationData",
        "backupRestore",
    ]

    /// Retired raw defaults names, grouped by the round that retired them.
    ///
    /// Hygiene, not a behaviour fix: nothing reads any of them any more, so a
    /// stored value already changes nothing. They are cleared to keep the
    /// domain honest, and because a later setting reusing one of these names
    /// would inherit a value nobody chose for it. None are migrated — each
    /// described a choice this build no longer offers.
    private static let retiredDefaultsNames = [
        // The first shape of the composing-key settings, which described what
        // a key does rather than which key does a job (the composing roster).
        // The shape they replaced never shipped in a release, so the only
        // installs that can hold one are builds from main.
        "returnKeyBehavior",
        "spaceKeyBehavior",
        "bracketPagingBehavior",
        "tabCycleBehavior",
        // The Appearance pane's two retired rows — the reasoning for each lives on
        // its type, `CandidateAccentColor` and `CandidateWindowStyle`.
        "candidateAccentColor",
        "candidateWindowStyle",
        // Full-width Punctuation and Shift-to-English, retired 2026-08-24. Full-width Punctuation is always
        // on now; the Shift toggle's whole feature went on 2026-08-26, when
        // this input method stopped having an English mode. Either way the
        // stored value is inert, and this only keeps the domain honest.
        "fullWidthPunctuationEnabled",
        "shiftTogglesAlphanumericEnabled",
        // Candidate Selection Keys, retired 2026-09-08: the slot key set is derived from
        // `toneInputScheme` now, and the ⇧ / ⌃ / ⌥ digit sets this key could
        // name no longer exist to migrate to.
        "candidateSlotModifier",
        // Association Records, retired 2026-09-25 when the engine dropped
        // `AppConfig.is_association_recording_enabled`: association recording
        // is always on, so a stored value (a `false` left by an older build or
        // a hand edit) is inert now.
        "associationRecordingEnabled",
        // Annotate in Brackets and Frequency Records, retired 2026-09-30: neither
        // toggle ever shipped in a release (both lived only in builds from
        // main between 2026-08-16 and 2026-08-24), nothing reads the keys any
        // more — the desktop never brackets, and the engine counts every pick
        // (`RecordUsage` tag 4 reserved). Mobile keeps its own Annotate in
        // Brackets key and UI.
        "outputBothScripts",
        "frequencyRecordingEnabled",
    ]

    /// Raw values of composing actions removed from the roster: Commit Hanji Directly and
    /// Commit Romanization Directly, retired 2026-08-25 when the Hanji/romanization switch was left as the
    /// one place a user chooses which script a commit writes.
    ///
    /// Their chords sit under the roster's `composingShortcut.<name>`
    /// (desktop-core's `ComposingAction::settings_key_name_for_raw`), and the
    /// pane resets only the rows the core still has — so a chord recorded on
    /// one of these rows is unreachable by every row the pane draws. Spelled
    /// out whole: tombstones in a namespace the core owns.
    private static let retiredComposingChordNames = [
        "composingShortcut.commitHanji",
        "composingShortcut.commitRomanization",
    ]

    /// The No Hyphens switch, retired 2026-10-06 when the Syllable Separator
    /// picker replaced it (`behavioral-invariants.md` §49). Unlike the names
    /// above it described a choice this build still offers, so a stored `true`
    /// carries over as `none` first — unless the picker already holds a choice.
    /// desktop-core twin: `SettingsDocument::from_json`.
    static let retiredHyphenlessRomanName = "hyphenlessRomanEnabled"

    static func run(userDefaults: UserDefaults = .standard) {
        carryOverHyphenlessRoman(userDefaults: userDefaults)
        for name in retiredDefaultsNames {
            userDefaults.removeObject(forKey: name)
        }
        for name in retiredComposingChordNames {
            userDefaults.removeObject(forKey: name)
        }
        if let pane = userDefaults.string(forKey: SettingsStore.Keys.selectedSettingsPane.name),
           retiredPaneRawValues.contains(pane)
        {
            userDefaults.removeObject(forKey: SettingsStore.Keys.selectedSettingsPane.name)
        }
        for name in retiredShortcutNames {
            KeyboardShortcuts.setShortcut(nil, for: name)
        }
    }

    private static func carryOverHyphenlessRoman(userDefaults: UserDefaults) {
        guard let stored = userDefaults.object(forKey: retiredHyphenlessRomanName) else { return }
        let separatorName = SettingsStore.Keys.syllableSeparator.name
        if stored as? Bool == true, userDefaults.object(forKey: separatorName) == nil {
            userDefaults.set(SyllableSeparator.noSeparator.rawValue, forKey: separatorName)
        }
        userDefaults.removeObject(forKey: retiredHyphenlessRomanName)
    }
}
