//! The one static library the macOS input method links
//! (docs/architecture/macos-desktop-core-roadmap.md D1, D2).
//!
//! Two swift-bridge seams in one archive: the engine's, unchanged, from
//! `engine/swift-ffi` (`process_request_bytes` and the logger calls), and the
//! desktop shell's, `desktop_request_bytes`, defined here. Both are bytes in,
//! bytes out — protobuf envelopes, no handle crosses, no `unsafe` here
//! (docs/contributing/rust-ffi-safety.md §1.1).

// Links the engine seam into this archive. Nothing here calls into it; without
// the `extern crate` rustc would not link an rlib this crate never names, and
// the archive would lack the engine's exports.
extern crate rust_taigi;

use std::panic::{catch_unwind, AssertUnwindSafe};

use prost::Message;
use protos::engine::ErrorCode;

/// prost types for `proto/desktop_shell.proto` (package `taigi.desktop_shell`).
#[allow(clippy::all, clippy::pedantic)]
mod shell {
    include!(concat!(env!("OUT_DIR"), "/taigi.desktop_shell.rs"));
}

use shell::{desktop_request, desktop_response, DesktopRequest, DesktopResponse, VersionReply};

// Bridge module. Doc comments live OUTSIDE this block — swift-bridge's parser
// rejects `///` on the items inside.
//
// `desktop_request_bytes`: decodes a `taigi.desktop_shell.DesktopRequest`,
// answers with an encoded `taigi.desktop_shell.DesktopResponse`. Always a
// decodable response — never panics across the seam.
#[swift_bridge::bridge]
mod ffi {
    extern "Rust" {
        fn desktop_request_bytes(bytes: &[u8]) -> Vec<u8>;
    }
}

fn desktop_request_bytes(bytes: &[u8]) -> Vec<u8> {
    answer(|| respond(bytes))
}

/// The seam's panic boundary (rust-ffi-safety.md §1.2): a panic in `respond`
/// answers FAIL_INTERNAL. Separate from `desktop_request_bytes` only so the
/// tests can drive this exact path with a panicking closure.
fn answer(respond: impl FnOnce() -> DesktopResponse) -> Vec<u8> {
    catch_unwind(AssertUnwindSafe(respond))
        .unwrap_or_else(|_| {
            log::error!("desktop request panicked");
            error_response(ErrorCode::FailInternal)
        })
        .encode_to_vec()
}

fn respond(bytes: &[u8]) -> DesktopResponse {
    // Checked before decoding, as on the engine seam (rust-ffi-safety.md §1.4).
    if dispatch::is_request_too_large(bytes.len()) {
        return error_response(ErrorCode::FailInvariant);
    }
    let request = match DesktopRequest::decode(bytes) {
        Ok(request) => request,
        Err(error) => {
            log::warn!("desktop request decode failed: {error}");
            return error_response(ErrorCode::FailParse);
        }
    };
    let Some(request) = request.request else {
        log::warn!("desktop request has no variant");
        return error_response(ErrorCode::FailInvariant);
    };
    let reply = match request {
        desktop_request::Request::Version(_) => desktop_response::Reply::Version(VersionReply {
            version: env!("CARGO_PKG_VERSION").to_owned(),
        }),
    };
    DesktopResponse {
        error: ErrorCode::Ok as i32,
        reply: Some(reply),
    }
}

