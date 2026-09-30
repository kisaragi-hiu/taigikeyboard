// Candidate click handler, extracted from SmartbarManager: handles Taigi candidate selection,
// English suggestion replacement, and overlay candidate selection, and formats the output text.

package com.siansiansu.taigikeyboard.ime.text.smartbar

import com.siansiansu.taigikeyboard.engine.RustEngineBridge
import com.siansiansu.taigikeyboard.engine.proto.CommitScript
import com.siansiansu.taigikeyboard.engine.tlDisplayToTps
import com.siansiansu.taigikeyboard.ime.core.PrefHelper
import com.siansiansu.taigikeyboard.ime.core.TaigiKeyboard
import com.siansiansu.taigikeyboard.ime.core.logging.TraceContext
import com.siansiansu.taigikeyboard.ime.core.logging.TraceId
import com.siansiansu.taigikeyboard.ime.core.logging.debug
import com.siansiansu.taigikeyboard.ime.dictionary.TaigiWord
import com.siansiansu.taigikeyboard.ime.text.composing.Usage
import com.siansiansu.taigikeyboard.ime.text.composing.UsageRecorder

/**
 * Handles candidate click events extracted from SmartbarManager.
 *
 * Processes Taigi candidate selection, English suggestion replacement,
 * and overlay suggestion selection with proper text output formatting.
 */
