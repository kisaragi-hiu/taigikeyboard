// Swift-side entry point to the desktop shell seam (`macos/crates/taigi-macos-ffi`).

import Foundation
import RustTaigiSwift

/// Thin wrapper around `desktop_request_bytes`, the second bytes-in / bytes-out
/// function in the Rust archive, beside the engine's (`RustEngineBridge`).
/// It carries `taigi.desktop_shell` envelopes
/// (`macos/crates/taigi-macos-ffi/proto/desktop_shell.proto`; requests in
/// docs/architecture/macos-desktop-core-roadmap.md D3).
enum DesktopCoreBridge {
    /// Sends an encoded `taigi.desktop_shell.DesktopRequest` across the seam
    /// and returns the encoded `DesktopResponse`. The Rust side catches its
    /// own panics and always answers with a decodable response.
    static func send(_ requestBytes: [UInt8]) -> [UInt8] {
        requestBytes.withUnsafeBufferPointer { buffer in
            desktop_request_bytes(buffer).toArray()
        }
    }

    /// Encodes one request — with `settings`, the snapshot it runs under,
    /// when there is one — sends it, and returns the reply `expected` picks
    /// out of an OK response. `nil` — logged here — when the round-trip
    /// failed, the core refused, or it answered OK with another kind of
    /// reply or none.
    static func roundtrip<Reply>(
        _ request: Taigi_DesktopShell_DesktopRequest.OneOf_Request,
        settings: Taigi_DesktopShell_SettingsSnapshot? = nil,
        op: String,
        expected: (Taigi_DesktopShell_DesktopResponse.OneOf_Reply) -> Reply?,
    ) -> Reply? {
        let requestBytes: [UInt8]
        do {
            requestBytes = try Array(envelope(request, settings: settings).serializedData())
        } catch {
            recordFailure(op: op, message: "encode failed: \(error)")
            return nil
        }
        guard let response = try? Taigi_DesktopShell_DesktopResponse(
            serializedBytes: Data(send(requestBytes)),
        ) else {
            recordFailure(op: op, message: "response decode failed")
            return nil
        }
        guard response.error == .ok else {
            recordFailure(op: op, message: "core returned \(response.error)")
            return nil
        }
        guard let reply = response.reply.flatMap(expected) else {
            recordFailure(op: op, message: "expected reply missing, got \(String(describing: response.reply))")
            return nil
        }
        return reply
    }

    /// The request and its settings snapshot in one envelope: they cross
    /// the seam together and the core applies both or neither.
    static func envelope(
        _ request: Taigi_DesktopShell_DesktopRequest.OneOf_Request,
        settings: Taigi_DesktopShell_SettingsSnapshot?,
    ) -> Taigi_DesktopShell_DesktopRequest {
        var envelope = Taigi_DesktopShell_DesktopRequest()
        envelope.request = request
        if let settings {
            envelope.settings = settings
        }
        return envelope
    }

    /// Logs a seam failure.
    private static func recordFailure(op: String, message: String) {
        logger.error("[\(op)] \(message)")
    }

    private static let logger = DebugLogger(category: "DesktopCoreBridge")
}
