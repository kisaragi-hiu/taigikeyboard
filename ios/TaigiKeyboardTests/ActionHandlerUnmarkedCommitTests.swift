// Pins the unmarked (un-split) commit resolver end to end from the cell the view hands the
// handler: `CandidateCellHelper.suggestionToHandle` → `parseRomanAndHanzi` → `formatOutputText`.
// Mirrors Android `UnmarkedCommitResolverTest` (behavioral-invariants §23 / §34).

import KeyboardKit
@testable import TaigiKeyboard
import XCTest

final class ActionHandlerUnmarkedCommitTests: XCTestCase {
    /// A continuous candidate as `TaigiAutocompleteService.dualScriptSuggestion` emits it:
    /// romanization as the text, the hanji (if any) as the subtitle.
    private func candidate(roman: String, hanji: String?) -> AutocompleteSuggestion {
        AutocompleteSuggestion(text: roman, title: roman, subtitle: hanji, additionalInfo: ["isContinuous": "true"])
    }

    private func commit(
        _ suggestion: AutocompleteSuggestion,
        isTranslateSwapped: Bool,
        isTPSLayout: Bool = false,
        isOutputBothScripts: Bool,
    ) -> ResolvedCommit {
        let handed = CandidateCellHelper.suggestionToHandle(
            for: suggestion,
            isTranslateSwapped: isTranslateSwapped,
            isTPSLayout: isTPSLayout,
            orMapsToER: true,
        )
        let effectiveSwapped = isTPSLayout || isTranslateSwapped
        let (roman, hanzi) = ActionHandler.parseRomanAndHanzi(
            from: handed,
            isNextWord: false,
            isTPSLayout: isTPSLayout,
            effectiveSwapped: effectiveSwapped,
        )
        return ActionHandler.formatOutputText(
            roman: roman,
            hanzi: hanzi,
            isTPSLayout: isTPSLayout,
            effectiveSwapped: effectiveSwapped,
            isOutputBothScripts: isOutputBothScripts,
            orMapsToER: true,
        )
    }

    /// A candidate with no hanji (the §34 literal, an out-of-vocabulary name) writes its
    /// romanization and earns the space under every TL/POJ mode — Hanji-first included.
    /// Mirrors Android `noHanji_commitsTheRomanizationUnderEveryMode`.
    func testNoHanji_commitsTheRomanizationUnderEveryMode() {
        for isTranslateSwapped in [false, true] {
            for isOutputBothScripts in [false, true] {
                let resolved = commit(
                    candidate(roman: "taigi", hanji: nil),
                    isTranslateSwapped: isTranslateSwapped,
                    isOutputBothScripts: isOutputBothScripts,
                )
                let mode = "swapped=\(isTranslateSwapped) both=\(isOutputBothScripts)"
                XCTAssertEqual(resolved.text, "taigi", mode)
                XCTAssertTrue(resolved.wroteRomanization, mode)
            }
        }
    }

    func testHanjiFirst_commitsTheHanjiWithoutSpace() {
        let resolved = commit(candidate(roman: "tâi-gí", hanji: "台語"), isTranslateSwapped: true, isOutputBothScripts: false)
        XCTAssertEqual(resolved.text, "台語")
        XCTAssertFalse(resolved.wroteRomanization)
    }

    func testBrackets_writeThePairEitherWayRound_soBothEarnTheSpace() {
        let hanjiLed = commit(candidate(roman: "tâi-gí", hanji: "台語"), isTranslateSwapped: true, isOutputBothScripts: true)
        XCTAssertEqual(hanjiLed.text, "台語 (tâi-gí)")
        XCTAssertTrue(hanjiLed.wroteRomanization)

        let romanLed = commit(candidate(roman: "tâi-gí", hanji: "台語"), isTranslateSwapped: false, isOutputBothScripts: true)
        XCTAssertEqual(romanLed.text, "tâi-gí (台語)")
        XCTAssertTrue(romanLed.wroteRomanization)
    }

    /// TPS composes Bopomofo, which takes no word spacing: a hanji-less candidate still
    /// commits without earning the space (what it writes under TPS is an open decision).
    func testTPS_noHanji_earnsNoSpace() {
        let resolved = commit(candidate(roman: "taigi", hanji: nil), isTranslateSwapped: false, isTPSLayout: true, isOutputBothScripts: false)
        XCTAssertFalse(resolved.wroteRomanization)
    }
}
