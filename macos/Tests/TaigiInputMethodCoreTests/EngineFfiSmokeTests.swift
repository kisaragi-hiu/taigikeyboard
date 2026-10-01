// Proves the committed macOS Rust artefacts actually link and round-trip.

import SwiftProtobuf
@testable import TaigiInputMethodCore
import XCTest

/// The first real gate on `macos/RustEngine/RustTaigi.xcframework`: PR1 only
/// validated the artefact's *shape* (one macos-arm64_x86_64 slice). This case
/// pushes bytes through the static archive and back, exercising SwiftProtobuf
/// decode, the C module import, the swift-bridge wrapper, and the Rust
/// dispatcher in one hop. The happy path is every other `RustEngineBridge*`
/// suite, which all cross the same seam.
final class EngineFfiSmokeTests: XCTestCase {
    /// Negative control, and the macOS side of case T4 in
    /// `docs/contributing/rust-ffi-safety.md` §6 (iOS runs it as
    /// `RustEngineBridgeTests.test_T4_malformedBytes_returnsFailParse`).
    /// The happy path alone cannot tell "the Rust dispatcher answered" from
    /// "something Swift-side handed back a default `Response`": only the Rust
    /// side turns undecodable bytes into `FAIL_PARSE`.
    func testProcessRequest_malformedBytes_returnsFailParse() throws {
        let responseBytes = RustEngineBridge.processRequest([0xFF, 0xFF, 0xFF, 0xFF])
        let response = try Taigi_Engine_Response(serializedBytes: Data(responseBytes))

        XCTAssertEqual(response.error, .failParse, "malformed bytes must come back as FAIL_PARSE")
    }
}