class CandidateClickHandler(
    private val prefs: PrefHelper,
    private val taigikeyboard: TaigiKeyboard,
    private val usage: UsageRecorder,
    private val getCurrentSuggestions: () -> List<TaigiWord>,
    private val getIsTranslateSwapped: () -> Boolean,
    private val getOutputBothScripts: () -> Boolean,
    private val getComposingManager: () -> com.siansiansu.taigikeyboard.ime.text.composing.ComposingManager?,
    private val onClearCandidates: () -> Unit,
    private val onNextWordPrediction: (displayText: String, roman: String) -> Unit,
    /**
     * Schedule a Taigi candidate recompute. Called only after a Continuous
     * mid-commit where the engine's `PerformAutocomplete` effect alone is
     * not enough to refresh the strip with the post-commit pending span.
     */
    private val onRequestCandidateRefresh: () -> Unit = {},
) {
    private val logger get() = taigikeyboard.compositionRoot.logger

    /**
     * Handle candidate click from RecyclerView.
     */
    fun handleCandidateClick(
        selectedWord: TaigiWord,
        index: Int,
    ) {
        TraceContext.withTrace(TraceId.next()) {
            logger.debug(TAG) { "[INPUT] fn=handleCandidateClick gesture=candidate-tap" }
            taigikeyboard.beginInputEvent()

            if (logger.isDebugEnabled) {
                val isNextWord = getCurrentSuggestions().firstOrNull()?.id?.let { it < 0 } ?: false
                logger.d(
                    TAG,
                    "[CLICK-ENTRY] onClick triggered, isNextWordMode=$isNextWord, suggestionsCount=${getCurrentSuggestions().size}",
                )
                logger.d(TAG, "[CLICK] index=$index, suggestionsSize=${getCurrentSuggestions().size}")
            }

            val ic = taigikeyboard.currentInputConnection ?: return

            val composingManager = getComposingManager()

            // Continuous-input branch routes BEFORE the sentinel-id branches.
            // The engine emits NextWordWordSelected on final commits which the
            // ComposingManager NextWordEffectRouter routes — onNextWordPrediction
            // is intentionally NOT called from the continuous branch to avoid
            // double-firing predictions.
            if (selectedWord.additionalInfo[TaigiWord.MetadataKeys.IS_CONTINUOUS] == "true" && composingManager != null) {
                handleContinuousCandidateClick(selectedWord, ic, composingManager)
                return@withTrace
            }

            val isEnglishSuggestion = selectedWord.id <= -100
            val isNextWordPrediction = selectedWord.id < 0 && !isEnglishSuggestion

            val cachedIsTranslateSwapped = getIsTranslateSwapped()
            val cachedOutputBothScripts = getOutputBothScripts()
            val isTPSLayout = prefs.isTpsLayout
            val effectiveSwapped = isTPSLayout || cachedIsTranslateSwapped

            val resolved =
                if (isEnglishSuggestion) {
                    // An English word IS Latin text, so it takes the spacing.
                    ResolvedCommit(selectedWord.roman, wroteRomanization = true)
                } else {
                    resolveTaigiCommit(selectedWord, isTPSLayout, effectiveSwapped, cachedOutputBothScripts)
                }
            val textToCommit = resolved.text

            if (logger.isDebugEnabled) {
                logger.d(
                    TAG,
                    "[CLICK] id=${selectedWord.id}, roman='${selectedWord.roman}', hanzi='${selectedWord.hanzi}'",
                )
                logger.d(
                    TAG,
                    "[CLICK] isTranslateSwapped=$cachedIsTranslateSwapped, effectiveSwapped=$effectiveSwapped, outputBothScripts=$cachedOutputBothScripts",
                )
                logger.d(
                    TAG,
                    "[CLICK] textToCommit='$textToCommit', isNextWord=$isNextWordPrediction, isEnglish=$isEnglishSuggestion",
                )
            }

            if (isEnglishSuggestion) {
                val textBeforeCursor = ic.getTextBeforeCursor(100, 0)?.toString() ?: ""
                val currentWord = NextWordController.extractCurrentWord(textBeforeCursor)
                if (currentWord.isNotEmpty()) {
                    ic.deleteSurroundingText(currentWord.length, 0)
                }
                ic.commitText(textToCommit, 1)
                onClearCandidates()
                logger.debug(TAG) { "[ENGLISH-CLICK] Replaced '$currentWord' with '$textToCommit'" }
            } else if (isNextWordPrediction) {
                logger.debug(TAG) { "[NEXTWORD-CLICK] BEFORE commitText: text='$textToCommit', ic=$ic" }
                val result = ic.commitText(textToCommit, 1)
                composingManager?.reset(ic)
                logger.debug(TAG) { "[NEXTWORD-CLICK] AFTER commitText: result=$result" }
            } else {
                composingManager?.selectSuggestion(textToCommit, ic)
            }

            appendAutoSpaceIfEarned(taigikeyboard, ic, textToCommit, resolved.wroteRomanization)

            // R5 pair-key (#7): the candidate's canonical-TL reading from the
            // metadata sidechannel keeps multi-reading Hanji in separate buckets; "" only
            // on wire skew / TPS-OOV / English rows.
            val canonicalTl = selectedWord.additionalInfo[TaigiWord.MetadataKeys.CANONICAL_TL] ?: ""
            usage.record(Usage(selectedWord.displayText, canonicalTl))

            // NextWord learns the canonical reading, not the rendered `roman`
            // (No Hyphens strips its hyphens, §49) — mirrors iOS
            // ActionHandler+Suggestions.swift `associationRoman` (`additionalInfo["tl"]`).
            onNextWordPrediction(selectedWord.displayText, canonicalTl.ifEmpty { selectedWord.roman })
        }
    }

    /**
     * Document text + auto-space verdict for one NextWord prediction tap,
     * strip or expanded overlay (a Continuous tap is resolved by the engine).
     * A §42 Hanji with Romanization [TaigiWord.MetadataKeys.CELL_SCRIPT]-marked cell commits
     * the script its marker names ([resolveMarkedCellCommit]); an unmarked
     * cell follows the mode ([resolveUnmarkedCommit]), with the Annotate in Brackets roman
     * TPS-rendered under the TPS layout.
     */
    private fun resolveTaigiCommit(
        word: TaigiWord,
        isTPSLayout: Boolean,
        effectiveSwapped: Boolean,
        outputBothScripts: Boolean,
    ): ResolvedCommit {
        word.additionalInfo[TaigiWord.MetadataKeys.CELL_SCRIPT]
            ?.let { cellScript ->
                resolveMarkedCellCommit(
                    cellScript = cellScript,
                    roman = word.roman,
                    hanzi = word.hanzi,
                    outputBothScripts = outputBothScripts,
                )
            }?.let { return it }
        val bracketRoman =
            if (isTPSLayout) RustEngineBridge.tlDisplayToTps(word.roman, prefs.tpsOrMapsToER) else word.roman
        return resolveUnmarkedCommit(
            roman = word.roman,
            bracketRoman = bracketRoman,
            hanzi = word.hanzi,
            effectiveSwapped = effectiveSwapped,
            outputBothScripts = outputBothScripts,
        )
    }

    /**
     * Handle English candidate click (3-column layout).
     */
    fun handleEnglishCandidateClick(index: Int) {
        TraceContext.withTrace(TraceId.next()) {
            logger.debug(TAG) { "[INPUT] fn=handleEnglishCandidateClick gesture=candidate-tap" }

            val currentSuggestions = getCurrentSuggestions()
            if (index >= currentSuggestions.size) return

            val selectedWord = currentSuggestions[index]
            val ic = taigikeyboard.currentInputConnection ?: return

            val textBeforeCursor = ic.getTextBeforeCursor(100, 0)?.toString() ?: ""
            val currentWord = NextWordController.extractCurrentWord(textBeforeCursor)

            if (currentWord.isNotEmpty()) {
                ic.deleteSurroundingText(currentWord.length, 0)
            }
            ic.commitText(selectedWord.roman, 1)
            onClearCandidates()

            logger.debug(TAG) { "[ENGLISH-CLICK] Replaced '$currentWord' with '${selectedWord.roman}'" }
        }
    }

    /**
     * Handle overlay suggestion selection.
     */
    fun handleOverlaySuggestionSelected(
        word: TaigiWord,
        index: Int,
    ) {
        taigikeyboard.beginInputEvent()
        val ic = taigikeyboard.currentInputConnection ?: return
        val composingManager = getComposingManager()

        // Continuous-input branch routes BEFORE the sentinel-id branches.
        // Overlay taps on Continuous candidates must go through commitContinuous
        // with the consumedBytes / syllableCount sidechannel; the default
        // selectSuggestion path would commit displayText only and mis-align
        // the engine pending buffer.
        if (word.additionalInfo[TaigiWord.MetadataKeys.IS_CONTINUOUS] == "true" && composingManager != null) {
            handleContinuousCandidateClick(word, ic, composingManager)
            return
        }

        val isNextWordPred = word.id < 0
        val cachedIsTranslateSwapped = getIsTranslateSwapped()
        val cachedOutputBothScripts = getOutputBothScripts()
        val isTPSLayout = prefs.isTpsLayout
        val effectiveSwapped = isTPSLayout || cachedIsTranslateSwapped

        val resolved = resolveTaigiCommit(word, isTPSLayout, effectiveSwapped, cachedOutputBothScripts)
        val textToCommit = resolved.text

        if (isNextWordPred) {
            ic.commitText(textToCommit, 1)
            composingManager?.reset(ic)
            logger.debug(TAG) { "[OVERLAY] NextWord commitText: '$textToCommit'" }
        } else {
            composingManager?.selectSuggestion(textToCommit, ic)
        }

        appendAutoSpaceIfEarned(taigikeyboard, ic, textToCommit, resolved.wroteRomanization)

        // R5 pair-key (#7): canonical-TL reading from the metadata
        // sidechannel; "" only on wire skew / TPS-OOV / English rows.
        val canonicalTl = word.additionalInfo[TaigiWord.MetadataKeys.CANONICAL_TL] ?: ""
        usage.record(Usage(word.displayText, canonicalTl))

        // NextWord learns the canonical reading (§49) — see the strip path above.
        onNextWordPrediction(word.displayText, canonicalTl.ifEmpty { word.roman })

        logger.debug(TAG) { "[OVERLAY] Selected suggestion: ${word.displayText} at index $index" }
    }

    /**
     * Continuous-input candidate tap (slot 0 and slot N, identical contract),
     * strip and expanded overlay alike.
     *
     * R5: the engine resolves what the pick writes from the candidate's own
     * scripts under the live settings, counts the pick, and answers what it
     * did — so Continuous commits match iOS / macOS / Windows / Linux by
     * construction. The pick is read off the word's metadata ([continuousPick]);
     * a §42 split cell commits the script its marker names ([commitScript]).
     *
     * `DISPLAY_TEXT`, `CONSUMED_BYTES`, and `SYLLABLE_COUNT` are
     * strict-required (Item 4 fork F2=A); missing or unparseable → drop the
     * tap. No fallback to `selectSuggestion(text)` — would lose
     * `consumedBytes` and corrupt `Phase::Continuous { raw }` byte alignment.
     */
    private fun handleContinuousCandidateClick(
        selectedWord: TaigiWord,
        ic: android.view.inputmethod.InputConnection,
        composingManager: com.siansiansu.taigikeyboard.ime.text.composing.ComposingManager,
    ) {
        val pick = continuousPick(selectedWord)
        if (pick == null) {
            val info = selectedWord.additionalInfo
            logger.w(
                TAG,
                "[CONTINUOUS] decode failed displayText=${info[TaigiWord.MetadataKeys.DISPLAY_TEXT]} consumedBytes=${info[TaigiWord.MetadataKeys.CONSUMED_BYTES]} syllableCount=${info[TaigiWord.MetadataKeys.SYLLABLE_COUNT]}",
            )
            return
        }
        val outcome = composingManager.commitContinuous(pick, ic)
        logger.debug(TAG) { "[CONTINUOUS] commit script=${pick.script} outcome=$outcome" }

        when (outcome) {
            // Mid-commit: engine stays in Continuous with a fresh pending span,
            // but `PerformAutocomplete` is a delegate no-op so the strip would
            // keep stale `consumedBytes` metadata until the next keypress.
            // Trigger the standard refresh. Final-commit deliberately skipped:
            // it emits NextWordWordSelected which drives async NextWord
            // predict; a Taigi refresh would see `rawInput=null` and call
            // `clearCandidates()`, racing with / wiping the fresh predictions.
            RustEngineBridge.ContinuousCommitOutcome.Nailed -> onRequestCandidateRefresh()
            // Auto-space only on the final commit, as the engine judged what
            // the pick wrote (romanization, no trailing `-`, §23).
            is RustEngineBridge.ContinuousCommitOutcome.Finalized ->
                appendAutoSpace(taigikeyboard, ic, taigikeyboard.prefs.isAutoSpaceEnabled && outcome.earnsAutoSpace)
            // A stale tap (the engine had silently reset to Idle) or a
            // rejected pick wrote nothing.
            RustEngineBridge.ContinuousCommitOutcome.Ignored -> Unit
        }
    }

    companion object {
        private const val TAG = "CandidateClickHandler"
    }
}

