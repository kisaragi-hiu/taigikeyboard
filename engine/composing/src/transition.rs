//! Pure transition function: `(state, intent, config) → ComposingResponse`.
//! Pure — no logging, no FFI, no platform types. The dispatcher in
//! `requests.rs` runs this and returns the response to callers.
//!
//! Effect ordering matters; iOS/Android downstream wrappers consume effects
//! in proto-list order. `prost` preserves order on `repeated Effect` fields.
//!
//! `Phase::Continuous` (v3.5.8) follows **Model B** (mainstream-aligned —
//! librime / khiin-rs / MOE / azooKey; see
//! `docs/engine/continuous-commit-and-display.md` §10): nailed segments are **NOT**
//! in the host document. The whole composition — `Σ nailed[i].display_text`
//! followed by the derived display of the pending `raw` tail — occupies a
//! single marked / preedit region until a hard finalize (Enter / final-commit
//! / external-suggestion commit), which writes the whole composition to the
//! document in one `CommitTextReplacingPreedit`. A candidate tap *nails* a
//! segment inside the composition (no document write). `ComposingResponse.
//! preedit` therefore carries the **whole composition** during Continuous;
//! tests inspect `nailed` via `Engine::snapshot_state`.

use crate::api::{
    combined_display, combined_display_with_tail, nailed_prefix, Applied, CaretDirection,
    CommitScript, EngineState, Intent, NailedSegment, Phase, Usage,
};
use crate::commit_text::{commit_resolution, resolve_commit_text};
use crate::derived::{
    buffer_input_mode, derived_display, display_caret_utf16, strip_tps_separator_markers,
};
use lexicon::LearnedEntry;
use protos::engine::composing_response::Preedit;
use protos::engine::effect;
use protos::engine::AppConfig;
use protos::engine::{
    ClearCandidates, ClearPreeditWithoutCommit, CommitOutcome, CommitTextReplacingPreedit,
    CommittedWord, ComposingResponse, Effect, NextWordClearForNewComposing,
    NextWordUpdateLastSelectedWord, NextWordWordSelected, RefreshCandidates, ResetCandidateContext,
    UpdatePreedit,
};

/// Learned phrases (§50) — the longest composition the final commit turns
/// into one learned `(Hanji, canonical-TL)` pair, in syllables. ChiaKey caps
/// its in-buffer word capture at 6 characters; a Taigi phrase past six
/// syllables is a clause, not a word.
const MAX_LEARNED_PHRASE_SYLLABLES: usize = 6;

/// Apply `intent` against `state`, mutate, return the proto response and
/// the phrase a final commit taught (§50).
pub(crate) fn apply(state: &mut EngineState, intent: Intent, config: &AppConfig) -> Applied {
    let response = match intent {
        Intent::Start { text } => match &state.phase {
            Phase::Continuous { .. } => start_under_continuous(state, text, config),
            // §21: a leading `--` neutral-tone marker typed from Idle is a document
            // literal, not composing input (see helper).
            Phase::Idle => begin_composition_or_insert_leading_hyphens(state, text, config),
        },
        Intent::Append { ch } => match &state.phase {
            Phase::Idle => begin_composition_or_insert_leading_hyphens(state, ch, config),
            Phase::Continuous { raw, caret, nailed } => {
                append_continuous(state, raw.clone(), *caret, nailed.clone(), &ch, config)
            }
        },
        Intent::AppendHyphen => {
            return apply(
                state,
                Intent::Append {
                    ch: "-".to_string(),
                },
                config,
            )
        }
        Intent::ReplaceLast { replacement } => replace_last(state, replacement, config),
        Intent::DeleteBackward => delete_backward(state, config),
        Intent::CommitRaw => commit_raw(state, config),
        Intent::SelectCandidate { text } => match &state.phase {
            Phase::Continuous { nailed, .. } => {
                select_candidate_under_continuous(state, nailed.clone(), text, config)
            }
            Phase::Idle => noop(state, config),
        },
        Intent::CommitPreeditThenInsertExternal { text } => match &state.phase {
            Phase::Continuous { raw, nailed, .. } => {
                commit_preedit_then_insert_external_under_continuous(
                    state,
                    raw.clone(),
                    nailed.clone(),
                    text,
                    config,
                )
            }
            Phase::Idle => insert_external_when_idle(state, text, config),
        },
        Intent::Reset => reset(state, config),
        // FetchAtPos is a read-only query that needs lexicon state; the
        // dispatcher short-circuits before reaching `apply`. Reaching
        // here means a caller bypassed dispatch (test path or future
        // refactor) — return a snapshot rather than panic so the
        // invariant "transition is total" holds (Codex post-impl
        // continuous_phase findings #2/#3 pattern).
        Intent::FetchAtPos { .. } => snapshot(state, config),
        Intent::CommitContinuous {
            canonical_text,
            association_tl,
            hanji,
            consumed_bytes,
            syllable_count,
            script,
            roman,
        } => {
            let pick = SegmentPick {
                canonical_text,
                association_tl,
                hanji,
                consumed_bytes,
                syllable_count,
            };
            return commit_continuous(state, pick, script, &roman, config);
        }
        Intent::TelexKey { key } => telex_key(state, &key, config),
        Intent::TpsKey { key } => tps_key(state, &key, config),
        Intent::MoveCaret { direction } => move_caret(state, direction, config),
    };
    response.into()
}

