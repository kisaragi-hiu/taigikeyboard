//! R6 — the TPS layout as a real `input_mode = "tps"` renders byte-identically
//! to the pre-R6 wire, where every platform sent TPS as `"tl"` with the TPS
//! fold already applied (`is_translate_swapped = true`, `hyphenless_roman =
//! false`: iOS `RustEngineBridge+Composing.swift` / Android
//! `RustEngineBridge.kt` `continuousAppConfig`). Under `"tps"` the flags are
//! the stored ones, so every stored combination must match the one legacy
//! wire. Whole responses (preedit, candidates, effects, commit resolution)
//! are compared, over the production lexicon.

use composing::api::Engine;
use composing::dispatch;
use protos::engine::composing_request::Method;
use protos::engine::{
    AppConfig, Append, CandidateMessage, CommitContinuous, CommitScript, ComposingResponse,
    DeleteBackward, EnterContinuous, Start,
};

use crate::common;
use crate::common::{req, Fetch};

/// What a pre-R6 platform sends on the TPS layout.
fn legacy_wire() -> AppConfig {
    AppConfig {
        input_mode: "tl".into(),
        is_translate_swapped: true,
        hyphenless_roman: false,
        ..AppConfig::default()
    }
}

/// The R6 wire: `"tps"` with every stored swap / No Hyphens combination.
fn tps_wires() -> Vec<AppConfig> {
    let mut wires = Vec::new();
    for swapped in [false, true] {
        for hyphenless in [false, true] {
            wires.push(AppConfig {
                input_mode: "tps".into(),
                is_translate_swapped: swapped,
                hyphenless_roman: hyphenless,
                ..AppConfig::default()
            });
        }
    }
    wires
}

fn fetch(engine: &mut Engine, config: &AppConfig) -> ComposingResponse {
    dispatch::apply(Fetch::default().intent(), engine, config)
}

fn send(engine: &mut Engine, method: Method, config: &AppConfig) -> ComposingResponse {
    dispatch::handle(&req(method), engine, config).expect("composing request")
}

fn lead_candidate(response: &ComposingResponse) -> CandidateMessage {
    response
        .continuous
        .as_ref()
        .and_then(|c| c.candidates.first().cloned())
        .expect("a candidate to pick")
}

/// A pick of `candidate`, legacy (`UNSPECIFIED`, the platform-formatted
/// `display_text` is the document text) or R5 (`LEAD`, the engine resolves it).
fn pick(candidate: &CandidateMessage, script: CommitScript) -> Method {
    let canonical = candidate.hanji.clone().unwrap_or(candidate.roman.clone());
    Method::CommitContinuous(CommitContinuous {
        display_text: canonical.clone(),
        canonical_text: canonical,
        association_tl: candidate.canonical_tl.clone(),
        hanji: candidate.hanji.clone(),
        consumed_bytes: candidate.consumed_span_end - candidate.consumed_span_start,
        syllable_count: candidate.syllable_count,
        script: script as i32,
        roman: candidate.roman.clone(),
    })
}

/// Every response of `Start(raw) → EnterContinuous → FetchAtPos`, then — when
/// `nail` names a script — a pick of the lead candidate, a fetch, an `Append`
/// of `appended`, a `DeleteBackward` and a last fetch.
fn session(
    raw: &str,
    nail: Option<(CommitScript, &str)>,
    config: &AppConfig,
) -> Vec<ComposingResponse> {
    let mut engine = Engine::new();
    let mut responses = vec![
        send(
            &mut engine,
            Method::Start(Start { text: raw.into() }),
            config,
        ),
        send(
            &mut engine,
            Method::EnterContinuous(EnterContinuous {}),
            config,
        ),
        fetch(&mut engine, config),
    ];
    if let Some((script, appended)) = nail {
        let candidate = lead_candidate(responses.last().expect("fetch response"));
        responses.push(send(&mut engine, pick(&candidate, script), config));
        responses.push(fetch(&mut engine, config));
        let append = Method::Append(Append {
            char: appended.into(),
        });
        responses.push(send(&mut engine, append, config));
        responses.push(send(
            &mut engine,
            Method::DeleteBackward(DeleteBackward {}),
            config,
        ));
        responses.push(fetch(&mut engine, config));
    }
    responses
}

