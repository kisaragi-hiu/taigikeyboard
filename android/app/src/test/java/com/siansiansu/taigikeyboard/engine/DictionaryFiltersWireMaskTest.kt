package com.siansiansu.taigikeyboard.engine

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Pins what `ComposingManager.fetchContinuousCandidates` puts in
 * `FetchAtPos.enabled_sources_bitmask` (`behavioral-invariants.md` §57).
 *
 * The engine answers "every dictionary off" with `0`, which the composing wire
 * reserves for "platform did not wire this" and turns back into every source.
 * [RustEngineBridge.DictionaryFilters.wireMask] sends the no-sources sentinel
 * instead. JVM tests cannot load the engine, so the engine's `0` is taken as
 * given here; iOS `DictionaryFiltersWireMaskTests` resolves it through the
 * engine, as do macOS `RustEngineBridgeDictionaryFiltersTests` and desktop
 * `engine_roundtrip.rs::all_sources_off_resolves_to_the_non_zero_sentinel`.
 */
class DictionaryFiltersWireMaskTest {
    private fun filters(mask: UInt) = RustEngineBridge.DictionaryFilters(dictionaryFilterBitmask = mask, enabledSources = emptySet())

    // INVARIANT_DICTIONARIES_ALL_OFF_OFFERS_NO_DICTIONARY_CANDIDATES (behavioral-invariants.md §57)
    @Test
    fun wireMask_resolvedZero_sendsTheNoSourcesSentinel() {
        val sent = filters(0u).wireMask

        assertEquals(RustEngineBridge.DictionaryFilters.NO_SOURCES_ENABLED_BITMASK, sent)
        assertEquals("exactly the kautian-gate bit", 1u shl 13, sent)
        assertEquals("the source region has to stay empty", 0u, sent and 0x1FFFu)
    }

    @Test
    fun wireMask_resolvedNonZero_goesOutUnchanged() {
        val cases = listOf(0b101u, 1u shl 12, (1u shl 13) or (1u shl 14) or 1u)
        for (mask in cases) {
            assertEquals("a resolved mask $mask must go out as it is", mask, filters(mask).wireMask)
        }
    }

    @Test
    fun wireMask_failedResolve_staysAllSourcesOn() {
        assertEquals(
            "a failed resolve widens the list, it never empties it",
            UInt.MAX_VALUE,
            RustEngineBridge.DictionaryFilters.ALL_SOURCES_ENABLED.wireMask,
        )
    }
}