/// `Intent::TelexKey` — edit the chunk before the caret through
/// `telex::apply_telex_key` (the key acts on the syllable being typed, which
/// is whatever ends that chunk) and keep the rest of the pending tail; a
/// `None` edit is a no-op. Under Continuous the nailed segments stay
/// untouched and the preedit re-renders the whole composition; selection
/// resets as a fresh typing step does.
fn telex_key(state: &mut EngineState, key: &str, config: &AppConfig) -> ComposingResponse {
    let mode = phonetics::api::composing_mode(config);
    match &state.phase {
        Phase::Idle => match crate::telex::apply_telex_key("", key, mode) {
            Some(text) => begin_composition(state, text, config),
            None => noop(state, config),
        },
        Phase::Continuous { raw, caret, nailed } => {
            match telex_before_caret(raw, *caret, key, mode) {
                Some((next, caret)) => step_continuous(state, next, caret, nailed.clone(), config),
                None => noop(state, config),
            }
        }
    }
}

/// `Intent::MoveCaret` — step the caret one char inside the pending tail.
/// The buffer is untouched, so the answer is the snapshot plus one
/// `UpdatePreedit` carrying the new caret and nothing else: no
/// `RefreshCandidates`, so candidates, highlight and page stay. At an
/// edge (the caret never enters a nailed segment) it is a plain snapshot.
fn move_caret(
    state: &mut EngineState,
    direction: Option<CaretDirection>,
    config: &AppConfig,
) -> ComposingResponse {
    let moved = match &mut state.phase {
        Phase::Idle => None,
        Phase::Continuous { raw, caret, .. } => direction
            .and_then(|direction| step_caret(raw, *caret, direction))
            .map(|next| *caret = next),
    };
    let mut resp = snapshot(state, config);
    if moved.is_some() {
        if let Some(preedit) = &resp.preedit {
            resp.effect.push(update_preedit(preedit));
        }
    }
    resp
}

// ---- Helpers ------------------------------------------------------

// Pending-tail edits around the caret. `caret` is a char boundary in
// `0..=raw.len()`; each returns the new buffer and the new caret.

fn insert_at_caret(raw: &str, caret: usize, text: &str) -> (String, usize) {
    let mut next = String::with_capacity(raw.len() + text.len());
    next.push_str(&raw[..caret]);
    next.push_str(text);
    next.push_str(&raw[caret..]);
    (next, caret + text.len())
}

/// `None` when nothing precedes the caret.
fn delete_before_caret(raw: &str, caret: usize) -> Option<(String, usize)> {
    let removed = raw[..caret].chars().next_back()?;
    let start = caret - removed.len_utf8();
    let mut next = String::with_capacity(raw.len());
    next.push_str(&raw[..start]);
    next.push_str(&raw[caret..]);
    Some((next, start))
}

fn replace_before_caret(raw: &str, caret: usize, replacement: &str) -> Option<(String, usize)> {
    let (next, caret) = delete_before_caret(raw, caret)?;
    Some(insert_at_caret(&next, caret, replacement))
}

/// Telex key on the chunk before the caret; the tail after it rides along.
fn telex_before_caret(
    raw: &str,
    caret: usize,
    key: &str,
    mode: phonetics::api::InputMode,
) -> Option<(String, usize)> {
    let prefix = crate::telex::apply_telex_key(&raw[..caret], key, mode)?;
    let next_caret = prefix.len();
    let mut next = prefix;
    next.push_str(&raw[caret..]);
    Some((next, next_caret))
}

/// The TPS Space key's marker in the raw buffer (§31).
const TPS_SEPARATOR: &str = " ";

/// TPS key on the chunk before the caret; the tail after it rides along.
/// `None` when the key changes nothing: an empty key, or a separator where
/// nothing precedes the caret or where a tone mark or a separator sits on
/// either side of it (the syllable is already closed, or the separator would
/// part a syllable from its own tone mark).
fn tps_key_before_caret(raw: &str, caret: usize, key: &str) -> Option<(String, usize)> {
    let prefix = &raw[..caret];
    if key == TPS_SEPARATOR {
        let closes_syllable = |c: char| c == ' ' || phonetics::is_tps_tone_mark(c);
        if closes_syllable(prefix.chars().next_back()?) {
            return None;
        }
        if raw[caret..].chars().next().is_some_and(closes_syllable) {
            return None;
        }
        return Some(insert_at_caret(raw, caret, key));
    }
    if key.is_empty() {
        return None;
    }
    let (adjusted, replace_last) = phonetics::tps_input_adjust(key, prefix);
    match replace_last {
        Some(replacement) => {
            // The adjuster replaces the character it read, so one precedes the caret.
            let (raw, caret) = replace_before_caret(raw, caret, &replacement)?;
            Some(insert_at_caret(&raw, caret, &adjusted))
        }
        None => Some(insert_at_caret(raw, caret, &adjusted)),
    }
}

/// `None` at the edge the step would cross.
fn step_caret(raw: &str, caret: usize, direction: CaretDirection) -> Option<usize> {
    match direction {
        CaretDirection::Left => raw[..caret]
            .chars()
            .next_back()
            .map(|c| caret - c.len_utf8()),
        CaretDirection::Right => raw[caret..].chars().next().map(|c| caret + c.len_utf8()),
    }
}

