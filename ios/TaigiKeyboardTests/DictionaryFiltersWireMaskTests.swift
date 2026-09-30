@testable import TaigiKeyboard
import XCTest

/// Pins what `ComposingManager.fetchContinuousCandidates` puts in
/// `FetchAtPos.enabled_sources_bitmask` (`behavioral-invariants.md` §57).
///
/// The engine answers "every dictionary off" with `0`, which the composing
/// wire reserves for "platform did not wire this" and turns back into every
/// source. `DictionaryFilters.wireMask` sends the no-sources sentinel instead.
/// Mirrors Android `DictionaryFiltersWireMaskTest`, macOS
/// `RustEngineBridgeDictionaryFiltersTests` and desktop
/// `lexicon.rs::wire_mask_turns_all_off_into_the_sentinel`.
final class DictionaryFiltersWireMaskTests: XCTestCase {
    // INVARIANT_DICTIONARIES_ALL_OFF_OFFERS_NO_DICTIONARY_CANDIDATES (behavioral-invariants.md §57)
    func testWireMask_everyDictionaryOffThroughTheEngine_sendsTheNoSourcesSentinel() {
        // trace: compute_filters with every toggle off → source bits 0-12
        // empty, kautian off so no subcollection region → 0 → sentinel 1 << 13.
        let filters = RustEngineBridge.lexiconDictionaryFilters(toggles: Self.toggles { _ in })

        XCTAssertEqual(filters.dictionaryFilterBitmask, 0, "the engine's own all-off answer")
        XCTAssertEqual(filters.wireMask, RustEngineBridge.noSourcesEnabledBitmask)
        XCTAssertEqual(filters.wireMask, 1 << 13, "exactly the kautian-gate bit")
        XCTAssertEqual(filters.wireMask & 0x1FFF, 0, "the source region has to stay empty")
    }

    func testWireMask_oneDictionaryOnThroughTheEngine_sendsTheResolvedMask() {
        // trace: only itaigi on → bit 2; kautian off → no subcollection region.
        let filters = RustEngineBridge.lexiconDictionaryFilters(
            toggles: Self.toggles { $0.isITaigiDictEnabled = true },
        )

        XCTAssertEqual(filters.dictionaryFilterBitmask, 1 << 2)
        XCTAssertEqual(filters.wireMask, 1 << 2)
    }

    func testWireMask_resolvedZero_mapsToTheSentinel() {
        let allOff = RustEngineBridge.DictionaryFilters(dictionaryFilterBitmask: 0, enabledSources: [])

        XCTAssertEqual(allOff.wireMask, RustEngineBridge.noSourcesEnabledBitmask)
    }

    func testWireMask_resolvedNonZero_goesOutUnchanged() {
        let cases: [UInt32] = [0b101, 1 << 12, (1 << 13) | (1 << 14) | 1]
        for mask in cases {
            let filters = RustEngineBridge.DictionaryFilters(dictionaryFilterBitmask: mask, enabledSources: [])
            XCTAssertEqual(filters.wireMask, mask, "a resolved mask \(mask) must go out as it is")
        }
    }

    func testWireMask_failedResolve_staysAllSourcesOn() {
        XCTAssertEqual(
            RustEngineBridge.DictionaryFilters.allSourcesEnabled.wireMask,
            UInt32.max,
            "a failed resolve widens the list, it never empties it",
        )
    }

    /// Every dictionary toggle off, then `configure` switches some back on.
    private static func toggles(
        _ configure: (inout StubEngineSettings) -> Void,
    ) -> RustEngineBridge.DictionaryToggles {
        var settings = StubEngineSettings()
        settings.isMoeDictEnabled = false
        settings.isNewwordDictEnabled = false
        settings.isITaigiDictEnabled = false
        settings.isTaiwanPlantDictEnabled = false
        settings.isTaiHuaDictEnabled = false
        settings.isTaiwanJapanDictEnabled = false
        settings.isKunggeDictEnabled = false
        settings.isSttiDictEnabled = false
        settings.isKhpooDictEnabled = false
        settings.isVariantEnabled = false
        settings.isKhiinEnabled = false
        settings.isLkkDictEnabled = false
        settings.isDevDictEnabled = false
        configure(&settings)
        return RustEngineBridge.DictionaryToggles(from: settings)
    }
}
