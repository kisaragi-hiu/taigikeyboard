// Drives the composing slice against the real engine and the real dictionary.

@testable import TaigiInputMethodCore
import XCTest

/// The Rust composing state is one per process, so these cases isolate
/// themselves by generation — see `GenerationCounter`.
final class RustEngineBridgeComposingTests: XCTestCase {
    private let settings = EngineSettings.defaults

    private var generation: UInt64 = 0

    override func setUp() {
        super.setUp()
        generation = TestFixtures.generationCounter.next()
        InstalledLexicon.installOnce()
    }

    /// Types `text` one character at a time, which is the only way a
    /// composition ever starts in production: `Append` begins the composition
    /// from Idle by itself, and there is no bridge op that seeds a whole buffer.
    @discardableResult
    private func compose(_ text: String) throws -> ComposingTransition {
        var last: ComposingTransition?
        for character in text {
            last = RustEngineBridge.composingAppend(
                String(character),
                settings: settings,
                generation: generation,
            )
        }
        return try XCTUnwrap(last, "composing '\(text)' produced no transition")
    }

    // MARK: - Composing

    func testAppend_rendersPreeditAndAsksHostToUpdateIt() throws {
        let transition = try compose("t")

        XCTAssertTrue(transition.isComposing, "starting a composition must report composing")
        XCTAssertEqual(transition.rawInput, "t")
        XCTAssertEqual(
            transition.effects.first,
            .updatePreedit("t", caretUTF16: 1),
            "the host has to be told to show the preedit before anything else",
        )
    }

    func testAppend_numericTone_rendersDiacriticInDisplayButKeepsRawDigits() throws {
        _ = try compose("tai")
        let transition = try XCTUnwrap(
            RustEngineBridge.composingAppend("5", settings: settings, generation: generation),
        )

        XCTAssertEqual(transition.rawInput, "tai5", "raw input stays the typed search key")
        XCTAssertEqual(
            transition.displayText,
            "t\u{00E2}i",
            "tone 5 must render as the circumflex the user reads",
        )
    }

    func testDeleteBackwardToEmpty_abortsWithoutTouchingTheDocument() throws {
        // The first keystroke already composes in the continuous phase, so
        // backspacing it away is the abort trio — never a document delete.
        _ = try compose("a")
        let transition = try XCTUnwrap(
            RustEngineBridge.composingDeleteBackward(settings: settings, generation: generation),
        )

        XCTAssertFalse(transition.isComposing)
        XCTAssertEqual(
            transition.effects,
            [.clearPreeditWithoutCommit, .clearCandidates, .nextWordClearForNewComposing],
            "the engine's abort trio must arrive whole and in order",
        )
    }

    func testReset_endsCompositionWithoutWritingToTheDocument() throws {
        _ = try compose("tai")
        let transition = try XCTUnwrap(RustEngineBridge.composingReset(generation: generation))

        XCTAssertFalse(transition.isComposing)
        XCTAssertTrue(
            transition.effects.contains(.clearPreeditWithoutCommit),
            "an abort must clear the preedit",
        )
        XCTAssertTrue(
            transition.effects.committedTexts.isEmpty,
            "an abort must not commit anything",
        )
    }

    func testCommitRaw_commitsTheCompositionAsRendered() throws {
        let composed = try compose("tai")

        let transition = try XCTUnwrap(
            RustEngineBridge.composingCommitRaw(settings: settings, generation: generation),
        )

        XCTAssertEqual(
            transition.effects.committedTexts,
            [composed.displayText],
            "Return must commit exactly what the marked region was showing",
        )
        XCTAssertFalse(transition.isComposing)
    }