/// Build a mid-composition step response (typing / replace-last /
/// delete-backward non-empty branches): update the preedit + request a fresh
/// autocomplete query against the new buffer. `display` is the **whole
/// composition** (`Σ nailed.display_text` + pending-tail derived form —
/// callers build it via [`combined_display`], Model B) while `raw` stays the
/// still-editable pending tail.
fn step_response(preedit: Preedit) -> ComposingResponse {
    let effects = vec![update_preedit(&preedit), refresh_candidates()];
    ComposingResponse {
        preedit: Some(preedit),
        effect: effects,
        is_composing: true,
        continuous: None,
        commit: None,
    }
}

/// The wire form of a live composition: the pending `raw`, the whole
/// `display` (nailed prefix + pending tail under Continuous, whose derived
/// form starts at byte `tail_start`) and the caret projected into it — the
/// prefix's UTF-16 length plus the caret's offset inside the tail
/// ([`display_caret_utf16`]).
fn composition_preedit(raw: String, caret: usize, display: String, tail_start: usize) -> Preedit {
    let (prefix, tail) = display.split_at(tail_start);
    let caret_utf16 = prefix.encode_utf16().count() + display_caret_utf16(&raw, tail, caret);
    Preedit {
        raw_input: raw,
        display_text: display,
        caret_utf16: caret_utf16 as u32,
    }
}

/// Begin a composition from Idle: the buffer becomes `raw` with nothing
/// nailed and the caret at its end. Empty `raw` stays Idle (a Continuous
/// phase is never empty in both `raw` and `nailed`).
fn begin_composition(
    state: &mut EngineState,
    raw: String,
    config: &AppConfig,
) -> ComposingResponse {
    if raw.is_empty() {
        return noop(state, config);
    }
    let caret = raw.len();
    step_continuous(state, raw, caret, Vec::new(), config)
}

/// One `Phase::Continuous` step on the pending tail: `nailed` is untouched,
/// the whole composition re-renders (Model B) and a fresh fetch is requested.
fn step_continuous(
    state: &mut EngineState,
    pending: String,
    caret: usize,
    nailed: Vec<NailedSegment>,
    config: &AppConfig,
) -> ComposingResponse {
    let (combined, tail_start) = combined_display_with_tail(&nailed, &pending, config);
    state.phase = Phase::Continuous {
        raw: pending.clone(),
        caret,
        nailed,
    };
    step_response(composition_preedit(pending, caret, combined, tail_start))
}

/// §21 INVARIANT_KHINSIANN_LEADING_MARKER_LITERAL — a leading ASCII-hyphen run
/// typed from `Phase::Idle` (no syllable content yet) is the neutral-tone (khinsiann)
/// marker `--` (e.g. `--ah` 矣). It is a **document literal**, not composing
/// input: insert the run verbatim and — if a syllable remainder follows in the
/// same text — begin a composition with the remainder. The underlined preedit then
/// covers only the convertible syllable, matching the candidate strip and the
/// reference IME (MOE).
///
/// Internal hyphens (typed AFTER syllable content, e.g. the hyphen in `tai-bak`)
/// never reach this fn — the buffer is already `Phase::Continuous`, so they stay
/// composing-boundary delimiters via the `Append` Continuous arm.
///
/// Production keystrokes arrive one char at a time, so the common case is
/// `text == "-"` (remainder empty → pure literal insert, stay Idle). The
/// split also covers a multi-char `Start { text: "--ah" }` from the engine API
/// / tests so no old-model entry survives.
fn begin_composition_or_insert_leading_hyphens(
    state: &mut EngineState,
    text: String,
    config: &AppConfig,
) -> ComposingResponse {
    let hyphen_len = text.bytes().take_while(|&b| b == b'-').count();
    if hyphen_len == 0 {
        return begin_composition(state, text, config);
    }
    let (run, remainder) = text.split_at(hyphen_len);
    if remainder.is_empty() {
        // All hyphens — insert the literal run, stay Idle (no preedit).
        return exit_to_idle(state, vec![commit_text_replacing_preedit(run.to_string())]);
    }
    // Leading run + syllable remainder (multi-char `Start` / engine-API /
    // test path ONLY — the platform sends one char per keystroke, so a
    // production leading `-` always arrives as `Start{"-"}` with an empty
    // remainder above). Insert the literal run first, then compose the
    // remainder. This emits a mixed commit+composing transition; callers MUST
    // feed leading-hyphen input char-by-char, not as one multi-char `Start`:
    // a mixed transition can desync Android's `onUpdateSelection` clear-hook
    // (commit fires before the preedit lands). Engine/proto level is correct
    // and tested; the contract is char-by-char at the platform boundary.
    let mut resp = begin_composition(state, remainder.to_string(), config);
    resp.effect
        .insert(0, commit_text_replacing_preedit(run.to_string()));
    resp
}

/// `Intent::TpsKey` — see the `TpsKey` proto comment. From Idle a glyph begins
/// the composition as `Append` does (a leading hyphen stays a document
/// literal, §21); under Continuous the nailed segments stay untouched.
fn tps_key(state: &mut EngineState, key: &str, config: &AppConfig) -> ComposingResponse {
    match &state.phase {
        Phase::Idle => match tps_key_before_caret("", 0, key) {
            Some((text, _)) => begin_composition_or_insert_leading_hyphens(state, text, config),
            None => noop(state, config),
        },
        Phase::Continuous { raw, caret, nailed } => match tps_key_before_caret(raw, *caret, key) {
            Some((next, caret)) => step_continuous(state, next, caret, nailed.clone(), config),
            None => noop(state, config),
        },
    }
}

