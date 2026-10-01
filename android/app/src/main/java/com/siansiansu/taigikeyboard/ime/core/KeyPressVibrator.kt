// Plays key-press vibration through a direct Vibrator so the app's own
// vibration-feedback toggle is the source of truth for whether typing
// vibrates — independent of the OS-level "haptic feedback / touch vibration"
// setting.
//
// Why not View.performHapticFeedback(KEYBOARD_TAP): that call forwards
// always=false to VibratorManagerService, which suppresses the KEYBOARD_TAP
// (USAGE_TOUCH) effect whenever Settings.System.HAPTIC_FEEDBACK_ENABLED is off
// — so the in-app toggle silently did nothing on devices (e.g. Samsung) that
// ship the OS-level touch-haptic setting off. Requires the
// android.permission.VIBRATE normal permission. Mirrors the direct-Vibrator
// approach used by florisboard / aiongtaigi-sushi / moe_taigi.
//
// Bypassing the OS touch-haptic gate is deliberate; it does NOT bypass the
// device master vibration switch, battery-saver, or OEM policy — those still
// suppress vibration, which is the intended platform behaviour.

package com.siansiansu.taigikeyboard.ime.core

import android.content.Context
import android.os.Build
import android.os.VibrationEffect
import android.os.Vibrator
import android.os.VibratorManager
import com.siansiansu.taigikeyboard.ime.settings.PrefHelper

class KeyPressVibrator(
    context: Context,
    private val prefs: PrefHelper,
) {
    private val vibrator: Vibrator? = resolveVibrator(context)

    /** Vibrates for one key press when the app toggle is on. Re-reads the
     *  pref live each call so a settings change takes effect immediately.
     *  Safe to call from the touch callback on the main thread. */
    fun vibrate() {
        if (!prefs.isVibrationFeedbackEnabled) return
        val vib = vibrator?.takeIf { it.hasVibrator() } ?: return
        // Either effect vibrates with the default USAGE (UNKNOWN), which keeps
        // it off the OS touch-haptic gate — see behavioral-invariants.md §36.
        val effect =
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                // EFFECT_CLICK is the closest predefined match to the AOSP
                // KEYBOARD_TAP mapping (EFFECT_TICK is noticeably weaker).
                VibrationEffect.createPredefined(VibrationEffect.EFFECT_CLICK)
            } else {
                // createPredefined is API 29+; API 28 gets a 20 ms
                // default-amplitude one-shot — a brief tap, matching key-press feel.
                VibrationEffect.createOneShot(20L, VibrationEffect.DEFAULT_AMPLITUDE)
            }
        vib.vibrate(effect)
    }

    private fun resolveVibrator(context: Context): Vibrator? =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            (context.getSystemService(Context.VIBRATOR_MANAGER_SERVICE) as? VibratorManager)?.defaultVibrator
        } else {
            @Suppress("DEPRECATION")
            context.getSystemService(Context.VIBRATOR_SERVICE) as? Vibrator
        }
}
