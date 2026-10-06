package com.siansiansu.taigikeyboard.ime.settings

/**
 * How the rendered romanization separates syllables (Syllable Separator,
 * `behavioral-invariants.md` §49): the dictionary hyphen (`tâi-uân`), a space
 * (`tâi uân`), or nothing (`tâiuân`). Sent as stored; the engine rewrites the
 * romanization and leaves the TPS layout alone (`AppConfig::rendered_syllable_joiner`).
 *
 * Mirrors iOS `Settings/SettingsModels.swift` `SyllableSeparator`. Pure Kotlin
 * enum — no platform dependencies. [storageValue] is the raw string persisted
 * under `PreferenceKeys.SYLLABLE_SEPARATOR`; identical across platforms.
 */
enum class SyllableSeparator(
    val storageValue: String,
) {
    HYPHEN("hyphen"),
    SPACE("space"),
    NONE("none"),
    ;

    companion object {
        /** Coerce a stored raw string into a separator; unknown / absent values fall back to [HYPHEN]. */
        fun fromStorage(raw: String?): SyllableSeparator = entries.firstOrNull { it.storageValue == raw } ?: HYPHEN
    }
}
