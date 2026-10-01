package com.siansiansu.taigikeyboard.ime.theme

import androidx.datastore.preferences.core.MutablePreferences
import androidx.datastore.preferences.core.mutablePreferencesOf
import com.siansiansu.taigikeyboard.ime.settings.PreferenceKeys
import org.json.JSONArray
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Tests for [LegacyAppearance.retire] — a look customized on the retired appearance screen
 * becomes a user theme, and the six retired keys go. Mirrors iOS SharedSettingsTests
 * `test_retireLegacyAppearance_*`.
 */
class LegacyAppearanceTest {
    private val retiredKeys =
        listOf(
            PreferenceKeys.KEY_HEIGHT_SCALE,
            PreferenceKeys.KEY_FONT_SIZE_SCALE,
            PreferenceKeys.CANDIDATE_TEXT_SIZE_SCALE,
            PreferenceKeys.KEY_CORNER_RADIUS,
            PreferenceKeys.KEY_BORDER_WIDTH,
            PreferenceKeys.COLOR_SETTINGS,
        )

    /** What the retired appearance screen left behind: a red background, larger keys, round corners. */
    private fun customizedLegacyLook(): MutablePreferences =
        mutablePreferencesOf().apply {
            this[PreferenceKeys.COLOR_SETTINGS] = RED_BACKGROUND.toJson()
            this[PreferenceKeys.KEY_HEIGHT_SCALE] = 1.2f
            this[PreferenceKeys.KEY_CORNER_RADIUS] = 10f
        }

    private fun retire(prefs: MutablePreferences) = LegacyAppearance.retire(prefs, themeName = "新主題", now = NOW, newId = NEW_ID)

    private fun themes(prefs: MutablePreferences) = UserTheme.decodeList(prefs[PreferenceKeys.USER_THEMES] ?: "[]")

    private fun assertRetiredKeysGone(prefs: MutablePreferences) {
        retiredKeys.forEach { assertFalse("${it.name} must be removed", it in prefs) }
    }

    @Test
    fun nothingStored_changesNothing() {
        val prefs = mutablePreferencesOf()
        retire(prefs)
        assertTrue(prefs.asMap().isEmpty())
    }

    @Test
    fun customizedDefault_becomesTheSelectedUserTheme() {
        val prefs = customizedLegacyLook()

        retire(prefs)

        val theme = themes(prefs).single()
        assertEquals(NEW_ID, theme.id)
        assertEquals("新主題", theme.name)
        assertEquals(NOW, theme.createdAt)
        assertEquals(RED_BACKGROUND.background, theme.appearance.colors.background)
        assertEquals(1.2f, theme.appearance.keyHeightScale, 0f)
        assertEquals(10f, theme.appearance.keyCornerRadius, 0f)
        assertEquals(0f, theme.appearance.keyShadowIntensity, 0f)
        assertEquals(NEW_ID, prefs[PreferenceKeys.SELECTED_THEME_ID])
        assertRetiredKeysGone(prefs)

        // Once the keys are gone a later start carries nothing again.
        LegacyAppearance.retire(prefs, themeName = "新主題", now = NOW)
        assertEquals(1, themes(prefs).size)
    }

    @Test
    fun factoryValues_areOnlyRemoved() {
        // An old reset wrote the factory values.
        val prefs =
            mutablePreferencesOf().apply {
                this[PreferenceKeys.KEY_HEIGHT_SCALE] = ThemeAppearance.DEFAULT_KEY_HEIGHT_SCALE
                this[PreferenceKeys.KEY_CORNER_RADIUS] = ThemeAppearance.DEFAULT_KEY_CORNER_RADIUS
            }

        retire(prefs)

        assertTrue(themes(prefs).isEmpty())
        assertEquals(null, prefs[PreferenceKeys.SELECTED_THEME_ID])
        assertRetiredKeysGone(prefs)
    }

    @Test
    fun builtInSelected_keepsTheSelection() {
        val prefs = customizedLegacyLook().apply { this[PreferenceKeys.SELECTED_THEME_ID] = "standardBlue" }

        retire(prefs)

        assertEquals(1, themes(prefs).size)
        assertEquals("standardBlue", prefs[PreferenceKeys.SELECTED_THEME_ID])
        assertRetiredKeysGone(prefs)
    }

    @Test
    fun deletedUserThemeSelected_selectsTheCarriedLook() {
        // An id nothing resolves showed the legacy look before; it shows the carried theme now.
        val prefs = customizedLegacyLook().apply { this[PreferenceKeys.SELECTED_THEME_ID] = "0f8fad5b-d9cb-469f-a165-70867728950e" }

        retire(prefs)

        assertEquals(NEW_ID, prefs[PreferenceKeys.SELECTED_THEME_ID])
    }

    @Test
    fun atTheCap_stillKeepsTheLook() {
        val existing =
            (0 until UserThemeStore.MAX_USER_THEMES).map {
                UserTheme(id = "id-$it", name = "T$it", appearance = ThemeAppearance.DEFAULT, createdAt = 0L, updatedAt = 0L)
            }
        val prefs = customizedLegacyLook().apply { this[PreferenceKeys.USER_THEMES] = UserTheme.encodeList(existing) }

        retire(prefs)

        assertEquals(UserThemeStore.MAX_USER_THEMES + 1, themes(prefs).size)
        assertEquals(existing.map { it.id } + NEW_ID, themes(prefs).map { it.id })
        assertRetiredKeysGone(prefs)
    }

    @Test
    fun unreadableThemeList_leavesItAndTheKeys() {
        val unreadable = "[{\"id\": \"a\"}, 7"
        val prefs = customizedLegacyLook().apply { this[PreferenceKeys.USER_THEMES] = unreadable }

        retire(prefs)

        assertEquals(unreadable, prefs[PreferenceKeys.USER_THEMES])
        assertEquals(null, prefs[PreferenceKeys.SELECTED_THEME_ID])
        assertTrue(PreferenceKeys.COLOR_SETTINGS in prefs)
    }

    @Test
    fun savedThemes_stayAsWritten() {
        // A saved theme keeps its stored values (no seeding rewrite of its empty colors); the carried one is appended.
        val saved = "[{\"id\":\"id-0\",\"name\":\"Mine\",\"appearance\":{\"colors\":{}},\"createdAt\":1,\"updatedAt\":1}]"
        val prefs = customizedLegacyLook().apply { this[PreferenceKeys.USER_THEMES] = saved }

        retire(prefs)

        val first = JSONArray(prefs[PreferenceKeys.USER_THEMES]).getJSONObject(0)
        assertEquals(0, first.getJSONObject("appearance").getJSONObject("colors").length())
        assertEquals(listOf("id-0", NEW_ID), themes(prefs).map { it.id })
    }

    private companion object {
        const val NOW = 1_700_000_000_000L
        const val NEW_ID = "6ba7b810-9dad-11d1-80b4-00c04fd430c8"
        val RED_BACKGROUND = KeyboardColorSettings(background = ThemeBackground.Solid(0xFFFF0000.toInt()))
    }
}
