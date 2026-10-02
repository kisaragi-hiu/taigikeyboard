// Swift-side entry point to the desktop shell seam (`macos/crates/taigi-macos-ffi`).

import RustTaigiSwift

/// Thin wrapper around `desktop_request_bytes`, the second bytes-in / bytes-out
/// function in the Rust archive, beside the engine's (`RustEngineBridge`).
/// It carries `taigi.desktop_shell` envelopes
/// (`macos/crates/taigi-macos-ffi/proto/desktop_shell.proto`); the typed
/// requests arrive with the phases that need them
/// (docs/architecture/macos-desktop-core-roadmap.md D3).
enum DesktopCoreBridge {
    /// Sends an encoded `taigi.desktop_shell.DesktopRequest` across the seam
    /// and returns the encoded `DesktopResponse`. The Rust side catches its
    /// own panics and always answers with a decodable response.
    static func send(_ requestBytes: [UInt8]) -> [UInt8] {
        requestBytes.withUnsafeBufferPointer { buffer in
            desktop_request_bytes(buffer).toArray()
        }
    }
}
