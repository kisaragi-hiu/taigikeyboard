//! Composing slice of the engine bridge: the intents the desktop sends and
//! the decoding of what comes back. Port of `RustEngineBridge+Composing.swift`.
//!
//! This is a subset of the engine's intents, for the reasons that
//! file documents at length: `AppendHyphen` is an alias for `Append("-")`,
//! `ReplaceLast` is TPS-only, `Start` is unnecessary (`Append` begins the
//! composition from Idle), `SelectCandidate` double-counts the nailed
//! prefix under `Phase::Continuous` so the literal commit is `CommitRaw`. Candidate navigation is a permanent platform-side
//! concern (`cross-platform-alignment.md` §4.1).
//!
//! Every op answers `None` when the round-trip itself failed, which is a
//! different thing from the engine answering that it is idle.

/// Which of a pick's scripts a candidate commit writes — the wire enum, so a
/// caller maps its own cell script onto it once.
pub use protos::engine::CommitScript;
use protos::engine::{
    composing_request, request, response, Append, CaretDirection as WireCaretDirection,
    CommitContinuous, CommitPreeditThenInsertExternal, CommitRaw, ComposingRequest,
    ComposingResponse, DeleteBackward, FetchAtPos, MoveCaret, Reset, TelexKey,
};

use crate::keys::CaretDirection;

use super::bridge::{app_config, record_failure, roundtrip};
use super::lexicon::dictionary_toggles;
use super::transition::{
    ComposingTransition, ContinuousCandidate, ContinuousCommitResult, ContinuousFetchResult,
};
use crate::settings::EngineSettings;

/// Appends one typed character to the raw buffer.
///
/// Carries the config with the swap flag, as every op that re-renders the
/// composition does: under `Phase::Continuous` the answer is the whole
/// marked region, nailed prefix included, and the prefix's word-boundary
/// spacing reads that flag. Without it a nail rendered `台gi` and the next
/// keystroke `台 gi` (found by the composing-caret round, 2026-09-09).
pub fn append(
    character: &str,
    settings: &EngineSettings,
    generation: u64,
) -> Option<ComposingTransition> {
    dispatch(
        composing_request::Method::Append(Append {
            char: character.to_owned(),
        }),
        "composingAppend",
        generation,
        Some(app_config(settings)),
    )
}

/// Applies one Telex key to the pending syllable's tone — or, for `z`,
/// types the affricate initial the input mode spells (`composing.proto`
/// `TelexKey`, `engine/composing/src/telex.rs`). Carries the same config
/// as `append` (`z` resolves by `input_mode`, the prefix by the spacing
/// flags). Port of `composingTelexKey` (`RustEngineBridge+Composing.swift`).
pub fn telex_key(
    key: &str,
    settings: &EngineSettings,
    generation: u64,
) -> Option<ComposingTransition> {
    dispatch(
        composing_request::Method::TelexKey(TelexKey {
            key: key.to_owned(),
        }),
        "composingTelexKey",
        generation,
        Some(app_config(settings)),
    )
}

/// Drops the character before the caret. Same config as `append`.
pub fn delete_backward(settings: &EngineSettings, generation: u64) -> Option<ComposingTransition> {
    dispatch(
        composing_request::Method::DeleteBackward(DeleteBackward {}),
        "composingDeleteBackward",
        generation,
        Some(app_config(settings)),
    )
}

/// Steps the caret one character inside the pending tail (`composing.proto`
/// `MoveCaret`). The buffer is untouched, so the engine answers with an
/// `UpdatePreedit` carrying the new caret and nothing else — no fetch is
/// requested. Same config as `append`: the answer re-renders the
/// composition the way the last keystroke did, so a move never changes the
/// text on screen (`composingMoveCaret`, `RustEngineBridge+Composing.swift`).
pub fn move_caret(
    direction: CaretDirection,
    settings: &EngineSettings,
    generation: u64,
) -> Option<ComposingTransition> {
    let wire = match direction {
        CaretDirection::Left => WireCaretDirection::Left,
        CaretDirection::Right => WireCaretDirection::Right,
    };
    dispatch(
        composing_request::Method::MoveCaret(MoveCaret {
            direction: wire as i32,
        }),
        "composingMoveCaret",
        generation,
        Some(app_config(settings)),
    )
}

/// Commits the whole composition exactly as the preedit renders it —
/// `Σ nailed.display_text + derived(pending)` under the continuous phase
/// (`transition.rs:443`). This is the literal-commit key.
pub fn commit_raw(settings: &EngineSettings, generation: u64) -> Option<ComposingTransition> {
    dispatch(
        composing_request::Method::CommitRaw(CommitRaw {}),
        "composingCommitRaw",
        generation,
        Some(app_config(settings)),
    )
}

/// Finalizes the composition and appends `text` after it, as one engine
/// step — the space bar and mid-composition punctuation.
pub fn commit_preedit_then_insert_external(
    text: &str,
    settings: &EngineSettings,
    generation: u64,
) -> Option<ComposingTransition> {
    dispatch(
        composing_request::Method::CommitPreeditThenInsertExternal(
            CommitPreeditThenInsertExternal {
                text: text.to_owned(),
            },
        ),
        "composingCommitPreeditThenInsertExternal",
        generation,
        Some(app_config(settings)),
    )
}

