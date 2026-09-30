// The wire config every request carries: what this platform pins, not what the user chose.

@testable import TaigiInputMethodCore
import XCTest

final class RustEngineBridgeAppConfigTests: XCTestCase {
    /// POJ `oo`→`o͘` / `nn`→`ⁿ` folding is a user setting on iOS and Android,
    /// whose on-screen keyboards have dedicated keys for both graphemes. A
    /// hardware keyboard has none, so macOS pins the fold on. Protobuf `bool`
    /// defaults to `false`, which means a refactor that drops an assignment
    /// leaves that grapheme untypable in POJ rather than failing loudly — this
    /// case is what notices.
    func testAppConfig_pinsBothPojDoubletapFoldsOn() {
        let config = RustEngineBridge.appConfig(.defaults)

        XCTAssertTrue(config.ooDoubletapEnabled)
        XCTAssertTrue(config.nnDoubletapEnabled)
    }

    func testAppConfig_identifiesThePlatformAsMacOS() {
        XCTAssertEqual(RustEngineBridge.appConfig(.defaults).platformID, .macos)
    }

    /// The display mode rides every request — the composing dispatcher
    /// collapses same-romanization rows on it and the next-word filter reads it
    /// too. `.unspecified` is the wire default and means side-by-side, so a
    /// dropped assignment would silently leave a romanization-only install with
    /// duplicate cells.
    func testAppConfig_carriesTheCandidateDisplayMode() {
        let sideBySide = TestFixtures.settings(candidateDisplayMode: .sideBySide)
        let romanOnly = TestFixtures.settings(candidateDisplayMode: .romanOnly)
        let combined = TestFixtures.settings(candidateDisplayMode: .combined)

        XCTAssertEqual(RustEngineBridge.appConfig(sideBySide).candidateDisplayMode, .sideBySide)
        XCTAssertEqual(RustEngineBridge.appConfig(romanOnly).candidateDisplayMode, .romanOnly)
        XCTAssertEqual(RustEngineBridge.appConfig(combined).candidateDisplayMode, .combined)
        XCTAssertNotEqual(RustEngineBridge.appConfig(.defaults).candidateDisplayMode, .unspecified)
    }

    /// Every request carries the swap: the composing engine renders the nailed
    /// prefix by it (§10.2) and the next-word decide table reads it, so the
    /// flag cannot be left to a subset of the requests.
    func testAppConfig_carriesTheSwap() {
        XCTAssertTrue(RustEngineBridge.appConfig(TestFixtures.settings(swapped: true)).isTranslateSwapped)
        XCTAssertFalse(RustEngineBridge.appConfig(TestFixtures.settings(swapped: false)).isTranslateSwapped)
    }
}