fn assert_wires_agree(raw: &str, nail: Option<(CommitScript, &str)>) -> Vec<ComposingResponse> {
    let legacy = session(raw, nail, &legacy_wire());
    for wire in tps_wires() {
        let tps = session(raw, nail, &wire);
        assert_eq!(tps, legacy, "{raw:?} {nail:?} under {wire:?}");
    }
    legacy
}

fn hanji(response: &ComposingResponse) -> Vec<String> {
    response
        .continuous
        .as_ref()
        .map(|c| {
            c.candidates
                .iter()
                .filter_map(|c| c.hanji.clone())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn bopomofo_buffers_fetch_the_same_candidates_and_preedit() {
    if !common::production_lexicon_ready() {
        return;
    }
    // trace: ㄉㄞˊㄨㄢˊ = tâi-uân (臺灣 / 台灣), ㄊㄞˊㄨㄢˊ = thâi-uân; the
    // Bopomofo content upgrades both wires to `InputMode::Tps` before any
    // config field is read.
    let responses = assert_wires_agree("ㄉㄞˊㄨㄢˊ", None);
    let hanji = hanji(&responses[2]);
    assert!(
        hanji.iter().any(|h| h == "臺灣" || h == "台灣"),
        "{hanji:?}"
    );
    assert_wires_agree("ㄊㄞˊㄨㄢˊ", None);
}

#[test]
fn a_separator_space_buffer_fetches_the_same_candidates() {
    if !common::production_lexicon_ready() {
        return;
    }
    // §31: the space is the tone-1 marker; `ㄍㄠ ㄉㄞ˪` reaches 交代 on both wires.
    let responses = assert_wires_agree("ㄍㄠ ㄉㄞ˪", None);
    let hanji = hanji(&responses[2]);
    assert!(hanji.iter().any(|h| h == "交代"), "{hanji:?}");
}

#[test]
fn a_buffer_without_bopomofo_composes_under_the_tl_tables_on_both_wires() {
    if !common::production_lexicon_ready() {
        return;
    }
    // A TPS buffer opening with `-` or a lone tone mark has no Bopomofo yet:
    // `composing_mode` reads `"tps"` as TL, exactly what `"tl"` gave.
    for raw in ["-", "--", "ˋ", "˙", "-ˋ", "ˋㄉㄞ"] {
        assert_wires_agree(raw, None);
    }
}

#[test]
fn the_nailed_prefix_after_a_legacy_commit_renders_the_same() {
    if !common::production_lexicon_ready() {
        return;
    }
    // Hanji-first on both wires: the nailed Hanji joins the pending tail with
    // no word space, and the tail's candidates keep the TPS fold.
    let responses = assert_wires_agree("ㄉㄞˊㄨㄢˊㄌㄤˊ", Some((CommitScript::Unspecified, "ㄚ")));
    let nailed = responses[3]
        .preedit
        .as_ref()
        .expect("preedit after the nail");
    assert!(!nailed.display_text.contains(' '), "{nailed:?}");
}

#[test]
fn an_r5_commit_resolves_the_same_document_text() {
    if !common::production_lexicon_ready() {
        return;
    }
    // `commit_text::hanji_leads` reads `renders_hanji_first`: the stored swap
    // under `"tps"`, the folded one under `"tl"` — both lead with the Hanji.
    let responses = assert_wires_agree("ㄉㄞˊㄨㄢˊㄌㄤˊ", Some((CommitScript::Lead, "ㄚ")));
    let resolution = responses[3].commit.as_ref().expect("an R5 resolution");
    assert!(!resolution.wrote_romanization, "{resolution:?}");
}

#[test]
fn negative_control_the_unfolded_tl_wire_renders_differently() {
    if !common::production_lexicon_ready() {
        return;
    }
    // `"tl"` with the stored swap `false` is the one wire the fold exists
    // for: its R5 lead commit writes the romanization, so the comparison
    // above would catch a `"tps"` that lost its Hanji-first reading.
    let nail = Some((CommitScript::Lead, "ㄚ"));
    let unfolded = AppConfig {
        is_translate_swapped: false,
        ..legacy_wire()
    };
    let responses = session("ㄉㄞˊㄨㄢˊㄌㄤˊ", nail, &unfolded);
    assert_ne!(responses, session("ㄉㄞˊㄨㄢˊㄌㄤˊ", nail, &legacy_wire()));
    let resolution = responses[3].commit.as_ref().expect("an R5 resolution");
    assert!(resolution.wrote_romanization, "{resolution:?}");
}
