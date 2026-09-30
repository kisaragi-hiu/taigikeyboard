//! §43 — runtime `tl_num` / `poj_num` derivation parity test.
//!
//! `SyllableReach` (`lexicon::continuous`) decides whether a strict-prefix
//! extension may be offered by measuring how far the typed body reaches into
//! the record's reading, ON THE KEY SURFACE the row was found under. For a
//! tone-bearing body that surface is the `tl:<tl_num>` / `poj:<poj_num>` family
//! `create_fst.py:141` emits from the precomputed `tl_num` / `poj_num` columns
//! of `dictionary/output/dictionary.csv`, and the runtime mirrors of those
//! columns are `phonetics::tl_num_syllable_ends_from_tl` /
//! `phonetics::poj_num_syllable_ends_from_tl`.
//!
//! Drift is SILENT: a mirror that disagrees with the column no longer matches
//! its own key, `SyllableReach` fails open, and the over-length candidates
//! §43 exists to remove come back for every affected reading — with no error,
//! no panic and no failing unit test, because a hermetic fixture builds its
//! keys from the same mirror it is testing. This file is the only thing that
//! catches it, so it reads the shipped CSV and asserts a byte-match for every
//! row.
//!
//! Two conventions, deliberately: the `tl_num` column keeps the reading's own
//! glyphs (`thò͘-sái` → `tho͘3sai2`) while `poj_num` is ASCII-folded
//! (`khuànn` → `khoann3`). Applying either convention to both columns costs
//! tens of thousands of divergences; this is why the two mirrors are separate
//! functions rather than one with a mode flag.
//!
//! The `tl_abbrev` / `poj_abbrev` columns (the whole-buffer abbreviation
//! lookup `lexicon::fetch_abbrev_candidates` verifies hits against) are
//! pinned here too, against `phonetics::derive_abbrev` over the TL / POJ
//! display — the runtime mirror of `dictionary/common/abbrev.py::
//! extract_abbrev`. Single-syllable rows carry an empty column and are
//! skipped like any other empty expectation.
//!
//! Soft-skips when the CSV is absent (lean checkout), like its
//! `tps_notone_parity` / `poj_notone_parity` siblings.

use test_support::dictionary_csv_or_skip;

/// The reading plus the six precomputed romanization key columns
/// `create_fst.py` emits key families from. Every gate selects all seven, so
/// the rows compared do not depend on which column is under test.
const COLUMNS: [&str; 7] = [
    "tl",
    "tl_num",
    "poj_num",
    "tl_notone",
    "poj_notone",
    "tl_abbrev",
    "poj_abbrev",
];

/// A reading the continuous path can actually reach: a `tl:`/`poj:` lookup key
/// is romanization, so a row whose `tl` column holds Hanji (an upstream
/// build-pipeline anomaly) is unreachable and out of the gate — the same
/// carve-out `tps_notone_parity` makes for non-Bopomofo `tps_notone`.
fn is_romanization(reading: &str) -> bool {
    !reading.is_empty() && reading.chars().any(|c| c.is_ascii_alphabetic())
}

#[test]
fn runtime_tl_num_matches_build_pipeline_for_every_row() {
    assert_column_parity("tl_num", |tl| phonetics::tl_num_syllable_ends_from_tl(tl).0);
}

#[test]
fn runtime_poj_num_matches_build_pipeline_for_every_row() {
    assert_column_parity("poj_num", |tl| {
        phonetics::poj_num_syllable_ends_from_tl(tl).0
    });
}

#[test]
fn runtime_tl_notone_matches_build_pipeline_for_every_row() {
    assert_column_parity("tl_notone", |tl| {
        strip_digits(&phonetics::tl_num_syllable_ends_from_tl(tl).0)
    });
}

#[test]
fn runtime_poj_notone_matches_build_pipeline_for_every_row() {
    assert_column_parity("poj_notone", |tl| {
        strip_digits(&phonetics::poj_num_syllable_ends_from_tl(tl).0)
    });
}

#[test]
fn runtime_tl_abbrev_matches_build_pipeline_for_every_row() {
    assert_column_parity("tl_abbrev", phonetics::derive_abbrev);
}

#[test]
fn runtime_poj_abbrev_matches_build_pipeline_for_every_row() {
    assert_column_parity("poj_abbrev", phonetics::poj_abbrev_from_tl);
}

fn strip_digits(face: &str) -> String {
    face.chars().filter(|c| !c.is_ascii_digit()).collect()
}

/// Every syllable end offset must land on a char boundary of the face and the
/// last one must be its full length — `SyllableReach` slices by these.
#[test]
fn syllable_ends_bound_the_face_they_describe() {
    for tl in [
        "tâi-uân",
        "kau-kuan",
        "ke-si-thâu-á",
        "hōo--guá",
        "koh",
        "thò͘-sái",
    ] {
        for (face, ends) in [
            phonetics::tl_num_syllable_ends_from_tl(tl),
            phonetics::poj_num_syllable_ends_from_tl(tl),
        ] {
            assert!(!ends.is_empty(), "{tl}: expected at least one syllable");
            assert_eq!(
                ends.last().copied().unwrap() as usize,
                face.len(),
                "{tl}: last end must be the face length ({face})"
            );
            for end in &ends {
                assert!(
                    face.is_char_boundary(*end as usize),
                    "{tl}: end {end} is not a char boundary of {face}"
                );
            }
            assert!(
                ends.windows(2).all(|w| w[0] < w[1]),
                "{tl}: ends must strictly increase, got {ends:?}"
            );
        }
    }
}

fn assert_column_parity(column: &str, derive: impl Fn(&str) -> String) {
    let Some(csv) = dictionary_csv_or_skip(column) else {
        return;
    };
    let expected_idx = COLUMNS
        .iter()
        .position(|c| *c == column)
        .unwrap_or_else(|| panic!("`{column}` is one of {COLUMNS:?}"));
    let rows: Vec<[&str; 7]> = csv.select(COLUMNS).collect();
    assert!(
        !rows.is_empty(),
        "expected non-empty `dictionary.csv` row stream"
    );

    let mut compared = 0usize;
    let mut skipped = 0usize;
    let mut drifted = 0usize;
    let mut drift: Vec<(String, String, String)> = Vec::new();
    for row in &rows {
        let (tl, expected) = (row[0], row[expected_idx]);
        if tl.is_empty() || expected.is_empty() {
            continue;
        }
        if !is_romanization(tl) {
            skipped += 1;
            continue;
        }
        compared += 1;
        let derived = derive(tl);
        if derived != expected {
            drifted += 1;
            if drift.len() < 20 {
                drift.push((tl.to_string(), derived, expected.to_string()));
            }
        }
    }

    assert!(
        compared > 100_000,
        "expected the shipped CSV to carry >100k comparable `{column}` rows, got {compared}"
    );
    assert!(
        drifted == 0,
        "runtime `{column}` derivation drifted from the build pipeline on {drifted} of {compared} \
         rows ({skipped} non-romanization rows skipped); first divergences (tl, derived, csv): \
         {drift:#?}"
    );
}