/// TPS auto-correct. Under `Phase::Continuous` it edits the
/// pending tail only — nailed segments are untouched — but the preedit
/// re-renders the whole composition (Model B).
fn replace_last(
    state: &mut EngineState,
    replacement: String,
    config: &AppConfig,
) -> ComposingResponse {
    match &state.phase {
        Phase::Continuous { raw, caret, nailed } => {
            let Some((new_pending, caret)) = replace_before_caret(raw, *caret, &replacement) else {
                return noop(state, config);
            };
            // Empty pending + empty nailed = degenerate Continuous state
            // (Codex post-impl finding #2). Exit to Idle and clear nextword.
            if new_pending.is_empty() && nailed.is_empty() {
                return exit_to_idle(state, abort_continuous_effects());
            }
            step_continuous(state, new_pending, caret, nailed.clone(), config)
        }
        Phase::Idle => noop(state, config),
    }
}

fn delete_backward(state: &mut EngineState, config: &AppConfig) -> ComposingResponse {
    match &state.phase {
        Phase::Continuous { raw, caret, nailed } => {
            delete_backward_continuous(state, raw.clone(), *caret, nailed.clone(), config)
        }
        Phase::Idle => noop(state, config),
    }
}

/// `DeleteBackward` under `Phase::Continuous` — **Model B** (Codex risk (v)).
/// Nailed segments are **not** in the document, so backspace never emits
/// `DeleteBackwardFromDocument`: it only re-shapes the single marked region.
/// Three branches:
///   1. pending non-empty → drop the char before the caret (nothing before
///      it → no-op, the caret does not fall through into a nailed segment);
///      if pending now empty
///      AND nailed is also empty, exit to Idle and clear the marked region
///      (no document char is touched — the char only ever lived in the
///      marked region); else stay Continuous and re-render the combined
///      composition.
///   2. pending empty AND nailed non-empty → **unnail** the last segment:
///      pop it, restore its `raw_text` as the new pending tail, roll back
///      NextWord's last-selected, and re-render the combined composition.
///      Authority is `raw_text` (never display-character count — swap / TPS
///      / both-scripts display can desync from raw).
///   3. pending empty AND nailed empty → exit to Idle (degenerate case;
///      shouldn't occur in steady state but guarded).
fn delete_backward_continuous(
    state: &mut EngineState,
    pending: String,
    caret: usize,
    nailed: Vec<NailedSegment>,
    config: &AppConfig,
) -> ComposingResponse {
    if !pending.is_empty() {
        let Some((new_pending, caret)) = delete_before_caret(&pending, caret) else {
            return noop(state, config);
        };
        if new_pending.is_empty() && nailed.is_empty() {
            return exit_to_idle(state, abort_continuous_effects());
        }
        return step_continuous(state, new_pending, caret, nailed, config);
    }

    // pending empty branches
    if nailed.is_empty() {
        return exit_to_idle(state, abort_continuous_effects());
    }

    let mut new_nailed = nailed;
    // JUSTIFICATION: `nailed.is_empty()` was checked at the branch above;
    // popping a non-empty Vec is a programmer-invariant guarantee, not a
    // data path.
    let popped = new_nailed.pop().expect("nailed non-empty checked above");
    // Model B: the popped segment was never in the document — unnailing it
    // just restores its raw text as the editable pending tail. No
    // `DeleteBackwardFromDocument`; the combined preedit re-render replaces
    // the marked region. Authority is `raw_text`, never a display-char
    // count (swap / TPS / both-scripts display can desync from raw).
    let new_pending = popped.raw_text;
    // Unnail handshake. NextWord learns nothing from it and keeps its
    // committed context (behavioral-invariants §40) — the popped segment can
    // no longer reach NextWord at all, because only the final commit's
    // `preceding` carries nailed segments. Kept for platforms that read the
    // effect stream; canonical key per v3.5.8 Phase 9 Bug 1 (Option A).
    let nextword_correction = match new_nailed.last() {
        Some(prev) => next_word_update_last_selected_word(
            prev.canonical_text.clone(),
            // R2: canonical TL (raw-slice fallback), as the commit path.
            association_roman(&prev.association_tl, &prev.raw_text),
        ),
        None => next_word_clear_for_new_composing(),
    };
    let (combined, tail_start) = combined_display_with_tail(&new_nailed, &new_pending, config);
    let caret = new_pending.len();
    state.phase = Phase::Continuous {
        raw: new_pending.clone(),
        caret,
        nailed: new_nailed,
    };

    let preedit = composition_preedit(new_pending, caret, combined, tail_start);
    let effects = vec![
        nextword_correction,
        update_preedit(&preedit),
        refresh_candidates(),
    ];
    ComposingResponse {
        preedit: Some(preedit),
        effect: effects,
        is_composing: true,
        continuous: None,
        commit: None,
    }
}

fn commit_raw(state: &mut EngineState, config: &AppConfig) -> ComposingResponse {
    match &state.phase {
        Phase::Continuous { raw, nailed, .. } => {
            commit_raw_continuous(state, raw.clone(), nailed.clone(), config)
        }
        Phase::Idle => noop(state, config),
    }
}

