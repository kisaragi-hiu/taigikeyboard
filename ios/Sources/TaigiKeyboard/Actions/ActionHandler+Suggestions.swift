// ActionHandler extension: suggestion selection (candidate commit, output formatting).

import Foundation
import KeyboardKit

/// What one NextWord prediction commit writes into the document, and whether
/// that string carries romanization — the single input the auto-space gate
/// reads. A Continuous commit is resolved by the engine.
// CROSS-PLATFORM INVARIANT — mirrors android `ResolvedCommit` and the engine's
// `engine/composing/src/commit_text.rs` `ResolvedCommit` (every Continuous commit, R5).
struct ResolvedCommit {
    let text: String
    let wroteRomanization: Bool
}

extension ActionHandler {
    // MARK: - Suggestion Selection

    func handleSuggestionSelection(_ suggestion: AutocompleteSuggestion) {
        // Raw input candidate: commit literal keystrokes directly (no tone conversion)
        if suggestion.additionalInfo["isRawInput"] == "true" {
            composingManager.commitComposition()
            // The literal keystrokes go into the document, so the verdict is
            // what the layout composes — romanization in TL/POJ, Bopomofo in
            // TPS, which takes no spacing. The output mode does not enter into
            // it: a raw commit writes the same string whichever script the
            // candidates would have led with.
            appendAutoSpaceIfEarned(
                documentText: suggestion.text,
                wroteRomanization: Self.rawPreeditWritesRomanization(isTPSLayout: isTPSLayout),
            )
            return
        }

        // Continuous candidate (R5): the engine resolves what the pick writes
        // from the candidate's own scripts under the live settings, counts the
        // pick, and answers the auto-space verdict — so Continuous commits match
        // macOS / Windows / Linux / Android by construction. The pick is read
        // off the suggestion's metadata, never its view-rewritten `text`.
        if suggestion.additionalInfo["isContinuous"] == "true" {
            // Strict-required metadata (Item 4 fork F2=A): missing → drop the
            // tap. Falling back to `selectCandidate(text:)` would lose
            // `consumedBytes` and corrupt `Phase::Continuous { raw }` alignment.
            guard let pick = Self.continuousPick(for: suggestion) else {
                logger.debug(
                    "[SELECT] continuous metadata decode failed; dropping tap. "
                        + "additionalInfo=\(suggestion.additionalInfo.description)",
                )
                return
            }
            // Auto-space only on the FINAL commit, as the engine judged what the
            // pick wrote (romanization, no trailing `-`, §23); a nail keeps
            // composing and a stale / rejected pick wrote nothing.
            if case let .finalized(earnsAutoSpace) = composingManager.commitContinuous(pick) {
                appendAutoSpace(ifEarned: settings.isAutoSpaceEnabled && earnsAutoSpace)
            }
            return
        }

        let isNextWordPrediction = suggestion.additionalInfo["isNextWord"] == "true"

        if composingManager.isComposing || isNextWordPrediction {
            let effectiveSwapped = isTPSLayout || settings.isHanjiFirst

            // §42 Hanji with Romanization split prediction cells carry a `cellScript` marker
            // (`ActionHandler.predictionSuggestions`) — the marker decides the
            // script, exactly as on the Continuous path; identity rides the
            // shared sidechannels, so nothing is parsed back from the cell.
            let roman: String
            let hanji: String?
            let resolved: ResolvedCommit
            if let cellScript = CandidateCellScript.marker(for: suggestion) {
                roman = suggestion.additionalInfo["tl"] ?? ""
                hanji = suggestion.additionalInfo["hanzi"]
                resolved = Self.markedCellCommit(
                    cellScript: cellScript,
                    cellText: suggestion.text,
                    roman: suggestion.additionalInfo[CandidateCellScript.bracketRomanKey],
                    isOutputBothScripts: settings.isOutputBothScripts,
                )
            } else {
                (roman, hanji) = Self.parseRomanAndHanji(
                    from: suggestion,
                    isNextWord: isNextWordPrediction,
                    isTPSLayout: isTPSLayout,
                    effectiveSwapped: effectiveSwapped,
                )
                resolved = Self.formatOutputText(
                    roman: roman,
                    hanji: hanji,
                    isTPSLayout: isTPSLayout,
                    effectiveSwapped: effectiveSwapped,
                    isOutputBothScripts: settings.isOutputBothScripts,
                    orMapsToER: settings.isTpsOrMappedToER,
                )
            }
            let textToCommit = resolved.text

            commitSuggestionText(textToCommit, isNextWord: isNextWordPrediction, suggestion: suggestion)

            let displayText = suggestion.additionalInfo["displayText"] ?? hanji ?? roman
            // R5 pair-key (#7): canonical-TL reading from the same
            // sidechannel; empty (legacy bucket) for a NextWord prediction
            // that carries no canonical TL.
            let canonicalTl = suggestion.additionalInfo["canonicalTl"] ?? ""
            CompositionRoot.usageRecorder.record(Usage(displayText: displayText, canonicalTl: canonicalTl))

            logger.debug("[SELECT] suggestion.text='\(suggestion.text)' subtitle='\(suggestion.subtitle ?? "nil")' additionalInfo=\(suggestion.additionalInfo.description)")
            logger.debug("[SELECT] parsed roman='\(roman)' hanji='\(hanji ?? "nil")' displayText='\(displayText)'")

            // Romanization mode: auto-space (unless trailing hyphen).
            // TPS mode disables auto-space (effectiveSwapped is true for TPS).
            appendAutoSpaceIfEarned(
                documentText: textToCommit,
                wroteRomanization: resolved.wroteRomanization,
            )

            // Fork: `roman` is the commit string (may be POJ/Hanji); the engine
            // expects raw TL (it calls `pojToTL` on it). Next-word candidates
            // carry raw TL on the `additionalInfo["tl"]` sidechannel — use it
            // here to preserve association-recording semantics.
            let associationRoman = isNextWordPrediction
                ? (suggestion.additionalInfo["tl"] ?? "")
                : roman
            nextWordController.process(text: displayText, roman: associationRoman)
        } else {
            keyboardContext.textDocumentProxy.insertText(suggestion.text)
        }
    }

