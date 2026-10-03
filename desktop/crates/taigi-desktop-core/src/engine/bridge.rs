//! The envelope round-trip and the per-request config snapshot.

use std::sync::atomic::{AtomicU32, Ordering};

use prost::Message;
use protos::engine::{request, response, AppConfig, ErrorCode, Platform, Request, Response};

use crate::platform::DesktopPlatform;
use crate::settings::EngineSettings;

static LAST_REQUEST_ID: AtomicU32 = AtomicU32::new(0);

fn next_request_id() -> u32 {
    LAST_REQUEST_ID
        .fetch_add(1, Ordering::Relaxed)
        .wrapping_add(1)
}

/// Encodes one request, dispatches it, and returns the response payload when
/// the engine reported success. `None` means the round-trip FAILED rather than
/// "the engine had nothing to say" — callers must keep the two apart, because
/// a failed round-trip leaves the engine's state untouched and any snapshot
/// synthesized here would contradict it.
///
/// Shared by every slice: envelope, id sequence, error checks and failure log
/// are identical, only the payload case differs.
pub(super) fn roundtrip(
    payload: request::Payload,
    op: &str,
    generation: u64,
    config: Option<AppConfig>,
) -> Option<response::Payload> {
    let request = Request {
        id: next_request_id(),
        generation,
        payload: Some(payload),
        config_snapshot: config,
    };
    let request_id = request.id;
    log::debug!("[engine->] op={op} id={request_id} generation={generation}");

    let response_bytes = dispatch::process_request(&request.encode_to_vec());
    let response = match Response::decode(response_bytes.as_slice()) {
        Ok(response) => response,
        Err(error) => {
            record_failure(op, &format!("response decode failed: {error}"));
            return None;
        }
    };
    // The call is synchronous, so a mismatched id means the response belongs
    // to some other request — reading its payload would apply another
    // operation's state to this one.
    if response.id != request_id {
        record_failure(
            op,
            &format!(
                "response id {} does not match request {request_id}",
                response.id
            ),
        );
        return None;
    }
    let error = ErrorCode::try_from(response.error).unwrap_or(ErrorCode::FailInternal);
    if error != ErrorCode::Ok {
        record_failure(op, &format!("engine returned {error:?}"));
        return None;
    }
    match response.payload {
        Some(payload) => Some(payload),
        None => {
            record_failure(op, "response carried no payload");
            None
        }
    }
}

/// One place for every bridge failure, so a degraded engine is visible in the
/// log instead of surfacing only as candidates that never appear.
pub(super) fn record_failure(op: &str, message: &str) {
    log::error!("[{op}] {message}");
}

/// The caller identity the engine validates — no engine behaviour branches
/// on it (envelope.proto `Platform`). Each shell passes its own desktop
/// (`taigi-macos-ffi` `runtime.rs` passes `DesktopPlatform::MacOS`).
fn wire_platform(platform: DesktopPlatform) -> Platform {
    match platform {
        DesktopPlatform::Windows => Platform::Windows,
        DesktopPlatform::Linux => Platform::Linux,
        DesktopPlatform::MacOS => Platform::Macos,
    }
}

