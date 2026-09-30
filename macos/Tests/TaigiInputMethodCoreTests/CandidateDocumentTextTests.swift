// Pins what each output setting writes into the document.

@testable import TaigiInputMethodCore
import XCTest

final class CandidateDocumentTextTests: XCTestCase {
    private let word = TestFixtures.candidate(roman: "tâi-gí", hanji: "台語")

    func testDefaults_writeTheRomanization() {
        let text = CandidateDocumentText.text(
            for: word,
            settings: TestFixtures.settings(swapped: false),
        )

        XCTAssertEqual(text, "tâi-gí")
    }

    func testSwapped_writesTheHanji() {
        let text = CandidateDocumentText.text(
            for: word,
            settings: TestFixtures.settings(swapped: true),
        )

        XCTAssertEqual(text, "台語")
    }

    func testRomanizationOnlyCandidate_writesTheRomanizationUnderEverySetting() {
        let romanOnly = TestFixtures.candidate(roman: "tâi-gí", hanji: nil)

        for swapped in [true, false] {
            XCTAssertEqual(
                CandidateDocumentText.text(
                    for: romanOnly,
                    settings: TestFixtures.settings(swapped: swapped),
                ),
                "tâi-gí",
                "no hanji exists to swap to (swapped: \(swapped))",
            )
        }
    }

    // MARK: - The auto-space verdict that travels with the text

    /// The truth table `AutoSpacePolicy.isGateActive` reads. Spacing is a
    /// property of romanization, so the verdict follows the STRING this branch
    /// picked, never the output mode.
    func testResolved_saysWhetherTheStringItPickedCarriesRomanization() {
        // trace: resolved() — hanji + !swapped → roman → true.
        XCTAssertTrue(
            CandidateDocumentText.resolved(
                for: word, settings: TestFixtures.settings(swapped: false),
            ).wroteRomanization,
        )
        // hanji + swapped → the bare 台語 → false.
        XCTAssertFalse(
            CandidateDocumentText.resolved(
                for: word, settings: TestFixtures.settings(swapped: true),
            ).wroteRomanization,
            "a pure 漢字 commit earns no space",
        )
    }

    /// trace: `resolved` — the hanji-absent arm. Romanization under EVERY
    /// mode, including the two the old mode proxy called a hanji commit
    /// (Hanji-first and Hanji with Romanization).
    func testResolved_aCandidateWithNoHanjiAlwaysCarriesRomanization() {
        let romanOnly = TestFixtures.candidate(roman: "taigi", hanji: nil)

        for swapped in [false, true] {
            let resolved = CandidateDocumentText.resolved(
                for: romanOnly,
                settings: TestFixtures.settings(swapped: swapped),
            )
            XCTAssertEqual(resolved.text, "taigi")
            XCTAssertTrue(resolved.wroteRomanization, "swapped: \(swapped)")
        }
    }

    /// Space writes the script the mode does NOT lead with, so the verdict
    /// inverts with it. The strings themselves are pinned by
    /// `testAlternate_isWhicheverScriptThePrimaryIsNot`.
    func testResolvedAlternate_invertsTheMode() {
        XCTAssertEqual(
            CandidateDocumentText.resolvedAlternate(
                for: word, settings: TestFixtures.settings(swapped: true),
            )?.wroteRomanization, true,
        )
        XCTAssertEqual(
            CandidateDocumentText.resolvedAlternate(
                for: word, settings: TestFixtures.settings(swapped: false),
            )?.wroteRomanization, false,
            "Space wrote the hanji",
        )
    }

    func testPresentButEmptyHanji_isTreatedAsAbsent() {
        let defective = TestFixtures.candidate(roman: "tâi-gí", hanji: "")

        XCTAssertEqual(
            CandidateDocumentText.text(for: defective, settings: TestFixtures.settings(swapped: false)),
            "tâi-gí",
        )
        XCTAssertEqual(
            CandidateDocumentText.text(for: defective, settings: TestFixtures.settings(swapped: true)),
            "tâi-gí",
            "swapping to an empty hanji would commit nothing at all",
        )
    }

    // MARK: - The other script (the Hanji/romanization key)

    /// Space writes whichever script Return does not — that is the whole
    /// gesture, and it is why the key needs no mode of its own.
    func testAlternate_isWhicheverScriptThePrimaryIsNot() {
        for swapped in [false, true] {
            let settings = TestFixtures.settings(swapped: swapped)
            let primary = CandidateDocumentText.text(for: word, settings: settings)
            let alternate = CandidateDocumentText.resolvedAlternate(for: word, settings: settings)?.text

            XCTAssertEqual(alternate, swapped ? "tâi-gí" : "台語", "swapped: \(swapped)")
            XCTAssertNotEqual(alternate, primary)
        }
    }

    /// A romanization-only candidate — the §34 literal candidate, an
    /// out-of-vocabulary name — has no second script, and saying so is what
    /// lets Space decline the key rather than write the romanization twice.
    func testAlternate_isAbsentWhenTheCandidateHasOneScript() {
        for hanji in [nil, ""] {
            let romanOnly = TestFixtures.candidate(roman: "Tsng-kiô", hanji: hanji)
            for swapped in [false, true] {
                XCTAssertNil(
                    CandidateDocumentText.resolvedAlternate(
                        for: romanOnly,
                        settings: TestFixtures.settings(swapped: swapped),
                    )?.text,
                    "hanji: \(String(describing: hanji)), swapped: \(swapped)",
                )
            }
        }
    }

    /// The romanization-only display shows no Hanji, so there is none to offer:
    /// Space stays `.ignored` there whatever the swap flag says, rather than
    /// writing a script the user never saw.
    func testAlternate_underRomanOnly_isAbsent() {
        for swapped in [false, true] {
            XCTAssertNil(
                CandidateDocumentText.resolvedAlternate(
                    for: word,
                    settings: TestFixtures.settings(swapped: swapped, candidateDisplayMode: .romanOnly),
                )?.text,
                "swapped: \(swapped)",
            )
        }
    }

    /// Space writes exactly the script the bar shows under the primary one.
    /// The user is looking at the offer before they take it; the two are
    /// resolved from the same settings, and this pins that they agree.
    func testAlternate_isTheScriptTheCellShowsBesideThePrimary() {
        for swapped in [false, true] {
            let settings = TestFixtures.settings(swapped: swapped)

            XCTAssertEqual(
                CandidateDocumentText.resolvedAlternate(for: word, settings: settings)?.text,
                CandidateCellContent.cell(for: word, settings: settings).annotation,
                "swapped: \(swapped)",
            )
        }
    }

    /// Verbatim, hyphens included. 298 dictionary entries write the neutral-tone `--`
    /// into the hanji field — MOE orthography, pinned by §21/S8 — and the mixed-script
    /// mixed entries carry the romanized half's own hyphen. Stripping either
    /// here would be this layer second-guessing the dictionary.
    func testAlternate_writesTheFieldVerbatim() {
        let hanjiSettings = TestFixtures.settings(swapped: false)
        for (roman, hanji) in [("kau--lâng", "交--人"), ("âng-kì-kì", "紅kì-kì")] {
            XCTAssertEqual(
                CandidateDocumentText.resolvedAlternate(
                    for: TestFixtures.candidate(roman: roman, hanji: hanji),
                    settings: hanjiSettings,
                )?.text,
                hanji,
            )
        }
    }
}
