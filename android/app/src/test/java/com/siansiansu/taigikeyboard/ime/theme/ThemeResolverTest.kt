package com.siansiansu.taigikeyboard.ime.theme

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNotNull
import org.junit.Test
import java.util.UUID

/**
 * Tests for [ThemeResolver] — the pure mapping from a selected theme id to the
 * [ThemeAppearance] the renderer consumes. Mirrors iOS ThemeResolverTests.
 */
class ThemeResolverTest {
    private fun customizedColors(): KeyboardColorSettings = KeyboardColorSettings(background = ThemeBackground.Solid(RED))

    private fun appearance(
        colors: KeyboardColorSettings = KeyboardColorSettings(),
        shadow: Float = 0f,
    ): ThemeAppearance = ThemeAppearance.DEFAULT.copy(colors = colors, keyShadowIntensity = shadow)

    private fun userTheme(
        id: String,
        appearance: ThemeAppearance,
    ): UserTheme = UserTheme(id = id, name = "T", appearance = appearance, createdAt = 0L, updatedAt = 0L)

    @Test
    fun resolved_default_returnsFactoryAppearance() {
        val resolved = ThemeResolver.resolved(ThemeId.DEFAULT, false, emptyList())
        assertEquals(ThemeAppearance.DEFAULT, resolved)
    }

    @Test
    fun resolved_knownUserTheme_carriesFullAppearance() {
        val id = UUID.randomUUID().toString()
        val app = appearance(customizedColors(), shadow = 0.3f).copy(keyHeightScale = 1.1f, keyFontSizeScale = 0.9f)
        val resolved = ThemeResolver.resolved(id, true, listOf(userTheme(id, app)))
        assertEquals(app, resolved)
    }

    @Test
    fun resolved_unknownId_fallsBackToFactoryAppearance() {
        val resolved = ThemeResolver.resolved("no_such_theme", false, emptyList())
        assertEquals(ThemeAppearance.DEFAULT, resolved)
    }

    @Test
    fun resolved_deletedUserTheme_fallsBackToFactoryAppearance() {
        val staleId = UUID.randomUUID().toString()
        val other = userTheme(UUID.randomUUID().toString(), appearance(customizedColors()))
        val resolved = ThemeResolver.resolved(staleId, false, listOf(other))
        assertEquals(ThemeAppearance.DEFAULT, resolved)
    }

    @Test
    fun resolved_builtInLight_resolvesThroughCatalog() {
        val expected = BuiltInThemes.theme("standardBlue")!!
        val resolved = ThemeResolver.resolved("standardBlue", false, emptyList())
        assertEquals(expected.colors(false), resolved.colors)
        assertNotEquals(KeyboardColorSettings(), resolved.colors)
        assertNotNull(resolved.colors.backgroundGradient)
        assertEquals(0f, resolved.keyShadowIntensity, 0f)
    }

    @Test
    fun resolved_builtInDark_resolvesThroughCatalog() {
        val expected = BuiltInThemes.theme("standardBlue")!!
        val resolved = ThemeResolver.resolved("standardBlue", true, emptyList())
        assertEquals(expected.colors(true), resolved.colors)
    }

    @Test
    fun resolved_builtIn_usesFactorySizes() {
        val resolved = ThemeResolver.resolved("standardBlue", true, emptyList())
        assertEquals(ThemeAppearance.DEFAULT.keyHeightScale, resolved.keyHeightScale, 0f)
        assertEquals(ThemeAppearance.DEFAULT.keyFontSizeScale, resolved.keyFontSizeScale, 0f)
        assertEquals(ThemeAppearance.DEFAULT.candidateTextSizeScale, resolved.candidateTextSizeScale, 0f)
        assertEquals(ThemeAppearance.DEFAULT.keyCornerRadius, resolved.keyCornerRadius, 0f)
        assertEquals(ThemeAppearance.DEFAULT.keyBorderWidth, resolved.keyBorderWidth, 0f)
        assertEquals(0f, resolved.keyShadowIntensity, 0f)
    }

    // An Outlined family theme (id "framedBlue") carries keyBorderWidth=1.0 ON TOP of
    // factory sizes; other scalars stay factory.
    @Test
    fun resolved_framedFamily_carriesKeyBorderWidth() {
        val resolved = ThemeResolver.resolved("framedBlue", false, emptyList())
        assertEquals(1.0f, resolved.keyBorderWidth, 0f)
        assertNotEquals(ThemeAppearance.DEFAULT.keyBorderWidth, resolved.keyBorderWidth)
        assertEquals(ThemeAppearance.DEFAULT.keyCornerRadius, resolved.keyCornerRadius, 0f)
    }

    // Filled / Borderless family themes keep the factory border (0).
    @Test
    fun resolved_classicAndCleanFamilies_keepFactoryKeyBorderWidth() {
        for (id in listOf("standardBlue", "cleanBlue")) {
            val resolved = ThemeResolver.resolved(id, false, emptyList())
            assertEquals(ThemeAppearance.DEFAULT.keyBorderWidth, resolved.keyBorderWidth, 0f)
        }
    }

    @Test
    fun resolved_builtIn_winsOverUnrelatedUserThemes() {
        val other = userTheme(UUID.randomUUID().toString(), appearance(customizedColors()))
        val resolved = ThemeResolver.resolved("standardBlue", true, listOf(other))
        assertEquals(BuiltInThemes.theme("standardBlue")!!.colors(true), resolved.colors)
    }

    @Test
    fun resolved_corruptUserThemeWithBuiltInId_doesNotShadowBuiltIn() {
        // A persisted user theme carrying a built-in id must NOT shadow the built-in:
        // the user branch is gated on a UUID-shaped id, so "standardBlue" resolves to
        // the built-in palette, not the bogus user theme.
        val bogus = userTheme("standardBlue", appearance(customizedColors()))
        val resolved = ThemeResolver.resolved("standardBlue", false, listOf(bogus))
        assertEquals(BuiltInThemes.theme("standardBlue")!!.colors(false), resolved.colors)
    }

    private companion object {
        const val RED = 0xFFFF0000.toInt()
    }
}
