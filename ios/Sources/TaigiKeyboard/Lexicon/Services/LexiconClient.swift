import Foundation

/// The engine's system-dictionary surface as `DictionarySearchService` sees it:
/// a seam so the service's policy can be tested with a fake lexicon. Mirrors
/// Android `ime/dictionary/LexiconClient.kt`; the shipped implementation is
/// `EngineLexiconClient`, the same shape as `UserDataClient`.
protocol LexiconClient: Sendable {
    func isHanji(_ text: String) -> Bool
    func dictionaryFilters(toggles: RustEngineBridge.DictionaryToggles) -> RustEngineBridge.DictionaryFilters
    func searchWithSources(
        input: String,
        inputMode: RustEngineBridge.LexiconInputMode,
        limit: UInt32,
        enabledSourcesBitmask: UInt32,
    ) -> [RustEngineBridge.LexiconRow]
    func searchByHanji(
        query: String,
        inputMode: RustEngineBridge.LexiconInputMode,
        limit: UInt32,
        enabledSourcesBitmask: UInt32,
    ) -> [RustEngineBridge.LexiconRow]
}

/// The engine's lexicon ops, one bridge call each.
struct EngineLexiconClient: LexiconClient {
    func isHanji(_ text: String) -> Bool {
        RustEngineBridge.isHanji(text)
    }

    func dictionaryFilters(toggles: RustEngineBridge.DictionaryToggles) -> RustEngineBridge.DictionaryFilters {
        RustEngineBridge.lexiconDictionaryFilters(toggles: toggles)
    }

    func searchWithSources(
        input: String,
        inputMode: RustEngineBridge.LexiconInputMode,
        limit: UInt32,
        enabledSourcesBitmask: UInt32,
    ) -> [RustEngineBridge.LexiconRow] {
        RustEngineBridge.lexiconSearchWithSources(
            input: input,
            inputMode: inputMode,
            limit: limit,
            enabledSourcesBitmask: enabledSourcesBitmask,
        )
    }

    func searchByHanji(
        query: String,
        inputMode: RustEngineBridge.LexiconInputMode,
        limit: UInt32,
        enabledSourcesBitmask: UInt32,
    ) -> [RustEngineBridge.LexiconRow] {
        RustEngineBridge.lexiconSearchByHanji(
            query: query,
            inputMode: inputMode,
            limit: limit,
            enabledSourcesBitmask: enabledSourcesBitmask,
        )
    }
}
