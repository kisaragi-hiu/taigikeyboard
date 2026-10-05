//! The previous-word context a continuous fetch ranks by (bigram-lm-roadmap
//! P5, behavioral-invariants §56): the bundled `association.bin`
//! continuations of the word the pending tail follows, as `ContextRanks`.
//! The user-learned continuations join in `user_data::with_stores`; before
//! the stores open, and in a build without them, the bundled ones rank alone.

use composing::requests::fetch_at_pos_intent;
use composing::{EngineHandle as ComposingHandle, ListContext, PendingSnapshot, UserRows};
use lexicon::search::AssociationHit;
use protos::engine::{composing_request, AppConfig, ComposingRequest, ComposingResponse};
use ranking::{ContextRanks, CONTEXT_RANK_BUNDLED};

/// Continuations read per layer — what the next-word strip would show.
pub(crate) const CONTEXT_ROWS: usize = nextword::api::DEFAULT_PREDICTION_LIMIT;

/// The word the listed buffer follows: a word of the composition, else —
/// when the list starts the composition — the last committed word inside its
/// association window (§56).
pub(crate) fn context_word(snapshot: &PendingSnapshot, now_ms: i64) -> Option<(String, String)> {
    match &snapshot.context {
        ListContext::Word(word, roman) => Some((word.clone(), roman.clone())),
        ListContext::Committed => nextword::EngineHandle::instance().context_snapshot(now_ms),
        ListContext::Cut => None,
    }
}

/// The bundled `association.bin` continuations of a committed word — the
/// §24 lookup (word key, character-key backoff), source-filtered by
/// `enabled_sources_bitmask`. A lookup failure (lexicon not installed yet)
/// yields no entries, so the caller's other rows still surface.
pub(crate) fn bundled_continuations(
    previous_word: &str,
    previous_tl: &str,
    limit: usize,
    enabled_sources_bitmask: u32,
) -> Vec<AssociationHit> {
    let lookup = lexicon::api::lookup_associations(
        previous_word,
        previous_tl,
        u32::try_from(limit).unwrap_or(u32::MAX),
        enabled_sources_bitmask,
    );
    match lookup {
        Ok(entries) => entries,
        Err(err) => {
            log::debug!("assoc.bundled_lookup_skipped: {err}");
            Vec::new()
        }
    }
}

/// The bundled continuations of `previous` as context ranks — the same
/// lookup as the next-word strip so the list agrees with it. Every source is
/// passed: an entry's mask is the next word's sources, and a next word the
/// fetch's own source filter dropped is never a candidate, so the candidate
/// list is the filter.
pub(crate) fn bundled_ranks(previous: Option<&(String, String)>) -> ContextRanks {
    let mut ranks = ContextRanks::new();
    let Some((word, word_tl)) = previous else {
        return ranks;
    };
    for entry in bundled_continuations(word, word_tl, CONTEXT_ROWS, u32::MAX) {
        ranks.insert(
            entry.candidate_word,
            entry.candidate_tl,
            CONTEXT_RANK_BUNDLED,
        );
    }
    ranks
}

/// A composing request without the user-data stores: a `FetchAtPos` ranks
/// by the bundled context, every other request goes straight to composing.
pub(crate) fn handle_composing_without_stores(
    request: &ComposingRequest,
    config: &AppConfig,
    generation: u64,
) -> Result<ComposingResponse, composing::ComposingError> {
    let composing = ComposingHandle::instance();
    let Some(composing_request::Method::FetchAtPos(sent)) = request.method.as_ref() else {
        return composing.handle(request, config, generation);
    };
    // One copy of the engine reads the context and answers the fetch.
    let Some(engine) = composing.engine_at(generation) else {
        return Ok(composing::Engine::idle_snapshot(config));
    };
    let snapshot = engine.pending_snapshot(sent.word_before_caret, config);
    let context = bundled_ranks(context_word(&snapshot, sent.now_ms).as_ref());
    Ok(composing::requests::query(
        &fetch_at_pos_intent(sent, UserRows::default(), context),
        &engine,
        config,
    ))
}
