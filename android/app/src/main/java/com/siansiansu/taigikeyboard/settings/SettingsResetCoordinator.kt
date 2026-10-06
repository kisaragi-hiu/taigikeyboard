package com.siansiansu.taigikeyboard.settings

import com.siansiansu.taigikeyboard.ime.settings.PrefHelper

// Resets every setting to its default (Settings tab "Reset Settings"), mirroring
// ios/Sources/TaigiKeyboard/Settings/SettingsResetCoordinator.swift.
//
// Settings only: learning records and the custom dictionary are user data, cleared from their own
// Dictionary-tab pages, never from here.
object SettingsResetCoordinator {
    suspend fun resetAll(prefs: PrefHelper) {
        prefs.resetToDefaults()
    }
}
