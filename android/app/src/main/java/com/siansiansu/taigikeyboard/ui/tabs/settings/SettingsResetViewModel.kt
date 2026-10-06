package com.siansiansu.taigikeyboard.ui.tabs.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.siansiansu.taigikeyboard.ime.settings.PrefHelper
import com.siansiansu.taigikeyboard.settings.SettingsResetCoordinator
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

// ViewModel for the "Reset all settings" flow in InputSettingsScreen.
// Owns the coroutine launch + resetCounter signal that InputSettingsScreen
// observes via remember(resetCounter) to re-read prefs after a reset.
//
// Activity supplies its warmed PrefHelper via the method param (matches
// DataManagementViewModel.exportBackup shape) and an onResult callback so the
// Toast stays at Activity scope. Settings only — learning records are cleared from their own page.
class SettingsResetViewModel : ViewModel() {
    private val _resetCounter = MutableStateFlow(0)
    val resetCounter: StateFlow<Int> = _resetCounter.asStateFlow()

    fun resetAllSettings(
        prefs: PrefHelper,
        onResult: (success: Boolean) -> Unit,
    ) {
        viewModelScope.launch {
            val success =
                try {
                    SettingsResetCoordinator.resetAll(prefs)
                    _resetCounter.value++
                    true
                } catch (_: Exception) {
                    false
                }
            onResult(success)
        }
    }
}