    // MARK: - Suggestion Helpers

    /// The engine request for a Continuous candidate tap, read off the
    /// suggestion's metadata alone (`TaigiAutocompleteService.continuousSidechannels`)
    /// — the view may have rewritten `text` (`CandidateCellHelper.suggestionToHandle`:
    /// the swap, the TPS rendering). `nil` when a strict key is missing.
    static func continuousPick(for suggestion: AutocompleteSuggestion) -> RustEngineBridge.ContinuousPick? {
        let info = suggestion.additionalInfo
        guard let canonicalText = info["displayText"],
              let roman = info["roman"],
              let consumedBytes = info["consumedBytes"].flatMap(UInt32.init),
              let syllableCount = info["syllableCount"].flatMap(UInt32.init)
        else {
            return nil
        }
        return RustEngineBridge.ContinuousPick(
            script: commitScript(for: suggestion),
            roman: roman,
            canonicalText: canonicalText,
            // Absent only on wire skew → "" → the engine falls back to the raw
            // committed slice for NextWord.
            associationTl: info["canonicalTl"] ?? "",
            hanji: info["hanji"],
            consumedBytes: consumedBytes,
            syllableCount: syllableCount,
        )
    }

    /// Which script a Continuous tap commits: a §42 split cell the one its
    /// marker names, every other cell what the output settings lead with. A
    /// defective marker is declined by `CandidateCellScript.marker`, so the
    /// cell commits the lead, as the render guard shows it.
    // CROSS-PLATFORM INVARIANT — mirrors android CandidateClickHandler.kt `commitScript`.
    static func commitScript(for suggestion: AutocompleteSuggestion) -> Taigi_Engine_CommitScript {
        switch CandidateCellScript.marker(for: suggestion) {
        case CandidateCellScript.hanji: .hanji
        case CandidateCellScript.roman: .roman
        default: .lead
        }
    }

