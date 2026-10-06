package com.siansiansu.taigikeyboard.ime.settings

import androidx.datastore.preferences.core.mutablePreferencesOf
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test

/**
 * Pins the Syllable Separator storage coercion and the one-time carry-over of the retired
 * No Hyphens switch ([carryOverHyphenlessRoman]). Mirrors iOS `SettingsKeyTests`
 * `test_storedNoHyphensSwitch_carriesOverAsNoSeparator`.
 */
class SyllableSeparatorTest {
    @Test
    fun fromStorage_roundTripsEverySeparator() {
        for (separator in SyllableSeparator.entries) {
            assertEquals(separator, SyllableSeparator.fromStorage(separator.storageValue))
        }
    }

    @Test
    fun fromStorage_unknownOrAbsentRaw_fallsBackToHyphen() {
        assertEquals(SyllableSeparator.HYPHEN, SyllableSeparator.fromStorage("garbage"))
        assertEquals(SyllableSeparator.HYPHEN, SyllableSeparator.fromStorage(null))
    }

    @Test
    fun storedNoHyphensSwitch_carriesOverAsNone() {
        val prefs = mutablePreferencesOf(PreferenceKeys.RETIRED_HYPHENLESS_ROMAN to true)
        carryOverHyphenlessRoman(prefs)
        assertEquals("none", prefs[PreferenceKeys.SYLLABLE_SEPARATOR])
        assertFalse(PreferenceKeys.RETIRED_HYPHENLESS_ROMAN in prefs)
    }

    @Test
    fun storedSeparator_winsOverTheRetiredSwitch() {
        val prefs =
            mutablePreferencesOf(
                PreferenceKeys.RETIRED_HYPHENLESS_ROMAN to true,
                PreferenceKeys.SYLLABLE_SEPARATOR to "space",
            )
        carryOverHyphenlessRoman(prefs)
        assertEquals("space", prefs[PreferenceKeys.SYLLABLE_SEPARATOR])
        assertFalse(PreferenceKeys.RETIRED_HYPHENLESS_ROMAN in prefs)
    }

    @Test
    fun switchOff_carriesNothingOver() {
        val prefs = mutablePreferencesOf(PreferenceKeys.RETIRED_HYPHENLESS_ROMAN to false)
        carryOverHyphenlessRoman(prefs)
        assertFalse(PreferenceKeys.SYLLABLE_SEPARATOR in prefs)
        assertFalse(PreferenceKeys.RETIRED_HYPHENLESS_ROMAN in prefs)
    }
}