/**
 * Annotate in Brackets / both-scripts commit form when Hanji leads — the romanization
 * rides in trailing brackets. Single spelling for the four hanji-first
 * commit sites; the roman-first inverse (`roman (hanzi)`) stays inline.
 */
private fun bracketedCommit(
    hanzi: String,
    roman: String,
): String = "$hanzi ($roman)"

// CROSS-PLATFORM INVARIANT — mirrors ios/Sources/TaigiKeyboard/Actions/ActionHandler+Suggestions.swift
// `formatOutputText` and engine/composing/src/commit_text.rs `resolve_commit_text`. Drift causes
// silent divergence (a missing or stray auto-space after a swapped-mode commit).

/**
 * Document text + auto-space verdict for an UNMARKED commit — the three
 * sites that build the string from the candidate's own `(roman, hanzi)`
 * pair rather than from a §42 cell marker.
 *
 * The verdict is resolved by the SAME arm that picks the string, never from
 * the output mode afterwards. Auto-space is a property of ROMANIZATION
 * (`guá beh khì` needs the gaps, 我欲去 does not), and a candidate with no
 * Hanji — the literal-romanization candidate (§34), an out-of-vocabulary name, a
 * romanization-only custom entry — falls to the last arm and writes its
 * romanization whatever the mode leads with.
 *
 * `bracketRoman` is the Annotate in Brackets rendering of `roman` (TPS-converted in a
 * TPS layout); the bare `roman` is what a roman-led commit writes.
 */
