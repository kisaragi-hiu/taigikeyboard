package com.siansiansu.taigikeyboard.ime.settings

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pins the candidate-display-mode storage coercion + the two platform-side derivation
 * rules (`effectiveHanjiFirst` / `effectiveOutputBothScripts`).
 * `PrefHelper` itself needs a real Android `Context` (DataStore), so the
 * rules are tested at the enum seam it delegates to — same rationale as
 * `EngineSettingsLiveReadTest`.
 */
class CandidateDisplayModeTest {
    @Test
    fun fromStorage_roundTripsEveryMode() {
        for (mode in CandidateDisplayMode.entries) {
            assertEquals(mode, CandidateDisplayMode.fromStorage(mode.storageValue))
        }
    }

    @Test
    fun fromStorage_unknownOrAbsentRaw_fallsBackToSideBySide() {
        assertEquals(CandidateDisplayMode.SIDE_BY_SIDE, CandidateDisplayMode.fromStorage("garbage"))
        assertEquals(CandidateDisplayMode.SIDE_BY_SIDE, CandidateDisplayMode.fromStorage(""))
        assertEquals(CandidateDisplayMode.SIDE_BY_SIDE, CandidateDisplayMode.fromStorage(null))
    }

    @Test
    fun storageValues_matchCrossPlatformRawStrings() {
        assertEquals("sideBySide", CandidateDisplayMode.SIDE_BY_SIDE.storageValue)
        assertEquals("romanOnly", CandidateDisplayMode.ROMAN_ONLY.storageValue)
        assertEquals("combined", CandidateDisplayMode.COMBINED.storageValue)
        assertEquals(CandidateDisplayMode.COMBINED, CandidateDisplayMode.fromStorage("combined"))
    }

    /**
     * A stored `true` reads `false` under ROMAN_ONLY and comes back once
     * the mode returns to SIDE_BY_SIDE — storage is never rewritten.
     */
    @Test
    fun test_INVARIANT_roman_only_suppresses_stored_script_flags_without_clearing_them() {
        val storedIsHanjiFirst = true
        val storedOutputBothScripts = true

        assertFalse(CandidateDisplayMode.ROMAN_ONLY.effectiveHanjiFirst(storedIsHanjiFirst))
        assertFalse(CandidateDisplayMode.ROMAN_ONLY.effectiveOutputBothScripts(storedOutputBothScripts))

        // Leaving roman-only restores the stored choice.
        assertTrue(CandidateDisplayMode.SIDE_BY_SIDE.effectiveHanjiFirst(storedIsHanjiFirst))
        assertTrue(CandidateDisplayMode.SIDE_BY_SIDE.effectiveOutputBothScripts(storedOutputBothScripts))

        // A stored `false` stays false in every mode but COMBINED's swap (pinned below).
        assertFalse(CandidateDisplayMode.SIDE_BY_SIDE.effectiveHanjiFirst(false))
        assertFalse(CandidateDisplayMode.SIDE_BY_SIDE.effectiveOutputBothScripts(false))
        assertFalse(CandidateDisplayMode.ROMAN_ONLY.effectiveHanjiFirst(false))
        assertFalse(CandidateDisplayMode.ROMAN_ONLY.effectiveOutputBothScripts(false))
    }

    /**
     * COMBINED projects to "cell leads with hanji, commit writes hanji":
     * swap reads `true` whatever is stored, but output-both-scripts passes the stored value through.
     * Storage is untouched, so SIDE_BY_SIDE restores the user's choice.
     */
    @Test
    fun test_INVARIANT_combined_forces_swap_true_and_passes_output_both_through() {
        // stored (swap=false, both=false) → effective (true, false)
        assertTrue(CandidateDisplayMode.COMBINED.effectiveHanjiFirst(false))
        assertFalse(CandidateDisplayMode.COMBINED.effectiveOutputBothScripts(false))

        // stored (swap=false, both=true) → effective (true, true)
        assertTrue(CandidateDisplayMode.COMBINED.effectiveOutputBothScripts(true))

        // stored swap=true is also true (idempotent projection).
        assertTrue(CandidateDisplayMode.COMBINED.effectiveHanjiFirst(true))

        // Back to SIDE_BY_SIDE: the stored `false` swap is what the user sees again.
    }

    /** The rules PrefHelper and the UI gates read live on the enum — pinned once. */
    @Test
    fun rules_perMode() {
        assertEquals(
            listOf(CandidateDisplayMode.SIDE_BY_SIDE, CandidateDisplayMode.COMBINED),
            CandidateDisplayMode.entries.filter { it.allowsSwapToggle },
        )
        assertEquals(listOf(CandidateDisplayMode.ROMAN_ONLY), CandidateDisplayMode.entries.filterNot { it.showsHanji })
    }

    /**
     * Punctuation width follows the STORED swap under SIDE_BY_SIDE / COMBINED —
     * COMBINED forces the candidate projection on but the 文/A key still picks
     * the width — and is always half-width under ROMAN_ONLY.
     */
    @Test
    fun test_INVARIANT_punctuation_width_follows_stored_swap_except_roman_only() {
        assertFalse(CandidateDisplayMode.COMBINED.effectiveFullWidthPunctuation(false))
        assertTrue(CandidateDisplayMode.COMBINED.effectiveFullWidthPunctuation(true))
        assertTrue(CandidateDisplayMode.COMBINED.effectiveHanjiFirst(false))
        assertFalse(CandidateDisplayMode.SIDE_BY_SIDE.effectiveFullWidthPunctuation(false))
        assertTrue(CandidateDisplayMode.SIDE_BY_SIDE.effectiveFullWidthPunctuation(true))
        assertFalse(CandidateDisplayMode.ROMAN_ONLY.effectiveFullWidthPunctuation(true))
    }
}