    /// Guards the mistake this bridge used to document: committing the literal
    /// by handing the marked-region text to `SelectCandidate` re-prepends the
    /// nailed prefix (`engine/composing/src/transition.rs:724`), so a
    /// composition reading `台北大學` with `台北` nailed would commit
    /// `台北台北大學`. `CommitRaw` is the op that does not, which is why
    /// `SelectCandidate` has no macOS wrapper at all.
    func testCommitRaw_afterANailedSegment_writesTheCompositionOnceNotTwice() throws {
        _ = try compose("taigi")
        let pendingBytes = UInt32("taigi".utf8.count)
        let candidates = try XCTUnwrap(
            XCTUnwrap(
                RustEngineBridge.composingFetchAtPos(settings: settings, generation: generation),
            ).candidates,
        )
        let partial = try XCTUnwrap(
            candidates.first { $0.consumedSpanEnd > 0 && $0.consumedSpanEnd < pendingBytes },
            "taigi must offer a candidate shorter than the whole buffer to nail",
        )
        let committed = try XCTUnwrap(RustEngineBridge.composingCommitContinuous(
            script: .primary,
            roman: partial.roman,
            canonicalText: partial.displayText,
            associationTl: partial.canonicalTl,
            hanji: partial.hanji,
            consumedBytes: partial.consumedSpanEnd,
            syllableCount: partial.syllableCount,
            settings: settings,
            generation: generation,
        ))
        XCTAssertEqual(committed.commit.outcome, .nailed)
        let nailed = committed.transition
        XCTAssertTrue(nailed.isComposing, "a partial candidate nails a segment and keeps composing")

        let transition = try XCTUnwrap(
            RustEngineBridge.composingCommitRaw(settings: settings, generation: generation),
        )

        XCTAssertEqual(
            transition.effects.committedTexts,
            [nailed.displayText],
            "the nailed prefix belongs to the document once, not twice",
        )
    }

    func testCommitPreeditThenInsertExternal_commitsCompositionAndTrailingTextTogether() throws {
        _ = try compose("tai")
        let transition = try XCTUnwrap(RustEngineBridge.composingCommitPreeditThenInsertExternal(
            " ",
            settings: settings,
            generation: generation,
        ))

        let committed = transition.effects.committedTexts
        XCTAssertEqual(
            committed.count,
            1,
            "one keystroke must reach the host as one document mutation, not two",
        )
        XCTAssertEqual(committed.first?.hasSuffix(" "), true, "the space must ride along with the commit")
        XCTAssertFalse(transition.isComposing)
    }

    // MARK: - Continuous input

    func testFetchAtPos_afterComposing_returnsCandidates() throws {
        _ = try compose("taigi")

        let result = try XCTUnwrap(
            RustEngineBridge.composingFetchAtPos(settings: settings, generation: generation),
        )
        let candidates = try XCTUnwrap(result.candidates)
        let first = try XCTUnwrap(
            candidates.first,
            "the installed dictionary must produce candidates for a real word",
        )
        XCTAssertFalse(first.displayText.isEmpty)
        XCTAssertLessThanOrEqual(
            first.consumedSpanEnd,
            UInt32("taigi".utf8.count),
            "a consumed span must stay inside the raw buffer",
        )
    }

    /// The fetch carries the toggles and the engine filters by them; the user's
    /// custom dictionary is left out so only `dictionary.bin` rows carry hanji.
    // INVARIANT_DICTIONARIES_ALL_OFF_OFFERS_NO_DICTIONARY_CANDIDATES (behavioral-invariants.md §57)
    func testFetchAtPos_everyDictionaryOff_offersNoDictionaryCandidates() throws {
        let allOff = TestFixtures.settings(customDict: false, dictionarySources: .allSourcesOff)
        for character in "taigi" {
            _ = RustEngineBridge.composingAppend(String(character), settings: allOff, generation: generation)
        }

        let candidates = try XCTUnwrap(
            RustEngineBridge.composingFetchAtPos(settings: allOff, generation: generation)?.candidates,
        )
        XCTAssertEqual(candidates.compactMap(\.hanji), [])
        XCTAssertFalse(candidates.isEmpty, "the typed-text literal is not a dictionary row")
    }

    func testFetchAtPos_whenNotComposing_reportsNoContinuousPhaseRatherThanFailure() throws {
        let result = try XCTUnwrap(
            RustEngineBridge.composingFetchAtPos(settings: settings, generation: generation),
            "an idle engine answered the query; that is not a bridge failure",
        )

        XCTAssertNil(result.candidates, "no continuous phase must read as nil, not as an empty list")
    }

    /// §34 under the shipped defaults: Show Typed Text First is OFF out of the box on
    /// every platform (USER 2026-10-02), so with TL/POJ text composed a
    /// dictionary candidate leads the list, not the preedit literal. The ON
    /// half is `TaigiInputControllerCandidateTests`, which drives the same
    /// invert through the settings the shipped provider reads.
    func testFetchAtPos_literalRomanCandidateIsAbsent_underTheShippedDefaults() throws {
        _ = try compose("taigi")

        let shown = try XCTUnwrap(
            RustEngineBridge.composingFetchAtPos(settings: settings, generation: generation),
        )
        let leading = try XCTUnwrap(XCTUnwrap(shown.candidates).first)

        XCTAssertNotNil(leading.hanji, "nothing forces the one-script literal to the front")
    }
}
