//! §52 — a typed-`-` segment that is one syllable IS that syllable (USER
//! 2026-10-08: `siam-tioh` offered 寫 `siá` above 閃 `siám`; "the `-`
//! separator should let the user define how syllables are split").
//!
//! The no-crossing rule alone let `sia|m` cut inside `siam`: `sia` never
//! crosses the barrier, `m` (毋) is a syllable, and the §18 phrase guard
//! kept `sia` because `si`+`a` (匙仔) reaches its end. Two layers close it:
//! the syllabifier admits no cut inside a closed one-syllable segment, and
//! the span's lookup pin drops a reading that splits one (阿姨 `a-î` under
//! the one key `ai` of a typed `ai-`).
//!
//! Fixture mirrors production around the report: `siam` / `sia` / `si` /
//! `a` / `m` are all syllables, `sia` holds both 寫 (one syllable) and
//! 匙仔 `sî-á` (two), `ai` holds 愛 (one) and 阿姨 `a-î` (two).

use crate::common::Fetch;
use crate::common::{
    build_dictionary_fst, build_syllables_fst, build_tkdb_v3, config, empty_association_bin,
    fetch_cells, fetch_hanji, install_lexicon, Cell, Row,
};
use lexicon::CustomEntry;
use test_support::{engine_install_lock, write_temp};

fn row(toneless_key: &'static str, hanji: &'static str, tl: &'static str, freq: u32) -> Row {
    Row {
        toneless_key,
        hanji,
        tl,
        syll: tl.split(['-', ' ']).filter(|s| !s.is_empty()).count() as u8,
        freq,
    }
}

fn fixture_rows() -> Vec<Row> {
    vec![
        row("siam", "閃", "siám", 600),
        row("sia", "寫", "siá", 7_896),
        row("sia", "匙仔", "sî-á", 27),
        row("m", "毋", "m̄", 9_000),
        row("tioh", "著", "tio̍h", 20_000),
        row("siamtioh", "閃著", "siám-tio̍h", 30),
        row("ai", "愛", "ài", 30_000),
        row("ai", "阿姨", "a-î", 3_000),
        row("tai", "台", "tâi", 5_000),
        row("taigi", "台語", "tâi-gí", 4_000),
        row("bun", "文", "bûn", 3_000),
    ]
}

fn install_fixture() {
    let rows = fixture_rows();
    let dict_path = write_temp("dictionary.bin", &build_tkdb_v3(&rows));
    let fst_path = build_dictionary_fst(&rows);
    let association_path = write_temp("association.bin", &empty_association_bin());
    let syllables_path = build_syllables_fst(&[
        "siam2", "sia2", "si5", "a1", "a2", "m7", "tioh8", "ai3", "i5", "tai5", "gi2", "bun5",
    ]);
    install_lexicon(&fst_path, &dict_path, &association_path, &syllables_path);
}

fn fetch(raw: &str, mode: &str, fetch: Fetch) -> Vec<Cell> {
    fetch_cells(&config(mode), raw, fetch)
}

fn hanji_of(cells: &[Cell]) -> Vec<&str> {
    cells.iter().filter_map(|c| c.0.as_deref()).collect()
}

// INVARIANT_TYPED_HYPHEN_IS_A_SYLLABLE_BOUNDARY (behavioral-invariants.md §52)
#[test]
fn a_typed_hyphen_closes_a_one_syllable_segment_against_inner_cuts() {
    let _lock = engine_install_lock();
    install_fixture();
    // The toned `siam2-tioh8` needs toned dictionary keys: production test
    // `one_syllable_segment_prod.rs`.
    for (raw, mode) in [("siam-tioh", "tl"), ("siam-", "tl"), ("siam-tioh", "poj")] {
        let cells = fetch(raw, mode, Fetch::default());
        let hanji = hanji_of(&cells);
        for cut in ["寫", "匙仔"] {
            assert!(
                !hanji.contains(&cut),
                "{raw} ({mode}): {cut} cuts inside the typed `siam`; got {cells:?}"
            );
        }
        assert!(hanji.contains(&"閃"), "{raw} ({mode}): got {cells:?}");
    }
    let cells = fetch("siam-tioh", "tl", Fetch::default());
    assert_eq!(cells[1].0.as_deref(), Some("閃著"), "got {cells:?}");
}

#[test]
fn without_a_hyphen_the_inner_cut_stays_a_reading() {
    // Control: nothing typed says `siam` is one syllable, so `sia|m|tioh`
    // remains a genuine reading.
    let _lock = engine_install_lock();
    install_fixture();
    let cells = fetch("siamtioh", "tl", Fetch::default());
    assert!(hanji_of(&cells).contains(&"寫"), "got {cells:?}");
}

#[test]
fn a_reading_that_splits_the_segment_drops_under_the_same_key() {
    // `ai` is one key for 愛 `ài` and 阿姨 `a-î`; a typed `ai-` keeps only
    // the one-syllable reading, `a-i` only the two-syllable one, and `ai`
    // with nothing typed keeps both.
    let _lock = engine_install_lock();
    install_fixture();
    for (raw, has_one_syllable, has_two_syllables) in [
        ("ai-", true, false),
        ("a-i", false, true),
        ("ai", true, true),
    ] {
        let hanji = fetch_hanji(raw, "tl", Fetch::default());
        let has = |word: &str| hanji.iter().any(|h| h == word);
        assert_eq!(has("愛"), has_one_syllable, "{raw}: got {hanji:?}");
        assert_eq!(has("阿姨"), has_two_syllables, "{raw}: got {hanji:?}");
    }
}

#[test]
fn a_custom_reading_that_splits_the_segment_drops_too() {
    let _lock = engine_install_lock();
    install_fixture();
    let cells = fetch(
        "ai-",
        "tl",
        Fetch {
            custom: vec![CustomEntry {
                roman: "a-ī".into(),
                hanji: Some("阿二".into()),
            }],
            ..Default::default()
        },
    );
    assert!(!hanji_of(&cells).contains(&"阿二"), "got {cells:?}");
}

#[test]
fn a_segment_that_is_not_one_syllable_still_splits_inside() {
    // `taigi` is no single syllable, so the typed `-` after it constrains
    // nothing inside: 台 `tâi` stays a left-anchored word.
    let _lock = engine_install_lock();
    install_fixture();
    let cells = fetch("taigi-bun", "tl", Fetch::default());
    let hanji = hanji_of(&cells);
    assert!(hanji.contains(&"台"), "got {cells:?}");
    assert!(hanji.contains(&"台語"), "got {cells:?}");
}
