package com.siansiansu.taigikeyboard.ime.theme

import android.content.res.Configuration

// A user theme renders light whatever the system night mode: the IME (TaigiKeyboard.lightContext)
// and the theme editor preview (KeyboardPreviewPanel) both resolve resources under this copy.
fun Configuration.withNightModeOff(): Configuration =
    Configuration(this).apply {
        uiMode = (uiMode and Configuration.UI_MODE_NIGHT_MASK.inv()) or Configuration.UI_MODE_NIGHT_NO
    }
