// The one AppConfig builder every Android bridge request carries, plus its settings projections.
// Top-level (not on `RustEngineBridge`, whose initializer loads the engine) so JVM tests can pin the wire.

package com.siansiansu.taigikeyboard.engine

import com.siansiansu.taigikeyboard.engine.proto.AppConfig
import com.siansiansu.taigikeyboard.engine.proto.Platform
import com.siansiansu.taigikeyboard.ime.settings.CandidateDisplayMode
import com.siansiansu.taigikeyboard.ime.settings.EngineSettings
import com.siansiansu.taigikeyboard.ime.settings.InputMode
import com.siansiansu.taigikeyboard.ime.settings.PojMarkerOptions
import com.siansiansu.taigikeyboard.engine.proto.CandidateDisplayMode as ProtoCandidateDisplayMode

/**
 * The one [AppConfig] builder: every request the Android bridges send carries
 * a config built here, so `platform_id` is set once. Composing passes the live
 * settings ([continuousAppConfig]), nextword the swap (plus the display fields
 * on its predict request), case transform the nasal-marker switch; a field a
 * request family does not read keeps its proto default. `pojMarkers == null`
 * leaves the three POJ marker fields at theirs (no folds, the marker follows
 * the case).
 *
 * [inputMode] is the engine's `input_mode` string ([engineInputMode]). TPS goes
 * out as `"tps"` with the swap and No Hyphens exactly as the settings hold
 * them: the engine applies the TPS fold itself (`AppConfig::renders_hanji_first`
 * / `renders_hyphenless`, `engine/protos/src/lib.rs`).
 *
 * CROSS-PLATFORM INVARIANT — mirrors ios/Sources/TaigiKeyboard/Engine/RustEngineBridge.swift appConfig.
 * Drift causes silent divergence (one platform renders TPS or the swap differently).
 */
internal fun appConfig(
    inputMode: String,
    pojMarkers: PojMarkerOptions? = null,
    isTranslateSwapped: Boolean = false,
    isOutputBothScripts: Boolean = false,
    candidateDisplayMode: CandidateDisplayMode = CandidateDisplayMode.SIDE_BY_SIDE,
    isHyphenlessRomanEnabled: Boolean = false,
    isTpsOrMappedToER: Boolean = false,
): AppConfig =
    AppConfig
        .newBuilder()
        .setPlatformId(Platform.PLATFORM_ANDROID)
        .setInputMode(inputMode)
        .apply {
            if (pojMarkers != null) {
                setOoDoubletapEnabled(pojMarkers.isDoubleTapOOEnabled)
                setNnDoubletapEnabled(pojMarkers.isDoubleTapNNEnabled)
                // Inverted on the wire (proto default = the marker follows the case, §53).
                setForceLowercaseNasalMarker(!pojMarkers.isNasalMarkerUppercaseEnabled)
            }
        }.setIsTranslateSwapped(isTranslateSwapped)
        .setOutputBothScripts(isOutputBothScripts)
        .setCandidateDisplayMode(candidateDisplayMode.toProto())
        .setHyphenlessRoman(isHyphenlessRomanEnabled)
        .setTpsOrMapsToEr(isTpsOrMappedToER)
        .build()

/**
 * The composing [AppConfig]: the live settings through [appConfig], including
 * the two v3.5.8 §10.2 word-boundary-spacing flags the engine's
 * `continuous_word_space` predicate consumes.
 *
 * The swap is the Candidate-Display-projected setting; a TPS layout is sent as
 * `"tps"` and the engine reads it as Hanji-first itself
 * (`AppConfig::renders_hanji_first`). `outputBothScripts` distinguishes
 * hanji-first (no inter-segment space) from both-scripts (`hit (彼)` — space
 * wanted); `is_translate_swapped` is `true` for both, so the second flag is
 * required. An unknown stored mode composes as TL.
 *
 * Every composing op that renders the composition sends it — under
 * Model B that is every mutation and every snapshot, not only the
 * commits: `Append` / `DeleteBackward` after a nail re-render the
 * nailed prefix through `combined_display(nailed, raw, config)` too,
 * so a nail and the keystroke after it must agree on the prefix
 * (the 2026-05-18 "commit entry points only" split left Hanji-first
 * showing `台 gi` while typing after `台`; desktop closed the same
 * drift in #31, S37). Only `Reset`, which carries no config, stays
 * outside.
 *
 * CROSS-PLATFORM INVARIANT — mirrors ios/Sources/TaigiKeyboard/Engine/RustEngineBridge+Composing.swift continuousAppConfig.
 * Drift causes silent divergence (hanji-first spurious word-boundary spaces).
 */
internal fun continuousAppConfig(settings: EngineSettings): AppConfig =
    appConfig(
        inputMode = engineInputMode(settings.inputMode, unknownAs = "tl"),
        pojMarkers = settings.pojMarkerOptions,
        isTranslateSwapped = settings.isTranslateSwapped,
        isOutputBothScripts = settings.isOutputBothScripts,
        candidateDisplayMode = settings.candidateDisplayMode,
        isHyphenlessRomanEnabled = settings.isHyphenlessRomanEnabled,
        isTpsOrMappedToER = settings.isTpsOrMappedToER,
    )

/**
 * The engine `input_mode` for a stored `inputMode` preference: the four modes
 * pass through, anything else reads as [unknownAs]. Composing has always read
 * an unknown value as TL, nextword as POJ ([InputMode.fromPrefString]); the
 * settings UI writes only the four, so the split is not observable.
 */
internal fun engineInputMode(
    storedInputMode: String,
    unknownAs: String,
): String = if (storedInputMode in ENGINE_INPUT_MODES) storedInputMode else unknownAs

private val ENGINE_INPUT_MODES = setOf("tl", "poj", "tps", "english")

/**
 * The engine `input_mode` for [this]. Android's enum has no TPS case (`"tps"`
 * settings collapse to [InputMode.TL] in `InputMode.fromPrefString`), which the
 * engine composes and cases exactly like `"tps"` (`phonetics::api::composing_mode`).
 */
internal fun InputMode.engineInputMode(): String =
    when (this) {
        InputMode.POJ -> "poj"
        InputMode.TL -> "tl"
        InputMode.ENGLISH -> "english"
    }

/**
 * `AppConfig.candidate_display_mode` (field 9). The engine collapses
 * same-roman candidate rows itself under ROMAN_ONLY (`handle_fetch_at_pos`
 * + nextword `filter`); every other request family ignores the field.
 */
internal fun CandidateDisplayMode.toProto(): ProtoCandidateDisplayMode =
    when (this) {
        CandidateDisplayMode.SIDE_BY_SIDE -> ProtoCandidateDisplayMode.CANDIDATE_DISPLAY_MODE_SIDE_BY_SIDE
        CandidateDisplayMode.ROMAN_ONLY -> ProtoCandidateDisplayMode.CANDIDATE_DISPLAY_MODE_ROMAN_ONLY
        CandidateDisplayMode.COMBINED -> ProtoCandidateDisplayMode.CANDIDATE_DISPLAY_MODE_COMBINED
    }