/// `Intent::CommitRaw` under `Phase::Continuous` (Enter) — v3.5.8 Phase 9
/// Item 3, **Model B (§10)**. Enter commits the **whole composition** —
/// `Σ nailed[i].display_text` + the derived display of the pending `raw`
/// tail — in one `CommitTextReplacingPreedit`, because under Model B the
/// nailed segments were never written to the document (they lived in the
/// marked region). This replaces the single marked region with the literal
/// finalized string. The per-nailed-segment NextWord associations already
/// fired as `NextWordUpdateLastSelectedWord` at nail time; this emits the
/// **single terminal** `NextWordWordSelected` for the last "word": the
/// pending tail when one exists, else the last nailed segment.
///
/// See `docs/engine/continuous-commit-and-display.md` §10.3 commit contract
/// (Enter commits the whole composition) and §10.7 "Enter after segments
/// already nailed" row.
fn commit_raw_continuous(
    state: &mut EngineState,
    raw: String,
    nailed: Vec<NailedSegment>,
    config: &AppConfig,
) -> ComposingResponse {
    let combined = combined_display(&nailed, &raw, config);
    // Defensive guard: empty composition (no nailed, empty raw) violates the
    // "Continuous is non-empty in at least one of pending / nailed"
    // invariant and is unreachable under normal flow. noop, don't panic.
    if combined.is_empty() {
        return noop(state, config);
    }
    // Single terminal NextWord word-selection for the last "word": the
    // pending tail when it exists (roman word, key = its derived form), else
    // the last nailed segment (canonical key — keeps association learning
    // mode-independent, v3.5.8 Phase 9 Bug 1 Option A / decision b). Every
    // nailed segment before it rides along as `preceding` — the only place
    // NextWord learns a nailed segment (behavioral-invariants §40).
    let terminal_nextword = if !raw.is_empty() {
        // §41 — both fields drop the separator marker. `text` goes through
        // `derived_display`; `roman` is the raw tail, which for TPS still
        // carries the marker, and that string becomes the association's
        // romanization key on both platforms. Learning `ㄍㄠ␣ㄉㄞ` where the
        // committed word is `ㄍㄠㄉㄞ` would key the row on a form no later
        // lookup reconstructs (Codex post-impl BLOCK 2026-08-21).
        // NextWord learns `roman` as sent, so the tail is put in canonical
        // TL form here: POJ folds to TL, TL keeps its `eng` / `ek` finals,
        // TPS and English pass through.
        let tail_display = derived_display(&raw, config);
        let tail_roman = phonetics::api::canonical_tl_form(
            &strip_tps_separator_markers(&raw),
            buffer_input_mode(&raw, config),
        );
        next_word_word_selected(tail_display, tail_roman, true, &nailed)
    } else {
        // raw empty → all input is nailed; the last nailed segment is the
        // final word. `nailed` is non-empty here (combined non-empty with
        // empty raw implies a nailed segment exists).
        match nailed.split_last() {
            Some((last, preceding)) => next_word_word_selected(
                last.canonical_text.clone(),
                // R2: this segment had a candidate selected at nail time →
                // use its canonical TL (raw-slice fallback). The pending-tail
                // branch above stays raw — there is no candidate there.
                association_roman(&last.association_tl, &last.raw_text),
                true,
                preceding,
            ),
            None => next_word_clear_for_new_composing(),
        }
    };
    let mut effects = finalize_effects(combined);
    effects.push(terminal_nextword);
    exit_to_idle(state, effects)
}

/// `Intent::CommitPreeditThenInsertExternal` from Idle — a plain insert
/// (emoji / paste with nothing composed). `CommitTextReplacingPreedit` is
/// no-op-on-empty-preedit safe on every platform.
fn insert_external_when_idle(
    state: &mut EngineState,
    external: String,
    config: &AppConfig,
) -> ComposingResponse {
    if external.is_empty() {
        return noop(state, config);
    }
    exit_to_idle(state, vec![commit_text_replacing_preedit(external)])
}

/// User-initiated reset — **Model B
/// (Codex risk (ii))**. Nailed segments were never written to the document;
/// the whole composition lived in one marked region, so the abort trio
/// clears that **entire** region and dropping the state discards every
/// nailed segment. Nothing reaches the document. Idle → no-op.
fn reset(state: &mut EngineState, config: &AppConfig) -> ComposingResponse {
    match state.phase {
        Phase::Idle => noop(state, config),
        Phase::Continuous { .. } => exit_to_idle(state, abort_continuous_effects()),
    }
}

pub(crate) fn snapshot(state: &EngineState, config: &AppConfig) -> ComposingResponse {
    let (preedit, is_composing) = match &state.phase {
        Phase::Idle => (Preedit::default(), false),
        // Model B: the composing-buffer surface is the whole composition
        // (Σ nailed.display_text + pending-tail derived form), not the
        // pending tail alone. `raw_input` stays the still-editable tail.
        Phase::Continuous { raw, caret, nailed } => {
            let (combined, tail_start) = combined_display_with_tail(nailed, raw, config);
            (
                composition_preedit(raw.clone(), *caret, combined, tail_start),
                true,
            )
        }
    };
    ComposingResponse {
        preedit: Some(preedit),
        effect: Vec::new(),
        is_composing,
        continuous: None,
        commit: None,
    }
}

fn exit_to_idle(state: &mut EngineState, effects: Vec<Effect>) -> ComposingResponse {
    state.phase = Phase::Idle;
    ComposingResponse {
        preedit: Some(Preedit::default()),
        effect: effects,
        is_composing: false,
        continuous: None,
        commit: None,
    }
}