/// The one config builder. The engine holds no settings of its own; every
/// request carries the snapshot it should be rendered under. `platform_id` is
/// set on every request, not only the ones that read it: the next-word engine
/// rejects the unset value outright (`engine/nextword/src/decide.rs:50-51`).
///
/// Both double-tap folds are unconditional here, unlike iOS and Android where
/// they are user settings: their on-screen keyboards have dedicated `o͘` and
/// `ⁿ` keys, a hardware keyboard has not, so switching the fold off would
/// leave both graphemes untypable in POJ.
///
/// The engine collapses same-roman rows under roman-only
/// (`candidate_display_mode`), shapes the romanization hyphenless (§49) and
/// cases the nasal marker (§53) in the preedit, the candidate fetch and the
/// next-word filter; the nasal switch is inverted on the wire (proto default =
/// the marker follows the case). The swap flag renders a continuous
/// composition's nailed prefix (`docs/engine/continuous-commit-and-display.md`
/// §10.2) and feeds the next-word decide table, where it suppresses recording
/// for raw-romanization commits (`decide.rs:86`). Sent by every composing op
/// that renders the composition and by every next-word request, matching iOS;
/// macOS sends this same builder's config.
pub(super) fn app_config(settings: &EngineSettings, platform: DesktopPlatform) -> AppConfig {
    AppConfig {
        input_mode: settings.input_mode.wire().to_owned(),
        oo_doubletap_enabled: true,
        nn_doubletap_enabled: true,
        is_hanji_first: settings.is_hanji_first,
        platform_id: wire_platform(platform) as i32,
        candidate_display_mode: settings.candidate_display_mode.wire() as i32,
        hyphenless_roman: settings.is_hyphenless_roman_enabled,
        force_lowercase_nasal_marker: !settings.is_nasal_marker_uppercase_enabled,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::test_support::TEST_PLATFORM;
    use crate::settings::{CandidateDisplayMode, InputMode};
    use protos::engine::CandidateDisplayMode as WireDisplayMode;

    /// The rendering tests below are not about the platform.
    fn config(settings: &EngineSettings) -> AppConfig {
        app_config(settings, TEST_PLATFORM)
    }

    #[test]
    fn app_config_carries_platform_and_unconditional_doubletaps() {
        // trace: `wire_platform` maps each shell's own desktop one to one;
        // before D6 the build target picked Linux or Windows.
        let settings = EngineSettings {
            input_mode: InputMode::Poj,
            ..EngineSettings::default()
        };
        for (platform, expected) in [
            (DesktopPlatform::Windows, Platform::Windows),
            (DesktopPlatform::Linux, Platform::Linux),
            (DesktopPlatform::MacOS, Platform::Macos),
        ] {
            assert_eq!(
                app_config(&settings, platform).platform_id,
                expected as i32,
                "{platform:?}"
            );
        }
        let config = config(&settings);
        assert_eq!(config.input_mode, "poj");
        assert!(config.oo_doubletap_enabled && config.nn_doubletap_enabled);
        assert_eq!(
            config.candidate_display_mode,
            WireDisplayMode::SideBySide as i32,
            "the default is spelled out, not left Unspecified"
        );
        assert!(!config.is_roman_only_display());
    }

    #[test]
    fn combined_reaches_the_wire_with_the_derived_swap() {
        // trace: the derivation is pinned in `document.rs`; here only the
        // forwarding — the bridge carries the mode and the swap it was handed,
        // and the engine's only normaliser still reads it as "not roman-only".
        let settings = EngineSettings {
            is_hanji_first: true,
            candidate_display_mode: CandidateDisplayMode::Combined,
            ..EngineSettings::default()
        };
        let continuous = config(&settings);
        assert_eq!(
            continuous.candidate_display_mode,
            WireDisplayMode::Combined as i32
        );
        assert!(continuous.is_hanji_first);
        assert!(!continuous.is_roman_only_display());
    }

    #[test]
    fn roman_only_reaches_the_config() {
        let settings = EngineSettings {
            candidate_display_mode: CandidateDisplayMode::RomanOnly,
            ..EngineSettings::default()
        };
        assert!(config(&settings).is_roman_only_display());
    }

    #[test]
    fn hyphenless_roman_reaches_the_config() {
        let settings = EngineSettings {
            is_hyphenless_roman_enabled: true,
            ..EngineSettings::default()
        };
        assert!(config(&settings).hyphenless_roman);
    }

    // INVARIANT_NASAL_MARKER_CASE_FOLLOWS_THE_SWITCH (behavioral-invariants.md §53)
    #[test]
    fn nasal_marker_uppercase_off_forces_the_lowercase_marker_through_the_config() {
        assert!(
            !config(&EngineSettings::default()).force_lowercase_nasal_marker,
            "ships ON = wire default"
        );
        let settings = EngineSettings {
            is_nasal_marker_uppercase_enabled: false,
            ..EngineSettings::default()
        };
        assert!(config(&settings).force_lowercase_nasal_marker);
    }

    #[test]
    fn app_config_carries_the_swap_flag() {
        let settings = EngineSettings {
            is_hanji_first: true,
            ..EngineSettings::default()
        };
        let swapped = config(&settings);
        assert!(swapped.is_hanji_first);
        assert!(
            !swapped.output_both_scripts,
            "desktop has no Annotate in Brackets: the wire field stays at its default"
        );
        let roman_first = EngineSettings {
            is_hanji_first: false,
            ..EngineSettings::default()
        };
        assert!(!config(&roman_first).is_hanji_first);
    }

    #[test]
    fn successive_request_ids_differ_and_start_non_zero() {
        let a = next_request_id();
        let b = next_request_id();
        assert_ne!(a, b);
        assert_ne!(a, 0);
    }
}
