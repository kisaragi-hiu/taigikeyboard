package com.siansiansu.taigikeyboard.ui.components

import androidx.compose.material.icons.Icons
import androidx.compose.ui.graphics.vector.ImageVector

/**
 * Shared icon definitions for settings screens.
 *
 * Used by InputSettingsScreen (Settings tab); mirrored by iOS SettingsIcons.swift.
 */
object SettingsIcons {
    val toolbar: ImageVector get() = Icons.Outlined.ViewStream
    val globe: ImageVector get() = Icons.Outlined.Language
    val sound: ImageVector get() = Icons.AutoMirrored.Outlined.VolumeUp
    val vibration: ImageVector get() = Icons.Outlined.Vibration
}
