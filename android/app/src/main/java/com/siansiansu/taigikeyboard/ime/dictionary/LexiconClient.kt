// The lexicon ops the Dictionary tab reads; `LexiconService` is the engine-backed one, JVM tests fake it.

package com.siansiansu.taigikeyboard.ime.dictionary

import com.siansiansu.taigikeyboard.engine.RustEngineBridge
import com.siansiansu.taigikeyboard.ime.core.Outcome

/**
 * The engine's system-dictionary surface as `DictionarySearchService` sees it.
 * Mirrors iOS `Lexicon/Services/LexiconClient.swift`; the shipped implementation is
 * [LexiconService], the same shape as `UserDataClient` / `EngineUserDataClient`.
 */
interface LexiconClient {
    fun isHanji(text: String): Boolean

    fun dictionaryFilters(toggles: RustEngineBridge.DictionaryToggles): RustEngineBridge.DictionaryFilters

    suspend fun searchWithSources(
        input: String,
        inputMode: RustEngineBridge.LexiconInputMode,
        filterBitmask: UInt,
        limit: Int,
    ): Outcome<List<DictionarySearchResult>, DictionaryError>

    suspend fun searchByHanji(
        input: String,
        inputMode: RustEngineBridge.LexiconInputMode,
        filterBitmask: UInt,
        limit: Int,
    ): Outcome<List<DictionarySearchResult>, DictionaryError>
}