fn noop(state: &EngineState, config: &AppConfig) -> ComposingResponse {
    snapshot(state, config)
}

// ---- Continuous-phase helpers --------------------------------------

/// `Intent::Start { text }` arriving while in `Phase::Continuous`. Codex
/// post-impl finding #3: silently dropping `text` would lose user input.
/// Treats the intent as "drop continuous state, then begin a fresh
/// composition with `text`" (no §21 leading-hyphen split here). Emits the
/// abort trio followed by the regular step effects; final state is
/// `Phase::Continuous { raw: text, nailed: [] }` (or Idle if `text` is empty).
fn start_under_continuous(
    state: &mut EngineState,
    text: String,
    config: &AppConfig,
) -> ComposingResponse {
    // Drop continuous state to Idle first.
    state.phase = Phase::Idle;
    let mut effects = abort_continuous_effects();
    let resp = begin_composition(state, text, config);
    effects.extend(resp.effect);
    ComposingResponse {
        effect: effects,
        ..resp
    }
}

/// `Intent::SelectCandidate { text }` arriving while in `Phase::Continuous`.
/// Codex post-impl finding #3: silently dropping `text` would lose user
/// selection. **Model B**: the nailed prefix is in the marked region (not
/// the document), so committing `text` alone would lose it. Commit the
/// whole composition with the pending tail replaced by `text` —
/// `Σ nailed[i].display_text + text` — in one `CommitTextReplacingPreedit`,
/// preserving the net-document parity the pre-Model-B behavior had
/// (nailed-in-doc + text). Then exit Continuous.
fn select_candidate_under_continuous(
    state: &mut EngineState,
    nailed: Vec<NailedSegment>,
    text: String,
    config: &AppConfig,
) -> ComposingResponse {
    if text.is_empty() {
        // Empty suggestion: drop continuous state without inserting. Mirrors
        // SelectCandidate-on-Idle being a no-op.
        return reset(state, config);
    }
    let mut combined = nailed_prefix(&nailed, config);
    combined.push_str(&text);
    let mut effects = finalize_effects(combined);
    effects.push(next_word_clear_for_new_composing());
    exit_to_idle(state, effects)
}

/// `Intent::CommitPreeditThenInsertExternal { text }` under Continuous.
/// Codex post-impl finding #3. **Model B**: the whole composition
/// (`Σ nailed[i].display_text` + pending derived display) plus the external
/// text are committed in one `CommitTextReplacingPreedit` — nailed segments
/// were never in the document, so they must ride the commit here too. Then
/// exits Continuous. Empty `text` collapses to `noop`.
fn commit_preedit_then_insert_external_under_continuous(
    state: &mut EngineState,
    raw: String,
    nailed: Vec<NailedSegment>,
    external: String,
    config: &AppConfig,
) -> ComposingResponse {
    if external.is_empty() {
        return noop(state, config);
    }
    let mut combined = combined_display(&nailed, &raw, config);
    combined.push_str(&external);
    let mut effects = finalize_effects(combined);
    effects.push(next_word_clear_for_new_composing());
    exit_to_idle(state, effects)
}

/// `Phase::Continuous` segment commit of the resolved `display_text`.
/// `consumed_bytes >= pending.len()` is the final-commit branch (exit to
/// Idle); otherwise mid-commit (stay in Continuous). Programmer-error inputs
/// (out-of-range or non-char-boundary `consumed_bytes`) collapse to `noop`
/// rather than panicking.
// Under **Model B (§10)** `display_text` is the segment's text **inside the
// marked region**, not yet in the document. `canonical_text` is the
// canonical dictionary key (`hanji.unwrap_or(roman)`) used for NextWord
// association so learning stays mode-independent (user decision b). Model B:
// a mid-commit emits NO `CommitTextReplacingPreedit` — it only re-renders
// the combined marked region; the single literal document write happens at
// final-commit (whole composition) or via Enter (`commit_raw_continuous`).
// Returns what the commit did beside the response.
fn nail_segment(
    state: &mut EngineState,
    display_text: String,
    pick: SegmentPick,
    config: &AppConfig,
) -> (Applied, CommitOutcome) {
    let SegmentPick {
        canonical_text: canonical,
        association_tl,
        hanji,
        consumed_bytes,
        syllable_count,
    } = pick;
    let Phase::Continuous { raw, nailed, .. } = &state.phase else {
        return (noop(state, config).into(), CommitOutcome::Ignored);
    };
    if display_text.is_empty()
        || consumed_bytes == 0
        || consumed_bytes > raw.len()
        || !raw.is_char_boundary(consumed_bytes)
    {
        return (noop(state, config).into(), CommitOutcome::Ignored);
    }
    let pending = raw.clone();
    let mut new_nailed = nailed.clone();
    let raw_text = pending[..consumed_bytes].to_string();
    let prev_end = new_nailed.last().map(|s| s.raw_span.1).unwrap_or(0);
    let raw_span = (prev_end, prev_end + consumed_bytes);
    let new_pending = pending[consumed_bytes..].to_string();
    // R2: the NextWord `roman` arg — canonical TL when the platform sent
    // it, else the raw committed slice (legacy / TPS-OOV fallback). Used
    // for whichever single effect this commit fires below (final OR mid).
    let next_word_roman = association_roman(&association_tl, &raw_text);
    let segment = NailedSegment {
        display_text: display_text.clone(),
        canonical_text: canonical.clone(),
        raw_text: raw_text.clone(),
        association_tl,
        hanji: hanji.filter(|h| !h.is_empty()),
        raw_span,
        syllable_count,
    };
    new_nailed.push(segment);

    if new_pending.is_empty() {
        // Final commit (Model B): the whole composition was in the marked
        // region; write all nailed segments' display text to the document
        // in one go, then exit to Idle. The single terminal WordSelected is
        // the final segment, with the earlier nailed segments as `preceding`
        // (behavioral-invariants §40).
        // Pending is empty here, so the whole composition is just the
        // nailed prefix (combined_display would append derived("") = "").
        let combined = nailed_prefix(&new_nailed, config);
        let mut effects = finalize_effects(combined);
        // `nailed` = every segment before the one just pushed.
        effects.push(next_word_word_selected(
            canonical,
            next_word_roman,
            true,
            nailed,
        ));
        // Learned phrases (§50): the whole composition, if it was a
        // sequence of hanji picks, becomes one learned pair.
        let learned = learned_phrase(&new_nailed);
        let applied = Applied {
            response: exit_to_idle(state, effects),
            learned,
            usage: None,
        };
        return (applied, CommitOutcome::Finalized);
    }

    // Mid-commit (Model B): stay in Continuous, NO document write — just
    // re-render the combined marked region (nailed prefix + new pending).
    // Accepting a candidate is a flush of what was typed, so the caret goes
    // to the end of the tail that is left.
    let (combined, tail_start) = combined_display_with_tail(&new_nailed, &new_pending, config);
    let caret = new_pending.len();
    state.phase = Phase::Continuous {
        raw: new_pending.clone(),
        caret,
        nailed: new_nailed,
    };
    let preedit = composition_preedit(new_pending, caret, combined, tail_start);
    let effects = vec![
        update_preedit(&preedit),
        next_word_update_last_selected_word(canonical, next_word_roman),
        refresh_candidates(),
    ];
    let response = ComposingResponse {
        preedit: Some(preedit),
        effect: effects,
        is_composing: true,
        continuous: None,
        commit: None,
    };
    (response.into(), CommitOutcome::Nailed)
}

