// The settings one composing operation reads. The provider that serves them
// lives in EngineSettingsProvider.swift.

import Foundation

/// The romanization the user types. macOS ships TL and POJ only — TPS is
/// deliberately out of scope for this platform (`docs/architecture/macos-roadmap.md`
/// § Goal), which is why this enum has no `tps` case to fall through.
enum InputMode: String, CaseIterable, Sendable {
    case tl
    case poj
}

/// How the candidate window renders the `(Hanji, romanization)` pair: both scripts
/// side by side (the swap setting decides which leads), each script as its own
/// adjacent cell (Hanji with Romanization, `PresentedCandidate`), or the romanization alone.
///
/// Raw values are the storage contract every platform shares
/// (`docs/reports/2026-08-30-hanlo-together-mode-research.md` §12) — the same
/// convention `isHanjiFirst` follows, so a future
/// settings transfer carries one vocabulary.
/// CROSS-PLATFORM INVARIANT — mirrors ios/Sources/TaigiKeyboard/Settings/SettingsModels.swift
/// `CandidateDisplayMode` and the Android `CandidateDisplayMode.storageValue`.
/// Drift changes which mode a transferred setting resolves to.
enum CandidateDisplayMode: String, CaseIterable, Sendable {
    case sideBySide
    case combined
    case romanOnly

    /// Whether the cell shows any Hanji — `false` only under `.romanOnly`.
    var showsHanji: Bool {
        self != .romanOnly
    }

    /// The mode after this one, in the order the Appearance picker lists them, and
    /// round again from the end — what the cycle shortcut steps through, so
    /// the key and the picker agree on what "next" is.
    var next: CandidateDisplayMode {
        let all = Self.allCases
        let index = all.firstIndex(of: self) ?? all.startIndex
        return all[(index + 1) % all.count]
    }

    /// The picker row's label for this mode — also what the cycle shortcut
    /// flashes, so the HUD names the mode in the words the pane uses.
    var displayNameKey: StringKey {
        switch self {
        case .sideBySide: .settingsCandidateDisplayModeSideBySide
        case .combined: .settingsCandidateDisplayModeCombined
        case .romanOnly: .settingsCandidateDisplayModeRomanOnly
        }
    }

    /// Whether the swap shortcut writes the stored swap — exactly where Hanji
    /// is on screen. Under `.combined` the cells are split per script, so the
    /// shortcut only picks the punctuation width (USER 2026-09-13: "Hanji with Romanization needs an
    /// isTranslateSwapped button"); `.romanOnly` leaves it inert and the stored
    /// swap waits for the way back.
    var allowsSwapToggle: Bool {
        showsHanji
    }

    /// Effective swap for a stored flag. `.combined` leads with — and commits —
    /// the Hanji: forcing the swap on is a compatibility projection of that,
    /// so every reader of the swap (auto-space, full-width punctuation, the
    /// nextword gates) behaves as today's hanji-first mode
    /// (`behavioral-invariants.md` §42). `.romanOnly` has no Hanji to lead with.
    /// CROSS-PLATFORM INVARIANT — mirrors ios `SettingsModels.swift`
    /// `CandidateDisplayMode.effectiveHanjiFirst`, android
    /// `CandidateDisplayMode.kt`, windows `engine_settings.rs`. Drift causes
    /// silent divergence.
    func effectiveHanjiFirst(stored: Bool) -> Bool {
        self == .combined || (stored && showsHanji)
    }

    /// Whether a typed punctuation key becomes full-width (`，` for `,`) for a
    /// stored swap flag — the stored flag masked by the display mode, NOT the
    /// candidate projection above, which `.combined` forces on while the swap
    /// shortcut still picks the width. Mirrored on ios / android / windows.
    func effectiveFullWidthPunctuation(stored: Bool) -> Bool {
        stored && showsHanji
    }
}

/// Immutable snapshot of everything the engine needs to render a composition.
///
/// A snapshot rather than a set of getters because a single user intent can
/// issue several FFI calls (a commit, then its next-word handshake), and those
/// calls must agree: reading the settings twice could straddle a change and
/// render the two halves of one intent under different rules.
struct EngineSettings: Equatable, Sendable {
    let inputMode: InputMode

    /// Word-boundary spacing input for the engine's `continuous_word_space`
    /// predicate (`docs/engine/continuous-commit-and-display.md` §10.2). Carried in
    /// the snapshot rather than hardcoded at the call sites because iOS's #380
    /// regression was exactly a call site that stopped passing the live value.
    /// (Annotate in Brackets — `output_both_scripts` on the wire — is a mobile
    /// setting; macOS never ships it, so the wire field stays at its default.)
    ///
    /// EFFECTIVE, not stored: under `candidateDisplayMode == .romanOnly` it
    /// reads `false` whatever the user has stored, because a mode that shows
    /// and commits only romanization has no Hanji to lead with. The stored
    /// value lives on in `UserDefaults` (`SettingsStore.storedIsHanjiFirst`)
    /// and comes back the moment the mode returns to `.sideBySide`. Under
    /// `.combined` the swap reads `true` whatever is stored — the Hanji cell
    /// comes first and is the `.primary` commit, the romanization cell beside
    /// it the `.alternate` one (`PresentedCandidate`). Every reader of "swap"
    /// — engine `AppConfig`, cell, document text, auto-space — reads THIS
    /// value, never the stored one; full-width punctuation reads
    /// `isFullWidthPunctuation` instead.
    /// CROSS-PLATFORM INVARIANT — mirrors ios/Sources/TaigiKeyboard/Settings/EngineSettings.swift
    /// `isHanjiFirst` (derived the same way) and the Windows
    /// `document.rs engine_settings()`. Drift changes what a
    /// romanization-only install commits.
    let isHanjiFirst: Bool