internal fun resolveUnmarkedCommit(
    roman: String,
    bracketRoman: String,
    hanzi: String?,
    effectiveSwapped: Boolean,
    outputBothScripts: Boolean,
): ResolvedCommit =
    when {
        // Annotate in Brackets writes the pair, so the romanization IS in the document
        // whichever half leads.
        outputBothScripts && !hanzi.isNullOrEmpty() ->
            ResolvedCommit(
                text =
                    if (effectiveSwapped) {
                        bracketedCommit(hanzi, bracketRoman)
                    } else {
                        "$bracketRoman ($hanzi)"
                    },
                wroteRomanization = true,
            )

        effectiveSwapped && !hanzi.isNullOrEmpty() ->
            ResolvedCommit(text = hanzi, wroteRomanization = false)

        else -> ResolvedCommit(text = roman, wroteRomanization = true)
    }

/**
 * Insert a single trailing space when auto-space is enabled, the commit wrote
 * romanization, and the committed text does not already end in a hyphen
 * continuation — and arm the punctuation swap on it.
 *
 * The ONE place a space and the knowledge that it is ours are set together, so
 * they can never come apart: a commit that earns nothing arms nothing, and the
 * event already consumed the previous arm ([TaigiKeyboard.beginInputEvent]).
 * Callers resolve [wroteRomanization] from the arm that picked the document
 * string — [resolveUnmarkedCommit], [resolveMarkedCellCommit], or
 * [rawPreeditWritesRomanization] — never from the output mode.
 */
