package com.siansiansu.taigikeyboard.ime.theme

import androidx.datastore.preferences.core.MutablePreferences
import androidx.datastore.preferences.core.Preferences
import com.siansiansu.taigikeyboard.ime.settings.PreferenceKeys
import org.json.JSONArray
import org.json.JSONException
import java.util.UUID

/**
 * The pre-theme global appearance: the six retired keys the old appearance screen wrote.
 * Mirrors iOS `SharedSettings.retireLegacyAppearance`.
 */
internal object LegacyAppearance {
    private val KEYS: List<Preferences.Key<*>> =
        listOf(
            PreferenceKeys.KEY_HEIGHT_SCALE,
            PreferenceKeys.KEY_FONT_SIZE_SCALE,
            PreferenceKeys.CANDIDATE_TEXT_SIZE_SCALE,
            PreferenceKeys.KEY_CORNER_RADIUS,
            PreferenceKeys.KEY_BORDER_WIDTH,
            PreferenceKeys.COLOR_SETTINGS,
        )

    /**
     * Carries a look customized on the retired screen into a user theme named [themeName] and
     * selects it when the keyboard was showing that look (the "default" theme, or an id nothing
     * resolves), then removes the retired keys; a factory look is only removed. Runs inside the
     * caller's DataStore transaction, so the theme and the removal land together or not at all,
     * and does nothing once the keys are gone. Past the user-theme cap: a look the user made is
     * never refused.
     */
    fun retire(
        prefs: MutablePreferences,
        themeName: String,
        now: Long,
        newId: String = UUID.randomUUID().toString(),
    ) {
        if (KEYS.none { it in prefs }) return
        val legacy = storedAppearance(prefs)
        if (legacy != ThemeAppearance.DEFAULT) {
            // A saved list that does not read back whole is left alone, and the keys with it.
            val saved = readableThemes(prefs[PreferenceKeys.USER_THEMES]) ?: return
            val selected = prefs[PreferenceKeys.SELECTED_THEME_ID] ?: ThemeId.DEFAULT
            val wasShowingLegacy =
                selected == ThemeId.DEFAULT ||
                    (BuiltInThemes.all.none { it.id == selected } && savedIds(saved).none { it == selected })
            val theme = UserTheme(id = newId, name = themeName, appearance = legacy, createdAt = now, updatedAt = now)
            // Appended to the raw list, so the saved themes keep their stored values (no seeding rewrite).
            prefs[PreferenceKeys.USER_THEMES] = saved.put(theme.toJson()).toString()
            if (wasShowingLegacy) prefs[PreferenceKeys.SELECTED_THEME_ID] = theme.id
        }
        KEYS.forEach { prefs.remove(it) }
    }

    /** The saved theme list, or null when it is not a JSON array of objects. */
    private fun readableThemes(json: String?): JSONArray? {
        if (json.isNullOrBlank()) return JSONArray()
        val array =
            try {
                JSONArray(json)
            } catch (e: JSONException) {
                return null
            }
        return array.takeIf { (0 until it.length()).all { index -> it.optJSONObject(index) != null } }
    }

    private fun savedIds(saved: JSONArray): List<String> = (0 until saved.length()).map { saved.getJSONObject(it).optString("id") }

    /** The look the retired keys hold; an absent key reads as its factory value, shadow as none. */
    private fun storedAppearance(prefs: MutablePreferences): ThemeAppearance =
        ThemeAppearance(
            colors = KeyboardColorSettings.fromJson(prefs[PreferenceKeys.COLOR_SETTINGS] ?: "{}"),
            keyHeightScale = prefs[PreferenceKeys.KEY_HEIGHT_SCALE] ?: ThemeAppearance.DEFAULT_KEY_HEIGHT_SCALE,
            keyFontSizeScale = prefs[PreferenceKeys.KEY_FONT_SIZE_SCALE] ?: ThemeAppearance.DEFAULT_KEY_FONT_SIZE_SCALE,
            candidateTextSizeScale =
                prefs[PreferenceKeys.CANDIDATE_TEXT_SIZE_SCALE] ?: ThemeAppearance.DEFAULT_CANDIDATE_TEXT_SIZE_SCALE,
            keyCornerRadius = prefs[PreferenceKeys.KEY_CORNER_RADIUS] ?: ThemeAppearance.DEFAULT_KEY_CORNER_RADIUS,
            keyBorderWidth = prefs[PreferenceKeys.KEY_BORDER_WIDTH] ?: ThemeAppearance.DEFAULT_KEY_BORDER_WIDTH,
        )
}
