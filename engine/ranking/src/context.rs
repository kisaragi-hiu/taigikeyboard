//! The previous-word context a continuous fetch ranks by: which candidates
//! the bigram tables say follow the word before the pending tail, and from
//! which layer. Built by `dispatch` (user bigrams + bundled `association.bin`
//! continuations), read by `lexicon` when it stamps `RawCandidate.context_rank`
//! (bigram-lm-roadmap D5; behavioral-invariants §56).

use std::collections::HashMap;
use std::sync::OnceLock;

/// A user-learned continuation of the previous word.
pub const CONTEXT_RANK_USER: u8 = 0;
/// A bundled (`association.bin`) continuation of the previous word.
pub const CONTEXT_RANK_BUNDLED: u8 = 1;
/// Not a known continuation — every candidate when there is no context.
pub const CONTEXT_RANK_NONE: u8 = 2;

/// Continuation ranks keyed by the `(display_text, canonical_tl)` pair, the
/// same identity `FrequencyMap` uses, with the same tolerant lookup: an
/// exact pair first, then the legacy `(display, "")` bucket (a user row
/// learned without a reading). Empty = no context — every rank is
/// [`CONTEXT_RANK_NONE`], so the sort order is the context-free one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextRanks {
    by_display: HashMap<String, HashMap<String, u8>>,
}

impl ContextRanks {
    pub fn new() -> Self {
        Self::default()
    }

    /// The shared empty map, for a fetch context that outlives a local.
    pub fn empty() -> &'static Self {
        static EMPTY: OnceLock<ContextRanks> = OnceLock::new();
        EMPTY.get_or_init(Self::new)
    }

    pub fn is_empty(&self) -> bool {
        self.by_display.is_empty()
    }

    /// Records `rank` for the pair; the better (lower) rank wins when the
    /// pair is both a user and a bundled continuation.
    pub fn insert(&mut self, display_text: String, canonical_tl: String, rank: u8) {
        let slot = self
            .by_display
            .entry(display_text)
            .or_default()
            .entry(canonical_tl)
            .or_insert(rank);
        *slot = (*slot).min(rank);
    }

    /// The pair's rank: exact `(display, tl)`, else the legacy `(display, "")`
    /// bucket, else [`CONTEXT_RANK_NONE`].
    pub fn rank(&self, display_text: &str, canonical_tl: &str) -> u8 {
        let Some(inner) = self.by_display.get(display_text) else {
            return CONTEXT_RANK_NONE;
        };
        let exact = inner.get(canonical_tl);
        let legacy = if canonical_tl.is_empty() {
            None
        } else {
            inner.get("")
        };
        exact.or(legacy).copied().unwrap_or(CONTEXT_RANK_NONE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_ranks_nothing() {
        let ranks = ContextRanks::new();
        assert!(ranks.is_empty());
        assert_eq!(ranks.rank("好", "hó"), CONTEXT_RANK_NONE);
        assert!(ContextRanks::empty().is_empty());
    }

    #[test]
    fn exact_pair_then_legacy_bucket() {
        let mut ranks = ContextRanks::new();
        ranks.insert("重".into(), "tāng".into(), CONTEXT_RANK_BUNDLED);
        ranks.insert("好".into(), String::new(), CONTEXT_RANK_USER);
        assert_eq!(ranks.rank("重", "tāng"), CONTEXT_RANK_BUNDLED);
        assert_eq!(
            ranks.rank("重", "tîng"),
            CONTEXT_RANK_NONE,
            "another reading"
        );
        assert_eq!(ranks.rank("好", "hó"), CONTEXT_RANK_USER, "legacy bucket");
        assert_eq!(ranks.rank("好", ""), CONTEXT_RANK_USER);
    }

    #[test]
    fn better_rank_wins_on_the_same_pair() {
        let mut ranks = ContextRanks::new();
        ranks.insert("好".into(), "hó".into(), CONTEXT_RANK_BUNDLED);
        ranks.insert("好".into(), "hó".into(), CONTEXT_RANK_USER);
        ranks.insert("好".into(), "hó".into(), CONTEXT_RANK_BUNDLED);
        assert_eq!(ranks.rank("好", "hó"), CONTEXT_RANK_USER);
    }
}