/// The pick a `CommitContinuous` names, apart from what it writes.
struct SegmentPick {
    canonical_text: String,
    association_tl: String,
    hanji: Option<String>,
    consumed_bytes: usize,
    syllable_count: u8,
}

/// `Intent::CommitContinuous` (R5): [`nail_segment`] with the document text
/// the engine resolves ([`resolve_commit_text`]), answering
/// `ComposingResponse.commit` and, for a pick that nailed or finalized, its
/// usage (`Applied.usage`). A pick with no script, no canonical text, or a
/// script it does not have, is ignored: no state change, no usage.
fn commit_continuous(
    state: &mut EngineState,
    pick: SegmentPick,
    script: Option<CommitScript>,
    roman: &str,
    config: &AppConfig,
) -> Applied {
    let hanji = pick.hanji.clone().filter(|hanji| !hanji.is_empty());
    let resolved = script
        .filter(|_| !pick.canonical_text.is_empty())
        .and_then(|script| resolve_commit_text(script, roman, hanji.as_deref(), config));
    let Some(resolved) = resolved else {
        let mut response = noop(state, config);
        response.commit = Some(commit_resolution(CommitOutcome::Ignored, None));
        return response.into();
    };
    let usage = Usage {
        display_text: pick.canonical_text.clone(),
        canonical_tl: pick.association_tl.clone(),
        hanji,
    };
    let (mut applied, outcome) = nail_segment(state, resolved.text.clone(), pick, config);
    if outcome != CommitOutcome::Ignored {
        applied.usage = Some(usage);
    }
    applied.response.commit = Some(commit_resolution(outcome, Some(resolved)));
    applied
}

/// `Append { ch }` under `Phase::Continuous`. Appends to the pending tail;
/// nailed segments are untouched. The preedit re-renders the **whole
/// composition** (Model B). Empty `ch` collapses to noop.
fn append_continuous(
    state: &mut EngineState,
    pending: String,
    caret: usize,
    nailed: Vec<NailedSegment>,
    ch: &str,
    config: &AppConfig,
) -> ComposingResponse {
    if ch.is_empty() {
        return noop(state, config);
    }
    let (new_pending, caret) = insert_at_caret(&pending, caret, ch);
    step_continuous(state, new_pending, caret, nailed, config)
}

// ---- Effect constructors ------------------------------------------

fn update_preedit(preedit: &Preedit) -> Effect {
    Effect {
        kind: Some(effect::Kind::UpdatePreedit(UpdatePreedit {
            display: preedit.display_text.clone(),
            caret_utf16: preedit.caret_utf16,
        })),
    }
}

fn clear_preedit_without_commit() -> Effect {
    Effect {
        kind: Some(effect::Kind::ClearPreeditWithoutCommit(
            ClearPreeditWithoutCommit {},
        )),
    }
}

fn commit_text_replacing_preedit(text: String) -> Effect {
    Effect {
        kind: Some(effect::Kind::CommitTextReplacingPreedit(
            CommitTextReplacingPreedit { text },
        )),
    }
}

fn clear_candidates() -> Effect {
    Effect {
        kind: Some(effect::Kind::ClearCandidates(ClearCandidates {})),
    }
}

fn refresh_candidates() -> Effect {
    Effect {
        kind: Some(effect::Kind::RefreshCandidates(RefreshCandidates {})),
    }
}

