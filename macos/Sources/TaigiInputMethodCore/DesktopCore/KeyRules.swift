// The Shortcuts pane's and the symbol picker's key rules, asked of desktop-core.

import AppKit

/// desktop-core's chord gate, recorder decision, resolved composing bindings,
/// fixed-row labels and symbol-picker reading, under the Mac's grammar — the
/// key-rule requests of the shell seam
/// (`macos/crates/taigi-macos-ffi/src/key_rules.rs`; roadmap P14). What is
/// AppKit's or the global tier's stays Swift: the recording field, a click,
/// a held key's repeat, `KeyboardShortcuts` and its policy.
///
/// Each answer is nil when the seam fails (logged). `press` and `chord` read
/// no settings and need no runtime. The pane's rows and the picker's keys are
/// read under the settings of the defaults they are given, so they need the
/// runtime's settings whitelist and answer nil before `Configure` — when the
/// input method types nothing either.
enum KeyRules {
    /// What a key press does to a shortcut-recording field.
    enum Press: Equatable {
        /// Bound; recording ends.
        case recorded(ComposingKeyChord)
        /// Turned down; recording goes on, the reason in place of the prompt.
        case refused(ComposingKeyChord.Rejection)
        /// The Escape key: the field gives up focus.
        case blur
        /// A bare Backspace, Delete or forward Delete: the field blanks.
        case blank
    }

    private static let logger = DebugLogger(category: "KeyRules")

    /// What `key` records in a recording field, or why not.
    static func press(_ key: KeyEventSnapshot) -> Press? {
        var request = Taigi_DesktopShell_PressRequest()
        request.event = Taigi_DesktopShell_KeyEvent(key)
        let reply = DesktopCoreBridge.roundtrip(.press(request), op: "press") {
            if case let .press(reply) = $0 {
                reply
            } else {
                nil
            }
        }
        switch reply?.outcome {
        case let .recorded(chord)?:
            return .recorded(ComposingKeyChord(chord))
        case let .refused(reason)?:
            if let rejection = ComposingKeyChord.Rejection(reason) {
                return .refused(rejection)
            }
        case .action(.blur)?:
            return .blur
        case .action(.blank)?:
            return .blank
        case nil:
            return nil
        default:
            break
        }
        logger.error("[press] an outcome this side does not know")
        return nil
    }

    /// The chord `key` and `modifiers` make, or why they make none — the gate
    /// a recorded chord passes. Only ⌘ ⌃ ⌥ ⇧ are read.
    static func chord(
        key: String?,
        modifiers: NSEvent.ModifierFlags,
    ) -> Result<ComposingKeyChord, ComposingKeyChord.Rejection>? {
        var request = Taigi_DesktopShell_ChordRequest()
        if let key {
            request.key = key
        }
        request.modifierFlags = UInt64(modifiers.rawValue)
        let reply = DesktopCoreBridge.roundtrip(.chord(request), op: "chord") {
            if case let .chord(reply) = $0 {
                reply
            } else {
                nil
            }
        }
        switch reply?.answer {
        case let .chord(chord)?:
            return .success(ComposingKeyChord(chord))
        case let .rejection(reason)?:
            if let rejection = ComposingKeyChord.Rejection(reason) {
                return .failure(rejection)
            }
        case nil:
            if reply == nil {
                return nil
            }
        }
        logger.error("[chord] an answer this side does not know")
        return nil
    }

    /// The Shortcuts pane's composing rows as `userDefaults` holds them, and
    /// its fixed rows.
    static func composingShortcuts(in userDefaults: UserDefaults) -> ComposingShortcuts? {
        guard let settings = snapshot(in: userDefaults, op: "composingShortcuts") else { return nil }
        let reply = DesktopCoreBridge.roundtrip(
            .composingShortcuts(Taigi_DesktopShell_ComposingShortcutsRequest()),
            settings: settings,
            op: "composingShortcuts",
        ) {
            if case let .composingShortcuts(reply) = $0 {
                reply
            } else {
                nil
            }
        }
        guard let reply else { return nil }
        var rows: [ComposingShortcuts.Row] = []
        for action in reply.actions {
            guard let label = StringKey(rawValue: action.labelKey), action.hasDefaultChord else {
                logger.error("[composingShortcuts] a row this side cannot draw: \(action.name)")
                return nil
            }
            rows.append(ComposingShortcuts.Row(
                name: action.name,
                settingsKey: action.settingsKey,
                label: label,
                group: Int(action.group),
                chord: action.hasChord ? ComposingKeyChord(action.chord) : nil,
                defaultChord: ComposingKeyChord(action.defaultChord),
            ))
        }
        return ComposingShortcuts(
            rows: rows,
            slotKeys: reply.slotKeys,
            shiftedSlotKeys: reply.shiftedSlotKeys,
            navigationKeys: reply.navigationKeys,
            caretChords: reply.caretChords,
            widthFlipChords: reply.widthFlipChords,
            cancelKey: reply.cancelKey,
        )
    }

    /// What `key` means while the symbol picker is up, under the bindings
    /// `userDefaults` holds.
    static func symbolPickerIntent(for key: KeyEventSnapshot, in userDefaults: UserDefaults) -> SymbolPickerIntent? {
        guard let settings = snapshot(in: userDefaults, op: "symbolPickerKey") else { return nil }
        var request = Taigi_DesktopShell_SymbolPickerKeyRequest()
        request.event = Taigi_DesktopShell_KeyEvent(key)
        let reply = DesktopCoreBridge.roundtrip(.symbolPickerKey(request), settings: settings, op: "symbolPickerKey") {
            if case let .symbolPicker(reply) = $0 {
                reply
            } else {
                nil
            }
        }
        switch reply?.intent {
        case .action(.close)?:
            return .close
        case .action(.confirm)?:
            return .confirm
        case .action(.closeAndPassThrough)?:
            return .closeAndPassThrough
        case let .navigate(direction)?:
            if let navigation = CandidateNavigation(direction) {
                return .navigate(navigation)
            }
        case let .pickSlot(slot)?:
            return .pickSlot(Int(slot))
        case nil:
            if reply == nil {
                return nil
            }
        default:
            break
        }
        logger.error("[symbolPickerKey] an intent this side does not know")
        return nil
    }

    /// The whitelisted settings as `userDefaults` holds them; nil — logged —
    /// before the runtime exists.
    private static func snapshot(in userDefaults: UserDefaults, op: String) -> Taigi_DesktopShell_SettingsSnapshot? {
        guard let runtime = DesktopCoreRuntime.configured else {
            logger.error("[\(op)] no desktop-core runtime — no settings to read under")
            return nil
        }
        return DesktopCoreRuntime.settingsSnapshot(runtime.settings, in: userDefaults)
    }
}
