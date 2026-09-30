//! v3.5.9 B-2 — runtime POJ-notone derivation parity test (Codex
//! post-impl SHOULD #1).
//!
//! `phonetics::KeyFamily::Poj.toneless_face` (read by the lexicon
//! `matches_continuous_poj_toneless_key` guard) derives each row's
//! `poj_notone` at runtime (steps: `key_family.rs::poj_notone_of_display`);
//! `dictionary.fst` is built from the precomputed `poj_notone` column of
//! `dictionary/output/dictionary.csv`. Drift silently rejects every POJ
//! family hit whose `tl ↔ poj_notone` mapping diverges.
//!
//! This parity test reads the shipped CSV and asserts that the runtime
//! derivation byte-matches the stored `poj_notone` column for every
//! row whose `tl` and `poj_notone` are both non-empty and ASCII
//! (anomaly rows where the build pipeline left non-ASCII glyphs in
//! `poj_notone` are skipped — they cannot be reached by any continuous
//! `poj:<ascii>` lookup key anyway). If the CSV is not present (some
//! lean checkouts may omit the 24 MB output file) the test no-ops
//! with a soft skip rather than blocking.

use phonetics::KeyFamily;
use test_support::dictionary_csv_or_skip;

/// `[tl, poj_notone]` per row; `None` (logged, the `suite` soft-skips) in a lean checkout.
fn read_rows(suite: &str) -> Option<Vec<[&'static str; 2]>> {
    Some(
        dictionary_csv_or_skip(suite)?
            .select(["tl", "poj_notone"])
            .collect(),
    )
}

#[test]
fn runtime_poj_notone_matches_build_pipeline_for_every_row() {
    let Some(rows) = read_rows("POJ-notone") else {
        return;
    };

    assert!(
        !rows.is_empty(),
        "expected non-empty `dictionary.csv` row stream"
    );

    let mut total = 0usize;
    let mut compared = 0usize;
    let mut anomalies = 0usize;
    let mut drift: Vec<(String, String, String)> = Vec::new();

    for &[tl, poj_notone] in &rows {
        total += 1;
        if tl.is_empty() || poj_notone.is_empty() {
            continue;
        }
        // Skip rows whose shipped `poj_notone` is not pure ASCII —
        // those are upstream build-pipeline anomalies (e.g. stacked
        // diacritics on a single base char survive `to_numeric_tone`)
        // that no user can possibly type as a continuous-input toneless
        // key. The matching guard correctly never reaches them via the
        // `poj:<ascii>` lookup path.
        if !poj_notone.is_ascii() {
            anomalies += 1;
            continue;
        }
        compared += 1;
        let derived = KeyFamily::Poj.toneless_face(tl);
        if derived != *poj_notone {
            drift.push((tl.to_string(), poj_notone.to_string(), derived));
            if drift.len() >= 10 {
                break;
            }
        }
    }

    assert!(
        drift.is_empty(),
        "runtime POJ-notone derivation drifted from build pipeline on \
         {} of {} compared rows (first {} shown):\n{}",
        drift.len(),
        compared,
        drift.len(),
        drift
            .iter()
            .map(|(tl, expected, got)| format!(
                "  tl={tl:?} expected poj_notone={expected:?} got {got:?}"
            ))
            .collect::<Vec<_>>()
            .join("\n"),
    );

    eprintln!(
        "POJ-notone parity OK: {compared}/{total} rows compared \
         ({anomalies} non-ASCII anomaly rows skipped)."
    );
}