fn reset_candidate_context() -> Effect {
    Effect {
        kind: Some(effect::Kind::ResetCandidateContext(
            ResetCandidateContext {},
        )),
    }
}

/// The abort trio every "drop Continuous state without writing to the
/// document" branch emits, in this order: clear the whole marked region,
/// reset autocomplete, tear down NextWord's continuous strip. A caller that
/// emits more (`start_under_continuous`) appends after the trio.
fn abort_continuous_effects() -> Vec<Effect> {
    vec![
        clear_preedit_without_commit(),
        clear_candidates(),
        next_word_clear_for_new_composing(),
    ]
}

/// The finalize trio every branch that commits text out of a composition
/// emits, in this order:
/// commit `text` replacing the preedit, reset autocomplete, reset the
/// autocomplete context. A caller that also fires a NextWord effect pushes
/// it after the trio.
fn finalize_effects(text: String) -> Vec<Effect> {
    vec![
        commit_text_replacing_preedit(text),
        clear_candidates(),
        reset_candidate_context(),
    ]
}

/// v3.6.1 R2 — resolve the NextWord `roman` arg for a committed
/// continuous segment: the candidate's canonical TL when the platform
/// supplied it (`CommitContinuous.association_tl` / `NailedSegment
/// .association_tl`), else the raw committed slice. The canonical-TL path
/// keeps the learned `prev_tl` / `next_tl` aligned with a normal candidate
/// commit (fixing continuous-vs-normal fragmentation); the raw fallback
/// preserves pre-R2 behavior for legacy callers + TPS-OOV hanji-absent
/// candidates that carry no canonical TL.
pub(crate) fn association_roman(association_tl: &str, raw_text: &str) -> String {
    if association_tl.is_empty() {
        raw_text.to_owned()
    } else {
        association_tl.to_owned()
    }
}

fn next_word_update_last_selected_word(text: String, roman: String) -> Effect {
    Effect {
        kind: Some(effect::Kind::NextWordUpdateLastSelectedWord(
            NextWordUpdateLastSelectedWord { text, roman },
        )),
    }
}

/// Final-commit NextWord handshake: the terminal word, with the nailed
/// segments committed before it (`preceding`, document order) so NextWord
/// learns the whole composition as one sequence (behavioral-invariants §40).
fn next_word_word_selected(
    text: String,
    roman: String,
    trigger_prediction: bool,
    preceding: &[NailedSegment],
) -> Effect {
    let preceding = preceding
        .iter()
        .map(|segment| CommittedWord {
            text: segment.canonical_text.clone(),
            roman: association_roman(&segment.association_tl, &segment.raw_text),
        })
        .collect();
    Effect {
        kind: Some(effect::Kind::NextWordWordSelected(NextWordWordSelected {
            text,
            roman,
            trigger_prediction,
            preceding,
        })),
    }
}

/// Learned phrases (§50) — the `(Hanji, canonical-TL)` pair a final
/// continuous commit learns from its nailed segments, or `None` when the
/// composition is not one: fewer than two segments, any segment without
/// a hanji pick or without a canonical TL, or more than
/// [`MAX_LEARNED_PHRASE_SYLLABLES`] in total. The TL pieces join under
/// the commit's word boundaries ([`crate::api::learned_reading`]): a
/// dictionary compound with `-`, separate words with a space, and the
/// separator the user typed wins — by kind only ([`canonical_separators`];
/// the dictionary cannot cover every phrase and the user manages the
/// separator, USER 2026-09-22). The cap counts
/// the joined TL, not the segments' echoed `syllable_count`: a
/// custom-dictionary pick reports `1` whatever its length
/// (`lexicon::custom_entry_to_candidate`).
fn learned_phrase(nailed: &[NailedSegment]) -> Option<LearnedEntry> {
    if nailed.len() < 2 {
        return None;
    }
    let mut hanji = String::new();
    for seg in nailed {
        // `commit_continuous` stored an empty hanji as `None` already.
        let h = seg.hanji.as_deref()?;
        if seg.association_tl.is_empty() {
            return None;
        }
        hanji.push_str(h);
    }
    let canonical_tl = canonical_separators(&crate::api::learned_reading(nailed));
    let syllable_count = phonetics::api::tl_syllables(&canonical_tl).count();
    if syllable_count == 0 || syllable_count > MAX_LEARNED_PHRASE_SYLLABLES {
        return None;
    }
    Some(LearnedEntry {
        hanji,
        canonical_tl,
    })
}

/// The canonical separators a learned TL stores: a `-` run of three or
/// more is the khinsiann `--` (`---` → `--`), and the khinsiann marker
/// binds to the word before it, so the word space in front of a
/// dictionary khinsiann piece drops (`kì --khí-lâi` → `kì--khí-lâi`).
fn canonical_separators(tl: &str) -> String {
    let mut out = String::with_capacity(tl.len());
    let mut run = 0;
    for c in tl.chars() {
        if c == '-' {
            if run == 0 && out.ends_with(' ') {
                out.pop();
            }
            run += 1;
            if run <= 2 {
                out.push(c);
            }
        } else {
            run = 0;
            out.push(c);
        }
    }
    out
}

fn next_word_clear_for_new_composing() -> Effect {
    Effect {
        kind: Some(effect::Kind::NextWordClearForNewComposing(
            NextWordClearForNewComposing {},
        )),
    }
}
