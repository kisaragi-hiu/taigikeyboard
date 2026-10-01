package com.siansiansu.taigikeyboard.ime.theme

/**
 * Resolves the active theme id into a [ThemeAppearance] for rendering. Pure +
 * deterministic (no keyboard runtime needed) so it is directly unit-testable.
 * Mirrors iOS ThemeResolver. Handles:
 *
 * - [ThemeId.DEFAULT] -> the factory appearance (all-null adaptive colors).
 * - a known user-theme id -> that theme's full appearance.
 * - a known built-in id -> factory sizes + the built-in's night-mode color variant.
 * - an unknown id (deleted user theme / stale built-in) -> the factory appearance,
 *   so the keyboard never renders an empty/broken theme.
 */
object ThemeResolver {
    fun resolved(
        themeId: String,
        isDark: Boolean,
        userThemes: List<UserTheme>,
        builtInThemes: List<BuiltInTheme> = BuiltInThemes.all,
    ): ThemeAppearance {
        if (themeId == ThemeId.DEFAULT) return ThemeAppearance.DEFAULT
        // Gate the user branch on a UUID-shaped id (mirrors iOS `id.uuidString == themeId`):
        // a built-in id like "standardBlue" can never resolve to a user theme, so a corrupt
        // persisted user theme carrying a built-in id cannot shadow the built-in.
        if (ThemeId.isUserTheme(themeId)) {
            userThemes.firstOrNull { it.id == themeId }?.let { return it.appearance }
        }
        builtInThemes.firstOrNull { it.id == themeId }?.let { builtIn ->
            // Built-in themes are colors-first -> factory sizes + their night-mode
            // color variant, plus an optional per-theme appearance override
            // (keyBorderWidth, used by the Outlined family).
            return ThemeAppearance.DEFAULT.copy(
                colors = builtIn.colors(isDark),
                keyBorderWidth = builtIn.keyBorderWidth ?: ThemeAppearance.DEFAULT.keyBorderWidth,
            )
        }
        return ThemeAppearance.DEFAULT
    }
}
