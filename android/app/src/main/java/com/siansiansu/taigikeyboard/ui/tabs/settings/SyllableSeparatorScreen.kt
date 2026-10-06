package com.siansiansu.taigikeyboard.ui.tabs.settings

// Sub-screen for the Syllable Separator (§49): hyphen, space, or none.

import androidx.compose.runtime.Composable
import com.siansiansu.taigikeyboard.i18n.generated.L10n
import com.siansiansu.taigikeyboard.i18n.generated.StringKey
import com.siansiansu.taigikeyboard.i18n.stringRes
import com.siansiansu.taigikeyboard.ime.settings.SyllableSeparator
import com.siansiansu.taigikeyboard.ui.components.SelectionListScreen

// Separator→label-key pairs, the hyphen (the default) first; used by SyllableSeparatorScreen and
// the InputSettingsScreen row. Structural (no resolved strings); resolved at render.
val syllableSeparatorOptions: List<Pair<SyllableSeparator, StringKey>> =
    listOf(
        SyllableSeparator.HYPHEN to StringKey.DESKTOP_TELEX_GUIDE_HYPHEN,
        SyllableSeparator.SPACE to StringKey.SETTINGS_SYLLABLE_SEPARATOR_SPACE,
        SyllableSeparator.NONE to StringKey.SETTINGS_SYLLABLE_SEPARATOR_NONE,
    )

@Composable
fun syllableSeparatorDisplayName(separator: SyllableSeparator): String = stringRes(syllableSeparatorOptions.first { it.first == separator }.second)

@Composable
fun SyllableSeparatorScreen(
    selected: SyllableSeparator,
    onSelected: (SyllableSeparator) -> Unit,
    onBack: () -> Unit,
) {
    SelectionListScreen(
        title = L10n.settingsSyllableSeparator,
        options = syllableSeparatorOptions,
        selected = selected,
        onSelected = onSelected,
        onBack = onBack,
    )
}
