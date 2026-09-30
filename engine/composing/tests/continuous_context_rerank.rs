//! INVARIANT_CONTINUOUS_CONTEXT_RERANK (behavioral-invariants §56): a
//! previous-word context re-ranks the continuous candidates — the walker's
//! slot 0 and the span-local list — without changing the segmentation. The
//! context itself is built by `dispatch` (bundled + user bigrams); these tests
//! hand a `ContextRanks` straight to the fetch, against the production lexicon.

use crate::common;

use crate::common::{config_tl, fetch_at_pos_response, fetch_cells, Fetch};
use composing::api::Engine;
use composing::Intent;
use ranking::{ContextRanks, CONTEXT_RANK_BUNDLED};

/// The bundled continuations of 真/tsin that read `tse`: 濟 under both its
/// readings (association.bin v2 word key `真\u{1}tsin`).
fn after_tsin() -> ContextRanks {
    let mut ranks = ContextRanks::new();
    ranks.insert("濟".into(), "tsē".into(), CONTEXT_RANK_BUNDLED);
    ranks.insert("濟".into(), "tsuē".into(), CONTEXT_RANK_BUNDLED);
    ranks
}

/// The hanji of the candidates in display order (the §34 literal dropped).
fn hanji(raw: &str, context: ContextRanks) -> Vec<String> {
    fetch_cells(
        &config_tl(),
        raw,
        Fetch {
            context,
            ..Fetch::default()
        },
    )
    .into_iter()
    .filter_map(|(hanji, ..)| hanji)
    .collect()
}

// After 真 the strip's own continuation 濟 leads the `tse` list — slot 0 (the
// walker's word for the edge) and the span-local list agree — where the
// context-free order leads with a more frequent homophone (今 today).
#[test]
fn context_hit_leads_slot_zero_and_the_list() {
    if !common::production_lexicon_ready() {
        return;
    }
    let baseline = hanji("tse", ContextRanks::new());
    assert!(baseline.contains(&"濟".to_owned()), "{baseline:?}");
    assert_ne!(
        baseline.first().map(String::as_str),
        Some("濟"),
        "{baseline:?}"
    );
    let ranked = hanji("tse", after_tsin());
    assert_eq!(ranked.first().map(String::as_str), Some("濟"), "{ranked:?}");
    // The two lists hold the same words: the context orders, never filters.
    let mut sorted_baseline = baseline.clone();
    let mut sorted_ranked = ranked.clone();
    sorted_baseline.sort();
    sorted_ranked.sort();
    assert_eq!(sorted_baseline, sorted_ranked);
}

// A context that names none of the candidates changes nothing.
#[test]
fn context_without_a_hit_is_the_context_free_order() {
    if !common::production_lexicon_ready() {
        return;
    }
    let mut unrelated = ContextRanks::new();
    unrelated.insert("飯".into(), "pn̄g".into(), CONTEXT_RANK_BUNDLED);
    assert_eq!(hanji("tse", unrelated), hanji("tse", ContextRanks::new()));
}

// The segmentation is frozen: S5 `taiuan` keeps its two-syllable slot 0
// (台員 / 台灣, one edge) with a context that favours the single 台 — a walker
// that re-priced edges on the pick could split the buffer.
#[test]
fn context_never_changes_the_segmentation() {
    if !common::production_lexicon_ready() {
        return;
    }
    let mut favours_tai = ContextRanks::new();
    favours_tai.insert("台".into(), "tâi".into(), CONTEXT_RANK_BUNDLED);
    let baseline = hanji("taiuan", ContextRanks::new());
    let ranked = hanji("taiuan", favours_tai);
    let slot0 = baseline.first().expect("a candidate");
    assert_eq!(slot0.chars().count(), 2, "{baseline:?}");
    assert_eq!(ranked.first(), Some(slot0), "{ranked:?}");
    // 台 itself moved up among the single-syllable rows, nothing else.
    let position = |list: &[String]| list.iter().position(|h| h == "台").expect("台 listed");
    assert!(position(&ranked) <= position(&baseline), "{ranked:?}");
}

// Inside a composition the pending tail follows the last nailed segment:
// `Engine::pending_context` names it by the identity the final commit's
// `preceding` carries (canonical text + association roman).
#[test]
fn pending_context_is_the_last_nailed_segment() {
    if !common::production_lexicon_ready() {
        return;
    }
    let config = config_tl();
    let mut engine = Engine::new();
    assert_eq!(engine.pending_context(), None);
    engine.apply(
        Intent::Start {
            text: "tsintse".to_string(),
        },
        &config,
    );
    engine.apply(Intent::EnterContinuous, &config);
    assert_eq!(engine.pending_context(), None, "nothing nailed yet");
    engine.apply(
        Intent::CommitContinuous {
            display_text: "真".to_string(),
            canonical_text: "真".to_string(),
            association_tl: "tsin".to_string(),
            hanji: Some("真".to_string()),
            consumed_bytes: 4,
            syllable_count: 1,
            resolve: None,
        },
        &config,
    );
    assert_eq!(
        engine.pending_context(),
        Some(("真".to_string(), "tsin".to_string()))
    );
    let response = fetch_at_pos_response(&config, "tse", Fetch::default());
    assert!(response.continuous.is_some(), "the fixture still fetches");
}