    /// §42 Hanji with Romanization marked-cell document text for a NextWord
    /// prediction tap (a Continuous tap is resolved by the engine).
    ///
    /// A split cell's `cellScript` marker is authoritative, so the document
    /// string resolves directly from the marker + info fields — never through
    /// `parseRomanAndHanji` (whose contract is "derive from the UI-shaped
    /// suggestion") and never through a new `formatOutputText` arm:
    /// - `"hanji"` cell commits the hanji; Annotate in Brackets ON appends the roman
    ///   sidechannel as `Hanji (romanization)` — today's swapped output. TPS never
    ///   applies (Hanji with Romanization is TL/POJ only), so the bracket roman is never TPS-rendered.
    /// - `"roman"` cell commits the BARE roman; Annotate in Brackets is ignored (desktop
    ///   `.alternate` parity).
    ///
    /// Precondition: `cellScript` came from `CandidateCellScript.marker(for:)`,
    /// so it is a marker this build knows and `cellText` is non-empty — a wire
    /// defect is declined there, by the render guard and this caller together.
    /// A hanji cell whose roman sidechannel is missing commits the bare hanji
    /// rather than empty brackets.
    ///
    /// Returns the document string plus whether the commit wrote romanization
    /// (the bracket form DID write it, so the hanji arm's verdict is
    /// `isOutputBothScripts`). Static with the settings flag injected so tests
    /// pin it without a keyboard context.
    static func markedCellCommit(
        cellScript: String,
        cellText: String,
        roman: String?,
        isOutputBothScripts: Bool,
    ) -> ResolvedCommit {
        guard cellScript == CandidateCellScript.hanji else {
            return ResolvedCommit(text: cellText, wroteRomanization: true)
        }
        guard isOutputBothScripts, let roman, !roman.isEmpty else {
            return ResolvedCommit(text: cellText, wroteRomanization: false)
        }
        return ResolvedCommit(
            text: Self.bracketedHanjiCommit(hanji: cellText, roman: roman),
            wroteRomanization: true,
        )
    }

    /// Whether to insert the trailing auto-space: the setting is on, the commit
    /// wrote romanization, and the committed DOCUMENT string does not end in a
    /// hyphen continuation (a mid-word hyphen keeps composing). The caller still
    /// owns the final-commit gate on the Continuous path.
    // CROSS-PLATFORM INVARIANT — mirrors android/.../smartbar/CandidateClickHandler.kt
    // `shouldAppendAutoSpace`. Drift causes silent divergence (one platform spacing
    // after a hyphen continuation).
    static func shouldAppendAutoSpace(
        isAutoSpaceEnabled: Bool,
        wroteRomanization: Bool,
        documentText: String,
    ) -> Bool {
        isAutoSpaceEnabled && wroteRomanization && !documentText.hasSuffix("-")
    }

    /// The Annotate in Brackets hanji-led output shape `Hanji (romanization)` — single spelling
    /// shared by `formatOutputText`'s swapped arm and the §42 marked hanji cell.
    static func bracketedHanjiCommit(hanji: String, roman: String) -> String {
        "\(hanji) (\(roman))"
    }

    /// Extract romanization and Hanji from suggestion based on display mode. The NextWord path
    /// restores the fields that were swapped earlier.
    static func parseRomanAndHanji(
        from suggestion: AutocompleteSuggestion,
        isNextWord: Bool,
        isTPSLayout: Bool,
        effectiveSwapped: Bool,
    ) -> (roman: String, hanji: String?) {
        if isNextWord {
            // CROSS-PLATFORM INVARIANT: next-word commit string == UI display string.
            // `suggestion.text` is mode-shaped (POJ in POJ mode, TL otherwise) by
            // `RustEngineBridge.nextwordPredictNext` (Rust shape rule), but `CandidateCellHelper.suggestionToHandle`
            // pre-swaps text↔subtitle in swapped/TPS modes before this handler runs —
            // so we must mirror that swap to recover the mode-shaped roman.
            // `additionalInfo["hanzi"]` carries hanji even for hanji-only predictions
            // (Case B) where `subtitle == nil`. The raw-TL sidechannel on
            // `additionalInfo["tl"]` is consumed separately at the association call
            // site (see `handleSuggestionSelection`).
            // Mirror: android/.../smartbar/NextWordController.kt:355-363 (TaigiWord.roman).
            // Swapped/TPS Case B (hanji-only, no roman): `subtitle == nil` after
            // `suggestionToHandle` (swap gate requires non-empty subtitle). Fall
            // back to `""` so bracket-mode output stays `"Hanji ()"` — matches the
            // pre-fix sidechannel behavior, avoids Hanji duplication.
            let roman = effectiveSwapped
                ? (suggestion.subtitle ?? "")
                : suggestion.text
            return (roman, suggestion.additionalInfo["hanzi"])
        } else if effectiveSwapped {
            // The swap rewrite (`suggestionToHandle`) only swaps a cell that has a
            // hanji subtitle, so a hanji-less cell arrives with its romanization as
            // the text and no hanji at all (§23 / §34; Android
            // `noHanji_commitsTheRomanizationUnderEveryMode`). TPS keeps the text as
            // hanji: the Bopomofo it commits takes no word spacing.
            if !isTPSLayout, suggestion.subtitle?.isEmpty ?? true {
                return (suggestion.text, nil)
            }
            return (suggestion.subtitle ?? suggestion.text, suggestion.text)
        } else {
            return (suggestion.text, suggestion.subtitle)
        }
    }

