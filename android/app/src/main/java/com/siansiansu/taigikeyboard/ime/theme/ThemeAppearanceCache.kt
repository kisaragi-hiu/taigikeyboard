package com.siansiansu.taigikeyboard.ime.theme

import android.content.Context
import android.content.res.Configuration
import com.siansiansu.taigikeyboard.ime.settings.PrefHelper

/**
 * Caches the resolved [ThemeAppearance] so the per-keystroke render path does not
 * re-parse the userThemes JSON on every call. Re-resolves through
 * [ThemeResolver] ONLY when one of the resolver inputs flips: the selected theme id,
 * the userThemes JSON, or the night-mode flag.
 *
 * One instance per consumer ([KeyboardAppearanceResolver] for keys, [SmartbarManager]
 * for the candidate strip). Resolution is pure + deterministic, so the two caches stay
 * in lockstep without sharing mutable state. `fontType` is NOT a theme input and
 * stays outside this cache.
 */
internal class ThemeAppearanceCache(
    private val prefs: PrefHelper,
) {
    private var cachedKey: Key? = null
    private var cached: ThemeAppearance = ThemeAppearance.DEFAULT

    fun resolve(isDark: Boolean): ThemeAppearance {
        val key = Key(
            selectedThemeId = prefs.selectedThemeId,
            userThemesJson = prefs.userThemes,
            isDark = isDark,
        )
        if (key != cachedKey) {
            cachedKey = key
            // Resolve through the single PrefHelper entry point so the resolver-input
            // wiring (selected id + userThemes) lives in ONE place; the cache only
            // adds the re-resolve gate that entry point's doc says it lacks.
            cached = prefs.resolvedAppearance(key.isDark)
        }
        return cached
    }

    /** Snapshot of every [ThemeResolver] input; equality drives the re-resolve gate. */
    private data class Key(
        val selectedThemeId: String,
        val userThemesJson: String,
        val isDark: Boolean,
    )
}

/**
 * Whether the keyboard should render its dark-mode theme variant. Reads the live
 * configuration so a night-mode flip is picked up on the next appearance push.
 */
internal fun isKeyboardNightMode(context: Context): Boolean =
    (context.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) ==
        Configuration.UI_MODE_NIGHT_YES