/// Abandons the composition without writing anything to the document.
pub fn reset(generation: u64) -> Option<ComposingTransition> {
    dispatch(
        composing_request::Method::Reset(Reset {}),
        "composingReset",
        generation,
        None,
    )
}

/// Reads the candidates for the current continuous composition.
///
/// Read-only, so it must be sent under the composition's EXISTING generation:
/// a bumped generation resets the engine before the query runs
/// (`engine/composing/src/handle.rs:61-66`).
///
/// `now_ms` is the clock the engine's recency ranking reads. The user's own
/// data is not an argument: the engine reads its stores itself and ranks in
/// one call (user-data-engine-roadmap P3b / P5).
pub fn fetch_at_pos(
    settings: &EngineSettings,
    generation: u64,
    now_ms: i64,
) -> Option<ContinuousFetchResult> {
    let fetch = FetchAtPos {
        now_ms,
        // The engine resolves the toggles into its source filter itself.
        toggles: Some(dictionary_toggles(&settings.dictionary_sources)),
        // §34/S22 — positive platform setting → inverted proto disable gate
        // (the field's own comment carries why), so Show Typed Text First ON leaves the
        // preedit literal leading the list and Enter commits what was typed.
        // CROSS-PLATFORM INVARIANT — mirrors
        // `macos/Sources/TaigiInputMethodCore/Engine/RustEngineBridge+Composing.swift`
        // `composingFetchAtPos`, which inverts the same setting onto the same field.
        literal_roman_candidate_disabled: !settings.is_literal_roman_candidate_enabled,
        // The engine reads the user's dictionary only with this setting on.
        custom_dictionary_disabled: !settings.is_custom_dict_enabled,
    };
    let response = composing_response(
        composing_request::Method::FetchAtPos(fetch),
        "composingFetchAtPos",
        generation,
        Some(app_config(settings)),
    )?;
    let candidates = response.continuous.as_ref().map(|continuous| {
        continuous
            .candidates
            .iter()
            .map(ContinuousCandidate::decode)
            .collect()
    });
    Some(ContinuousFetchResult {
        transition: ComposingTransition::decode(&response),
        candidates,
    })
}

/// What a candidate commit round-trips. Every field except `script` comes
/// verbatim from the `ContinuousCandidate` the user picked — in particular
/// `consumed_bytes` is the candidate's `consumed_span_end`, an absolute
/// offset into the pending raw buffer (`composing.proto` `CommitContinuous`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitContinuousArgs<'a> {
    /// Which of the pick's scripts the document gets, relative to the output
    /// settings; the engine resolves the text (`composing::commit_text`).
    pub script: CommitScript,
    /// The pick's display romanization, as the candidate carried it.
    pub roman: &'a str,
    /// The identity keys the engine learns from (Core Principle #7).
    pub canonical_text: &'a str,
    pub association_tl: &'a str,
    /// §50 — the picked candidate's hanji, `None` for a hanji-less pick; the
    /// engine learns a composition only when every segment carried one.
    pub hanji: Option<&'a str>,
    pub consumed_bytes: u32,
    pub syllable_count: u32,
}

/// Commits one candidate returned by `fetch_at_pos`. Consuming the whole
/// pending buffer makes this a final commit; anything less nails the segment
/// and stays continuous, writing nothing (Model B). The engine resolves the
/// document text from `script` and the settings, reports what the commit did,
/// and — with the user data open — counts the pick itself (R5).
pub fn commit_continuous(
    args: &CommitContinuousArgs<'_>,
    settings: &EngineSettings,
    generation: u64,
) -> Option<ContinuousCommitResult> {
    let response = composing_response(
        composing_request::Method::CommitContinuous(CommitContinuous {
            consumed_bytes: args.consumed_bytes,
            syllable_count: args.syllable_count,
            canonical_text: args.canonical_text.to_owned(),
            association_tl: args.association_tl.to_owned(),
            hanji: args.hanji.filter(|h| !h.is_empty()).map(str::to_owned),
            script: args.script as i32,
            roman: args.roman.to_owned(),
        }),
        "composingCommitContinuous",
        generation,
        Some(app_config(settings)),
    )?;
    Some(ContinuousCommitResult {
        transition: ComposingTransition::decode(&response),
        // Always set on this path; a default reads as UNSPECIFIED → ignored.
        commit: response.commit.unwrap_or_default(),
    })
}

fn dispatch(
    method: composing_request::Method,
    op: &str,
    generation: u64,
    config: Option<protos::engine::AppConfig>,
) -> Option<ComposingTransition> {
    composing_response(method, op, generation, config)
        .map(|response| ComposingTransition::decode(&response))
}

fn composing_response(
    method: composing_request::Method,
    op: &str,
    generation: u64,
    config: Option<protos::engine::AppConfig>,
) -> Option<ComposingResponse> {
    let payload = request::Payload::Composing(ComposingRequest {
        method: Some(method),
    });
    match roundtrip(payload, op, generation, config)? {
        response::Payload::Composing(response) => Some(response),
        other => {
            record_failure(op, &format!("expected a composing payload, got {other:?}"));
            None
        }
    }
}