    /// `CandidateDisplayMode.effectiveFullWidthPunctuation(stored:)` — read by
    /// `FullWidthPunctuation.documentPunctuation(_:isWidthFlip:settings:)` only.
    let isFullWidthPunctuation: Bool

    /// Whether the candidate window shows both scripts or the romanization
    /// alone. Sent to the engine as `AppConfig.candidate_display_mode`, which
    /// is what collapses same-romanization rows under `.romanOnly`
    /// (`engine/composing/src/requests.rs`, `engine/nextword/src/filter.rs`);
    /// on this side it selects the cell arm and derives the swap above.
    /// CROSS-PLATFORM INVARIANT — mirrors ios/Sources/TaigiKeyboard/Settings/EngineSettings.swift
    /// `candidateDisplayMode`, which defaults it to side-by-side. Drift changes
    /// what a fresh install's candidate cells show.
    let candidateDisplayMode: CandidateDisplayMode

    /// §34/S22 — when on, TL/POJ composing surfaces the preedit literal as the
    /// index-0 candidate so mixed-script writing commits the romanization in one keystroke, and
    /// Return on a fresh bar writes what was typed. The bridge inverts it into
    /// `FetchAtPos.literal_roman_candidate_disabled`.
    /// CROSS-PLATFORM INVARIANT — mirrors `isLiteralRomanCandidateEnabled` in
    /// ios/Sources/TaigiKeyboard/Settings/SharedSettings.swift and
    /// `literalRomanCandidateEnabled` in android/…/ime/settings/PrefHelper.kt;
    /// every platform defaults it OFF (USER 2026-10-02).
    /// Drift changes which candidate leads the list on a fresh install.
    let isLiteralRomanCandidateEnabled: Bool

    /// No Hyphens (`behavioral-invariants.md` §49) — sent as
    /// `AppConfig.hyphenless_roman` on every request; no TPS layout here.
    /// CROSS-PLATFORM INVARIANT — mirrors `isHyphenlessRomanEnabled` in
    /// ios/Sources/TaigiKeyboard/Settings/SharedSettings.swift and
    /// android/…/ime/settings/PrefHelper.kt, both OFF.
    let isHyphenlessRomanEnabled: Bool

    /// ⁿ becomes ᴺ in capitals (`behavioral-invariants.md` §53) — the POJ nasal marker follows
    /// the case of the letters before it (`SIÂᴺ`); off, it is always `ⁿ`.
    /// Sent inverted as `AppConfig.force_lowercase_nasal_marker` on the base
    /// config.
    /// CROSS-PLATFORM INVARIANT — mirrors `isNasalMarkerUppercaseEnabled` in
    /// ios/Sources/TaigiKeyboard/Settings/SharedSettings.swift and
    /// `nasalMarkerUppercaseEnabled` in android/…/ime/settings/PrefHelper.kt, all ON.
    let isNasalMarkerUppercaseEnabled: Bool

    /// Whether the user's own dictionary contributes candidates. Gates the
    /// lookup itself, not just the display: with it off nothing is read from
    /// `custom_dictionary.db` (`FetchAtPos.custom_dictionary_disabled`).
    /// CROSS-PLATFORM INVARIANT — mirrors ios/Sources/TaigiKeyboard/Settings/SharedSettings.swift:51,
    /// which defaults it ON.
    let isCustomDictEnabled: Bool

    /// Which bundled dictionaries the engine may draw candidates from. Reaches
    /// the engine as `FetchAtPos.toggles`, which it resolves into its source
    /// filter (`RustEngineBridge.dictionaryTogglesProto`).
    let dictionarySources: DictionarySourceToggles

    /// What a fresh install types with. Every value matches the iOS and Android
    /// default for the same setting, so someone using two of the four platforms
    /// gets the same composition and the same candidate order out of the box.
    ///
    /// Hanji-first since 2026-09-18 (USER: "by default Hanji is output first, that is, Hanji is the
    /// title and romanization is the subtitle"): the stored swap is on, and the two effective
    /// fields are DERIVED from it under Hanji–Romanization Pairing the way `SettingsStore.current`
    /// derives them, so the snapshot cannot say one thing about the swap and
    /// another about the width.
    static let defaults: EngineSettings = {
        let storedSwap = true
        let mode = CandidateDisplayMode.sideBySide
        return EngineSettings(
            inputMode: .tl,
            isHanjiFirst: mode.effectiveHanjiFirst(stored: storedSwap),
            isFullWidthPunctuation: mode.effectiveFullWidthPunctuation(stored: storedSwap),
            candidateDisplayMode: mode,
            isLiteralRomanCandidateEnabled: false,
            isHyphenlessRomanEnabled: false,
            isNasalMarkerUppercaseEnabled: true,
            isCustomDictEnabled: true,
            dictionarySources: .defaults,
        )
    }()
}