internal fun appendAutoSpaceIfEarned(
    taigikeyboard: TaigiKeyboard,
    ic: android.view.inputmethod.InputConnection,
    committedText: String,
    wroteRomanization: Boolean,
) = appendAutoSpace(
    taigikeyboard,
    ic,
    shouldAppendAutoSpace(taigikeyboard.prefs.isAutoSpaceEnabled, wroteRomanization, committedText),
)

/**
 * The one insertion site of the auto space: [isEarned] is the whole verdict,
 * the live Auto-Space setting included ([appendAutoSpaceIfEarned], or the
 * engine's `earns_auto_space` ANDed with it on a Continuous commit).
 */
private fun appendAutoSpace(
    taigikeyboard: TaigiKeyboard,
    ic: android.view.inputmethod.InputConnection,
    isEarned: Boolean,
) {
    if (!isEarned) return
    ic.commitText(" ", 1)
    taigikeyboard.armAutoSpaceSwap()
}

/**
 * The engine request for a Continuous candidate tap, read off the word's
 * metadata (`TaigiAutocompleteService.kt` `continuousSidechannels`) and its own
 * `roman` / `hanzi` (the candidate's, never rewritten on Android). `null` when
 * a strict key is missing. Mirrors iOS `ActionHandler.continuousPick(for:)`.
 */
internal fun continuousPick(word: TaigiWord): RustEngineBridge.ContinuousPick? {
    val info = word.additionalInfo
    val canonicalText = info[TaigiWord.MetadataKeys.DISPLAY_TEXT] ?: return null
    val consumedBytes = info[TaigiWord.MetadataKeys.CONSUMED_BYTES]?.toIntOrNull() ?: return null
    val syllableCount = info[TaigiWord.MetadataKeys.SYLLABLE_COUNT]?.toIntOrNull() ?: return null
    return RustEngineBridge.ContinuousPick(
        script = commitScript(word),
        roman = word.roman,
        canonicalText = canonicalText,
        // Absent only on wire skew → "" → the engine falls back to the raw
        // committed slice for NextWord.
        associationTl = info[TaigiWord.MetadataKeys.CANONICAL_TL] ?: "",
        // §50 — the pick's hanji (identity, not the committed script): the
        // engine learns a composition only when every segment carried one.
        hanji = word.hanzi?.takeIf { it.isNotEmpty() },
        consumedBytes = consumedBytes,
        syllableCount = syllableCount,
    )
}

// CROSS-PLATFORM INVARIANT — mirrors ios ActionHandler+Suggestions.swift `commitScript(for:)`.

/**
 * Which script a Continuous tap commits: a §42 split cell the one its
 * [TaigiWord.MetadataKeys.CELL_SCRIPT] marker names, every other cell what the
 * output settings lead with. An unknown marker commits the lead; a marker
 * whose script the pick lacks the engine resolves to the lead too.
 */
