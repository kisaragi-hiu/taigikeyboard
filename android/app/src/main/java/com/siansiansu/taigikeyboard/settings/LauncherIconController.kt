package com.siansiansu.taigikeyboard.settings

import android.content.ComponentName
import android.content.Context
import android.content.pm.PackageManager
import com.siansiansu.taigikeyboard.ime.core.CompositionRoot

// Restores the launcher icon (an activity alias) to its manifest default once.
//
// The retired `advanced__show_app_icon` setting could disable the alias, and
// a disabled component state outlives both the setting and the app's updates.
// Nothing hides the icon any more, so an install that still has it hidden is
// put back to the manifest state on the first start after the upgrade.
object LauncherIconController {
    private const val TAG = "LauncherIconController"
    private const val SETTINGS_ACTIVITY_NAME = "com.siansiansu.taigikeyboard.SettingsLauncherAlias"

    /** `true` once the component state is the manifest default. */
    fun restoreManifestDefault(context: Context): Boolean =
        try {
            context.packageManager.setComponentEnabledSetting(
                ComponentName(context, SETTINGS_ACTIVITY_NAME),
                PackageManager.COMPONENT_ENABLED_STATE_DEFAULT,
                PackageManager.DONT_KILL_APP,
            )
            true
        } catch (e: IllegalArgumentException) {
            CompositionRoot.shared(context).logger.w(TAG, "[LAUNCHER] alias restore refused: ${e.message}", e)
            false
        }
}
