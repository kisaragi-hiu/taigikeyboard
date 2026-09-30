@testable import TaigiKeyboard
import XCTest

// The wire config every request carries: what `RustEngineBridge.appConfig` and the composing
// projection `continuousAppConfig` put on each field. Mirrors Android `EngineAppConfigTest`.
final class RustEngineBridgeAppConfigTests: XCTestCase {
    /// The TPS layout goes out as `"tps"` with the swap and No Hyphens as stored — the engine
    /// applies the TPS fold itself (`AppConfig::renders_hanji_first` / `renders_hyphenless`).
    func testContinuousAppConfig_tpsLayout_sendsTpsWithTheStoredFlags() {
        let settings = StubEngineSettings(
            inputMode: .tps,
            isTranslateSwapped: false,
            isHyphenlessRomanEnabled: true,
            isTpsOrMappedToER: true,
        )

        let config = RustEngineBridge.continuousAppConfig(settings)

        XCTAssertEqual(config.inputMode, "tps")
        XCTAssertFalse(config.isTranslateSwapped, "the swap goes out unfolded; the engine reads tps as Hanji-first")
        XCTAssertTrue(config.hyphenlessRoman, "No Hyphens goes out as stored; the engine exempts tps")
        XCTAssertTrue(config.tpsOrMapsToEr, "the or→er dialect switch rides the config")
        XCTAssertEqual(config.platformID, .ios)
    }

    func testContinuousAppConfig_romanizationModes_forwardEverySetting() {
        let cases: [(InputMode, String)] = [(.tl, "tl"), (.poj, "poj"), (.english, "english")]
        for (mode, wire) in cases {
            let settings = StubEngineSettings(
                inputMode: mode,
                isTranslateSwapped: true,
                isOutputBothScripts: true,
                candidateDisplayMode: .combined,
                isHyphenlessRomanEnabled: true,
                pojMarkerOptions: PojMarkerOptions(
                    isDoubleTapOOEnabled: true,
                    isDoubleTapNNEnabled: true,
                    isNasalMarkerUppercaseEnabled: false,
                ),
            )

            let config = RustEngineBridge.continuousAppConfig(settings)

            XCTAssertEqual(config.inputMode, wire, "\(mode)")
            XCTAssertTrue(config.isTranslateSwapped, "\(mode)")
            XCTAssertTrue(config.outputBothScripts, "\(mode)")
            XCTAssertEqual(config.candidateDisplayMode, .combined, "\(mode)")
            XCTAssertTrue(config.hyphenlessRoman, "\(mode)")
            XCTAssertTrue(config.ooDoubletapEnabled, "\(mode)")
            XCTAssertTrue(config.nnDoubletapEnabled, "\(mode)")
            XCTAssertTrue(config.forceLowercaseNasalMarker, "\(mode): ⁿ becomes ᴺ OFF is inverted on the wire")
            XCTAssertFalse(config.tpsOrMapsToEr, "\(mode)")
            XCTAssertEqual(config.platformID, .ios, "\(mode)")
        }
    }

    /// Nextword and case transform pass only what they read; the rest keeps the proto defaults,
    /// and `platform_id` is set on every request (nextword rejects it unset).
    func testAppConfig_unpassedFields_keepTheProtoDefaults() {
        let config = RustEngineBridge.appConfig(mode: .tps, isTranslateSwapped: true)

        XCTAssertEqual(config.inputMode, "tps")
        XCTAssertTrue(config.isTranslateSwapped)
        XCTAssertEqual(config.platformID, .ios)
        XCTAssertFalse(config.ooDoubletapEnabled)
        XCTAssertFalse(config.nnDoubletapEnabled)
        XCTAssertFalse(config.forceLowercaseNasalMarker)
        XCTAssertFalse(config.outputBothScripts)
        XCTAssertEqual(config.candidateDisplayMode, .sideBySide)
        XCTAssertFalse(config.hyphenlessRoman)
        XCTAssertFalse(config.tpsOrMapsToEr)
    }
}
