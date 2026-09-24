package com.siansiansu.taigikeyboard.ui.tabs.settings

// Sub-screen for selecting the active input mode (POJ / TL / English / TPS).

import androidx.compose.runtime.Composable
import com.siansiansu.taigikeyboard.i18n.generated.L10n
import com.siansiansu.taigikeyboard.i18n.generated.StringKey
import com.siansiansu.taigikeyboard.i18n.stringRes
import com.siansiansu.taigikeyboard.ui.components.SelectionListScreen

// Shared input-mode key→label-key pairs, used by InputModeScreen and InputSettingsScreen.
// Structural (no resolved strings) so it stays class-load safe; the label is resolved at render.
val inputModeOptions: List<Pair<String, StringKey>> =
    listOf(
        "poj" to StringKey.SETTINGS_POJ_MODE,
        "tl" to StringKey.SETTINGS_TL_MODE,
        "english" to StringKey.SETTINGS_ENGLISH_MODE,
        "tps" to StringKey.SETTINGS_TPS_MODE,
    )

@Composable
fun inputModeDisplayName(mode: String): String = stringRes(inputModeOptions.firstOrNull { it.first == mode }?.second ?: StringKey.SETTINGS_TL_MODE)

@Composable
fun InputModeScreen(
    selectedMode: String,
    onModeSelected: (String) -> Unit,
    onBack: () -> Unit,
) {
    SelectionListScreen(
        title = L10n.settingsInputMode,
        options = inputModeOptions,
        selected = selectedMode,
        onSelected = onModeSelected,
        onBack = onBack,
    )
}