    /// Format output text based on display mode (roman, Hanji, or both scripts); a TPS layout
    /// uses the TPS bracket rendering.
    /// The document string AND whether writing it puts romanization in the
    /// document — resolved together, by the one branch that picks the string.
    ///
    /// Auto-space is a property of ROMANIZATION (`guá beh khì` needs the gaps,
    /// 我欲去 does not), so its gate has to answer for the string this commit
    /// actually writes. Deriving the verdict from the output mode instead is
    /// only ever an approximation, and it is wrong for a candidate with no
    /// Hanji: the literal romanization candidate (§34), an out-of-vocabulary name, a
    /// romanization-only custom entry all fall to the last arm and write
    /// romanization whatever the mode leads with.
    // CROSS-PLATFORM INVARIANT — mirrors android/.../CandidateClickHandler.kt
    // `resolveUnmarkedCommit` (prediction taps) and engine/composing/src/commit_text.rs
    // `resolve_commit_text` (Continuous taps). Drift changes which commits earn a space.
    static func formatOutputText(
        roman: String,
        hanji: String?,
        isTPSLayout: Bool,
        effectiveSwapped: Bool,
        isOutputBothScripts: Bool,
        orMapsToER: Bool,
    ) -> ResolvedCommit {
        let bracketRoman = isTPSLayout
            ? RustEngineBridge.tlDisplayToTPS(roman, orMapsToER: orMapsToER)
            : roman

        // Annotate in Brackets writes the pair, so the romanization IS in the document
        // whichever half leads.
        if isOutputBothScripts, let hanji, !hanji.isEmpty {
            let text = effectiveSwapped
                ? Self.bracketedHanjiCommit(hanji: hanji, roman: bracketRoman)
                : "\(bracketRoman) (\(hanji))"
            return ResolvedCommit(text: text, wroteRomanization: true)
        } else if effectiveSwapped, let hanji, !hanji.isEmpty {
            return ResolvedCommit(text: hanji, wroteRomanization: false)
        } else {
            return ResolvedCommit(text: roman, wroteRomanization: true)
        }
    }

    /// Inserts the trailing auto space when this commit earned it, and arms
    /// the punctuation swap on it — the ONE insertion site, so the space and
    /// the knowledge that it is ours can never come apart.
    ///
    /// A commit that earns nothing arms nothing: the event already consumed
    /// the previous arm (`beginInputEvent`), so the next punctuation key sees
    /// no space of ours to swap.
    func appendAutoSpaceIfEarned(documentText: String, wroteRomanization: Bool) {
        appendAutoSpace(ifEarned: Self.shouldAppendAutoSpace(
            isAutoSpaceEnabled: settings.isAutoSpaceEnabled,
            wroteRomanization: wroteRomanization,
            documentText: documentText,
        ))
    }

    private func appendAutoSpace(ifEarned isEarned: Bool) {
        guard isEarned else { return }
        keyboardContext.textDocumentProxy.insertText(" ")
        armAutoSpaceSwap()
    }

    /// Whether the layout in use composes romanization — TL and POJ do, TPS
    /// composes Bopomofo, which takes no word spacing. The verdict for every
    /// commit that writes the composition AS TYPED (the raw-input candidate,
    /// Return while composing), none of which goes through a candidate's
    /// rendering.
    // CROSS-PLATFORM INVARIANT — one name on all five platforms: android
    // `rawPreeditWritesRomanization`, desktop-core (macOS / Windows / Linux)
    // `policies::raw_preedit_writes_romanization`.
    static func rawPreeditWritesRomanization(isTPSLayout: Bool) -> Bool {
        !isTPSLayout
    }

    /// The layout composes Bopomofo rather than romanization. Named once —
    /// four call sites in this file alone read it (Android: `prefs.isTpsLayout`).
    var isTPSLayout: Bool {
        settings.keyboardLayoutType == .tps
    }

    /// Commit text via proxy (NextWord) or composing manager (regular candidate).
    /// The `suggestion` parameter is kept for future telemetry/logging use
    /// but ComposingManager only needs the candidate text.
    private func commitSuggestionText(_ text: String, isNextWord: Bool, suggestion _: AutocompleteSuggestion) {
        if isNextWord {
            keyboardContext.textDocumentProxy.insertText(text)
        } else {
            composingManager.selectCandidate(text: text)
        }
    }
}