fn error_response(code: ErrorCode) -> DesktopResponse {
    DesktopResponse {
        error: code as i32,
        reply: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shell::VersionRequest;

    fn decode(bytes: &[u8]) -> DesktopResponse {
        DesktopResponse::decode(bytes).expect("the seam always answers a decodable response")
    }

    fn send(request: &DesktopRequest) -> DesktopResponse {
        decode(&desktop_request_bytes(&request.encode_to_vec()))
    }

    fn version_request() -> DesktopRequest {
        DesktopRequest {
            request: Some(desktop_request::Request::Version(VersionRequest {})),
        }
    }

    fn version_reply() -> Option<desktop_response::Reply> {
        // trace: CARGO_PKG_VERSION = macos/Cargo.toml [workspace.package] version.
        Some(desktop_response::Reply::Version(VersionReply {
            version: env!("CARGO_PKG_VERSION").to_owned(),
        }))
    }

    /// A Version request padded with one unknown length-delimited field
    /// (field 15) to exactly `total_len` bytes — still a valid request, which
    /// the decoder answers like the bare one.
    fn padded_version_request(total_len: usize) -> Vec<u8> {
        let mut bytes = version_request().encode_to_vec();
        let header_len = bytes.len() + prost::encoding::key_len(15);
        // The payload length's own varint counts toward `total_len`.
        let payload_len = (1..=10)
            .find_map(|varint_len| {
                let len = total_len - header_len - varint_len;
                (prost::encoding::encoded_len_varint(len as u64) == varint_len).then_some(len)
            })
            .expect("a length whose varint fits");
        prost::encoding::encode_key(15, prost::encoding::WireType::LengthDelimited, &mut bytes);
        prost::encoding::encode_varint(payload_len as u64, &mut bytes);
        bytes.resize(total_len, 0);
        bytes
    }

    #[test]
    fn version_request_answers_the_crate_version() {
        let response = send(&version_request());

        assert_eq!(response.error, ErrorCode::Ok as i32);
        assert_eq!(response.reply, version_reply());
    }

    /// rust-ffi-safety.md §6 T1, on this seam's own catch path.
    #[test]
    fn panic_answers_fail_internal() {
        let response = decode(&answer(|| panic!("injected panic")));

        assert_eq!(response.error, ErrorCode::FailInternal as i32);
        assert_eq!(response.reply, None);
    }

    /// rust-ffi-safety.md §6 T3 for a stateless request: concurrent calls
    /// each answer. (No runtime state exists yet to race on.)
    #[test]
    fn concurrent_version_requests_all_answer() {
        let request = version_request().encode_to_vec();
        std::thread::scope(|scope| {
            let workers: Vec<_> = (0..8)
                .map(|_| scope.spawn(|| decode(&desktop_request_bytes(&request))))
                .collect();
            for worker in workers {
                let response = worker.join().expect("no panic escapes the seam");
                assert_eq!(response.error, ErrorCode::Ok as i32);
                assert_eq!(response.reply, version_reply());
            }
        });
    }

    /// rust-ffi-safety.md §6 T4: malformed bytes answer FAIL_PARSE.
    #[test]
    fn malformed_bytes_answer_fail_parse() {
        let response = decode(&desktop_request_bytes(&[0xFF, 0xFF, 0xFF, 0xFF]));

        assert_eq!(response.error, ErrorCode::FailParse as i32);
        assert_eq!(response.reply, None);
    }

    #[test]
    fn request_without_variant_answers_fail_invariant() {
        let response = send(&DesktopRequest { request: None });

        assert_eq!(response.error, ErrorCode::FailInvariant as i32);
        assert_eq!(response.reply, None);
    }

    /// A newer Swift side's variant this library does not know: prost keeps
    /// it as an unknown field, so the request decodes with no variant.
    #[test]
    fn unknown_variant_answers_fail_invariant() {
        // trace: field 99, wire type 2 → key 99 << 3 | 2 = 794 = varint [0x9A, 0x06]; length 0.
        let response = decode(&desktop_request_bytes(&[0x9A, 0x06, 0x00]));

        assert_eq!(response.error, ErrorCode::FailInvariant as i32);
        assert_eq!(response.reply, None);
    }

    /// rust-ffi-safety.md §6 T5: a valid request of exactly the cap is
    /// answered; one byte more is refused before decoding.
    #[test]
    fn request_size_cap_is_inclusive() {
        let at_cap = padded_version_request(dispatch::MAX_REQUEST_BYTES);
        assert_eq!(at_cap.len(), dispatch::MAX_REQUEST_BYTES);
        let response = decode(&desktop_request_bytes(&at_cap));
        assert_eq!(response.error, ErrorCode::Ok as i32);
        assert_eq!(response.reply, version_reply());

        let over_cap = padded_version_request(dispatch::MAX_REQUEST_BYTES + 1);
        let response = decode(&desktop_request_bytes(&over_cap));
        assert_eq!(response.error, ErrorCode::FailInvariant as i32);
        assert_eq!(response.reply, None);
    }
}
