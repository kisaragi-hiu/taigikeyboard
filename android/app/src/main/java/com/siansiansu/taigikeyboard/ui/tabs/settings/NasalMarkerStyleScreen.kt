package com.siansiansu.taigikeyboard.ui.tabs.settings

// Sub-screen for Nasal mark in POJ capitals (§53): ᴺ or ⁿ, over the stored switch.

import androidx.compose.runtime.Composable
import com.siansiansu.taigikeyboard.i18n.generated.L10n
import com.siansiansu.taigikeyboard.i18n.generated.StringKey
import com.siansiansu.taigikeyboard.i18n.stringRes
import com.siansiansu.taigikeyboard.ui.components.SelectionListScreen

// Stored switch→label-key pairs, ᴺ (the default) first; used by NasalMarkerStyleScreen and the
// InputSettingsScreen row. Structural (no resolved strings); resolved at render.
val nasalMarkerStyleOptions: List<Pair<Boolean, StringKey>> =
    listOf(
        true to StringKey.SETTINGS_NASAL_MARKER_UPPERCASE_CAPITAL,
        false to StringKey.SETTINGS_NASAL_MARKER_UPPERCASE_SMALL,
    )

@Composable
fun nasalMarkerStyleDisplayName(isUppercase: Boolean): String = stringRes(nasalMarkerStyleOptions.first { it.first == isUppercase }.second)

@Composable
fun NasalMarkerStyleScreen(
    isUppercase: Boolean,
    onSelected: (Boolean) -> Unit,
    onBack: () -> Unit,
) {
    SelectionListScreen(
        title = L10n.settingsNasalMarkerUppercase,
        options = nasalMarkerStyleOptions,
        selected = isUppercase,
        onSelected = onSelected,
        onBack = onBack,
    )
}
