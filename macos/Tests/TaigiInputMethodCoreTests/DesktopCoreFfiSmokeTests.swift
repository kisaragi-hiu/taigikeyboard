// Proves the desktop shell seam links and round-trips from Swift.

import Foundation
import SwiftProtobuf
@testable import TaigiInputMethodCore
import XCTest

/// `desktop_request_bytes` lives in the same static archive as the engine's
/// `process_request_bytes` (docs/architecture/macos-desktop-core-roadmap.md D1).
/// These cases push bytes through it and back: the generated
/// `RustTaigiDesktop.swift` wrapper, the shared `RustTaigi` C module, and the
/// prost decoder in `macos/crates/taigi-macos-ffi`. `EngineFfiSmokeTests` does
/// the same for the engine's seam in the same process.
final class DesktopCoreFfiSmokeTests: XCTestCase {
    func testVersionRequest_answersTheLibraryVersion() throws {
        var request = Taigi_DesktopShell_DesktopRequest()
        request.version = Taigi_DesktopShell_VersionRequest()

        let responseBytes = try DesktopCoreBridge.send([UInt8](request.serializedData()))
        let response = try Taigi_DesktopShell_DesktopResponse(serializedBytes: Data(responseBytes))

        XCTAssertEqual(response.error, .ok)
        // The crate version (`macos/Cargo.toml`); its exact value is pinned by
        // the crate's own Rust test. Here: a Rust-built reply, not a default.
        XCTAssertNotNil(
            response.version.version.wholeMatch(of: /\d+\.\d+\.\d+/),
            "expected a semantic version, got \"\(response.version.version)\"",
        )
    }

    /// Negative control (rust-ffi-safety.md §6 T4): only the Rust side turns
    /// undecodable bytes into `FAIL_PARSE`, so this tells "Rust answered" from
    /// "Swift decoded an empty default response".
    func testMalformedBytes_returnFailParse() throws {
        let responseBytes = DesktopCoreBridge.send([0xFF, 0xFF, 0xFF, 0xFF])
        let response = try Taigi_DesktopShell_DesktopResponse(serializedBytes: Data(responseBytes))

        XCTAssertEqual(response.error, .failParse)
    }
}
