//! The previous-word context a continuous fetch ranks by (bigram-lm-roadmap
//! P5, behavioral-invariants §56): the bundled `association.bin`
//! continuations of the word the pending tail follows, as `ContextRanks`.
//! The user-learned continuations join in `user_data::with_stores`; before
//! the stores open, and in a build without them, the bundled ones rank alone.

use composing::{EngineHandle as ComposingHandle, Intent, PendingSnapshot, UserRows};
use protos::engine::{
    composing_request, AppConfig, AssocLookupRequest, ComposingRequest, ComposingResponse,
    FetchAtPos, LexiconAssocEntry,
};
use ranking::{ContextRanks, CONTEXT_RANK_BUNDLED};

/// Continuations read per layer — what the next-word strip would show.
pub(crate) const CONTEXT_ROWS: usize = nextword::api::DEFAULT_PREDICTION_LIMIT;

/// The word the pending tail follows: the composition's last nailed segment,
/// else the last committed word inside its association window (§56).
pub(crate) fn context_word(snapshot: &PendingSnapshot, now_ms: i64) -> Option<(String, String)> {
    snapshot
        .previous_word
        .clone()
        .or_else(|| nextword::EngineHandle::instance().context_snapshot(now_ms))
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
) -> Vec<LexiconAssocEntry> {
    let lookup = lexicon::api::assoc_lookup(AssocLookupRequest {
        previous_word: previous_word.to_owned(),
        previous_tl: previous_tl.to_owned(),
        limit: u32::try_from(limit).unwrap_or(u32::MAX),
        enabled_sources_bitmask,
    });
    match lookup {
        Ok(response) => response.entries,
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

/// The fetch intent for `sent`, with the rows and context the engine read.
pub(crate) fn fetch_intent(
    sent: &FetchAtPos,
    user_rows: UserRows,
    context: ContextRanks,
) -> Intent {
    Intent::FetchAtPos {
        now_ms: sent.now_ms,
        enabled_sources_bitmask: sent.enabled_sources_bitmask,
        literal_roman_candidate_disabled: sent.literal_roman_candidate_disabled,
        user_rows,
        context,
    }
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
    // A stale generation answers the idle snapshot inside `query`.
    let context = composing
        .pending_snapshot(generation)
        .map(|snapshot| bundled_ranks(context_word(&snapshot, sent.now_ms).as_ref()))
        .unwrap_or_default();
    Ok(composing.query(
        &fetch_intent(sent, UserRows::default(), context),
        config,
        generation,
    ))
}