internal fun commitScript(word: TaigiWord): CommitScript =
    when (word.additionalInfo[TaigiWord.MetadataKeys.CELL_SCRIPT]) {
        TaigiWord.MetadataKeys.CELL_SCRIPT_HANJI -> CommitScript.COMMIT_SCRIPT_HANJI
        TaigiWord.MetadataKeys.CELL_SCRIPT_ROMAN -> CommitScript.COMMIT_SCRIPT_ROMAN
        else -> CommitScript.COMMIT_SCRIPT_LEAD
    }

// CROSS-PLATFORM INVARIANT — one name on all four platforms: ios
// `ActionHandler.rawPreeditWritesRomanization`, macOS/Windows
// `AutoSpacePolicy.rawPreeditWritesRomanization(inputMode:)` /
// `policies::raw_preedit_writes_romanization`.

/**
 * Whether the layout in use composes romanization — TL and POJ do, TPS
 * composes Bopomofo, which takes no word spacing. The verdict for every commit
 * that writes the composition AS TYPED (Enter on the raw input), which does
 * not go through a candidate's rendering.
 */
internal fun rawPreeditWritesRomanization(isTPSLayout: Boolean): Boolean = !isTPSLayout

// CROSS-PLATFORM INVARIANT — mirrors ios/Sources/TaigiKeyboard/Actions/ActionHandler+Suggestions.swift
// `shouldAppendAutoSpace`. Drift causes silent divergence (one platform spacing after a
// hyphen continuation).

/**
 * Whether to insert the trailing auto-space: the setting is on, the commit
 * wrote romanization, and the committed DOCUMENT string does not end in a
 * hyphen continuation (a mid-word hyphen keeps composing). The Continuous
 * caller still owns the final-commit gate.
 */
internal fun shouldAppendAutoSpace(
    isAutoSpaceEnabled: Boolean,
    wroteRomanization: Boolean,
    committedText: String,
): Boolean = isAutoSpaceEnabled && wroteRomanization && !committedText.endsWith("-")

// CROSS-PLATFORM INVARIANT — mirrors ios/Sources/TaigiKeyboard/Keyboard/ActionHandler+Suggestions.swift marked-cell commit resolve
// and the desktop `.alternate` commit rule. Drift causes silent divergence (a bracketed roman-cell commit, or a missing auto-space).

/**
 * Document text + auto-space verdict for a Hanji with Romanization
 * [TaigiWord.MetadataKeys.CELL_SCRIPT]-marked cell (§42 second exception).
 * Hanji cell → the Hanji, or `Hanji (romanization)` when Annotate in Brackets is on (only then
 * did the commit write romanization); roman cell → the BARE roman, brackets
 * IGNORED (desktop `.alternate` parity), always romanization. Returns
 * `null` for an unknown marker or a marker whose payload is missing (wire
 * defect) — the caller falls back to the unmarked mode-derived path.
 * Top-level pure function so the contract is unit-testable without
 * collaborators.
 */
internal fun resolveMarkedCellCommit(
    cellScript: String,
    roman: String,
    hanzi: String?,
    outputBothScripts: Boolean,
): ResolvedCommit? =
    when {
        cellScript == TaigiWord.MetadataKeys.CELL_SCRIPT_ROMAN && roman.isNotEmpty() ->
            ResolvedCommit(text = roman, wroteRomanization = true)

        cellScript == TaigiWord.MetadataKeys.CELL_SCRIPT_HANJI && !hanzi.isNullOrEmpty() ->
            if (outputBothScripts && roman.isNotEmpty()) {
                ResolvedCommit(text = bracketedCommit(hanzi, roman), wroteRomanization = true)
            } else {
                // No roman to bracket → the bare Hanji, never empty brackets.
                ResolvedCommit(text = hanzi, wroteRomanization = false)
            }

        else -> null
    }

// CROSS-PLATFORM INVARIANT — mirrors ios `ActionHandler.ResolvedCommit` and the
// engine's `engine/composing/src/commit_text.rs` `ResolvedCommit` (macOS + desktop, R5).

/**
 * What one commit writes into the document, and whether that string carries
 * romanization — the single input the auto-space gate reads.
 */
internal data class ResolvedCommit(
    val text: String,
    val wroteRomanization: Boolean,
)
