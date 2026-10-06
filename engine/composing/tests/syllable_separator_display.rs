//! Syllable Separator (`AppConfig.syllable_separator`, USER 2026-09-20 /
//! 2026-10-06) integration test (`INVARIANT_SYLLABLE_SEPARATOR_DISPLAY_ONLY`).
//! Under None or Space, every candidate's rendered `roman` rewrites the
//! dictionary's inter-syllable `-` (dropped / a space) and the neutral-tone
//! marker `--` (`·` / ` ·`) — dictionary and custom rows alike — while the
//! identity sidechannels the platform round-trips on commit (`display_text`,
//! `canonical_tl`) keep the dictionary form, and the §34 literal keeps
//! whatever the user typed. Hyphen changes nothing.
//!
//! Hermetic `LexiconHandle` install comes from `tests/common/mod.rs`.

use protos::engine::{AppConfig, SyllableSeparator};

use crate::common::Fetch;
use crate::common::{
    build_dictionary_fst, build_syllables_fst, build_tkdb_v3, cell_with_hanji, config,
    empty_association_bin, fetch_cells, install_lexicon, Cell, Row,
};
use lexicon::CustomEntry;
use test_support::{engine_install_lock, write_temp};

fn fixture_rows() -> Vec<Row> {
    vec![
        Row {
            toneless_key: "taiuan",
            hanji: "台灣",
            tl: "tâi-uân",
            syll: 2,
            freq: 9000,
        },
        Row {
            toneless_key: "hoogua",
            hanji: "予我",
            tl: "hōo--guá",
            syll: 2,
            freq: 9000,
        },
        Row {
            toneless_key: "tai",
            hanji: "台",
            tl: "tâi",
            syll: 1,
            freq: 100,
        },
        Row {
            toneless_key: "uan",
            hanji: "灣",
            tl: "uân",
            syll: 1,
            freq: 100,
        },
        Row {
            toneless_key: "hoo",
            hanji: "予",
            tl: "hōo",
            syll: 1,
            freq: 100,
        },
        Row {
            toneless_key: "gua",
            hanji: "我",
            tl: "guá",
            syll: 1,
            freq: 100,
        },
        Row {
            toneless_key: "so",
            hanji: "鎖",
            tl: "só",
            syll: 1,
            freq: 100,
        },
        Row {
            toneless_key: "si",
            hanji: "匙",
            tl: "sî",
            syll: 1,
            freq: 100,
        },
    ]
}

fn install_fixture() {
    let rows = fixture_rows();
    let dict_path = write_temp("dictionary.bin", &build_tkdb_v3(&rows));
    let fst_path = build_dictionary_fst(&rows);
    let association_path = write_temp("association.bin", &empty_association_bin());
    let syllables_path = build_syllables_fst(&["tai5", "uan5", "hoo7", "gua2", "so2", "si5"]);
    install_lexicon(&fst_path, &dict_path, &association_path, &syllables_path);
}

fn fetch(
    raw: &str,
    input_mode: &str,
    separator: SyllableSeparator,
    custom: Vec<CustomEntry>,
) -> Vec<Cell> {
    let cfg = AppConfig {
        syllable_separator: separator as i32,
        ..config(input_mode)
    };
    let fetch = Fetch {
        custom,
        ..Default::default()
    };
    fetch_cells(&cfg, raw, fetch)
}

#[test]
fn separator_rewrites_the_dictionary_hyphen_but_keeps_the_identity_keys() {
    let _lock = engine_install_lock();
    install_fixture();
    let cells = fetch("taiuan", "tl", SyllableSeparator::Space, vec![]);
    assert_eq!(cell_with_hanji(&cells, "台灣").1, "tâi uân", "Space");
    let cells = fetch("taiuan", "tl", SyllableSeparator::None, vec![]);
    let taiuan = cell_with_hanji(&cells, "台灣");
    assert_eq!(taiuan.1, "tâiuân", "rendered roman");
    assert_eq!(taiuan.2, "台灣", "display_text (詞頻 key) untouched");
    assert_eq!(
        taiuan.3, "tâi-uân",
        "canonical_tl (association key) untouched"
    );
    assert_eq!(cells[0].1, "taiuan", "the §34 literal is what was typed");
}

#[test]
fn separator_writes_the_khinsiann_marker_as_a_middle_dot() {
    let _lock = engine_install_lock();
    install_fixture();
    let cells = fetch("hoogua", "tl", SyllableSeparator::None, vec![]);
    let hoogua = cell_with_hanji(&cells, "予我");
    assert_eq!(hoogua.1, "hōo\u{00b7}guá");
    assert_eq!(hoogua.3, "hōo--guá");
    let cells = fetch("hoogua", "tl", SyllableSeparator::Space, vec![]);
    let hoogua = cell_with_hanji(&cells, "予我");
    assert_eq!(
        hoogua.1, "hōo \u{00b7}guá",
        "the dot stays on the neutral syllable"
    );
    assert_eq!(hoogua.3, "hōo--guá");
}

#[test]
fn separator_applies_after_the_poj_presentation_pass() {
    let _lock = engine_install_lock();
    install_fixture();
    let cells = fetch("taioan", "poj", SyllableSeparator::Space, vec![]);
    assert_eq!(
        cell_with_hanji(&cells, "台灣").1,
        "tâi oân",
        "POJ spelling, spaced"
    );
    let cells = fetch("taioan", "poj", SyllableSeparator::None, vec![]);
    let taiuan = cell_with_hanji(&cells, "台灣");
    assert_eq!(taiuan.1, "tâioân", "POJ spelling, no hyphen");
    assert_eq!(taiuan.3, "tâi-uân", "canonical TL keeps the hyphen");
}

#[test]
fn separator_covers_custom_dictionary_rows() {
    let _lock = engine_install_lock();
    install_fixture();
    let custom = vec![CustomEntry {
        roman: "só-sî".into(),
        hanji: Some("鎖匙".into()),
    }];
    let cells = fetch("sosi", "tl", SyllableSeparator::Space, custom.clone());
    assert_eq!(cell_with_hanji(&cells, "鎖匙").1, "só sî");
    let cells = fetch("sosi", "tl", SyllableSeparator::None, custom);
    let sosi = cell_with_hanji(&cells, "鎖匙");
    assert_eq!(sosi.1, "sósî");
    assert_eq!(sosi.3, "só-sî", "custom identity keeps the stored hyphen");
}

#[test]
fn separator_leaves_a_typed_hyphen_in_the_literal_alone() {
    let _lock = engine_install_lock();
    install_fixture();
    for (separator, rendered) in [
        (SyllableSeparator::None, "tâiuân"),
        (SyllableSeparator::Space, "tâi uân"),
    ] {
        let cells = fetch("tai-uan", "tl", separator, vec![]);
        assert_eq!(cells[0].1, "tai-uan", "literal renders the typed hyphen");
        assert_eq!(cells[0].0, None);
        let taiuan = cell_with_hanji(&cells, "台灣");
        assert_eq!(taiuan.1, rendered, "dictionary row follows the setting");
    }
}

#[test]
fn hyphen_renders_the_dictionary_form() {
    let _lock = engine_install_lock();
    install_fixture();
    let off = fetch("hoogua", "tl", SyllableSeparator::Hyphen, vec![]);
    assert_eq!(cell_with_hanji(&off, "予我").1, "hōo--guá");
    // Same rows, same order, same identity — only `roman` differs.
    for separator in [SyllableSeparator::None, SyllableSeparator::Space] {
        let on = fetch("hoogua", "tl", separator, vec![]);
        assert_eq!(on.len(), off.len(), "{separator:?}");
        for (a, b) in on.iter().zip(&off) {
            assert_eq!((&a.0, &a.2, &a.3), (&b.0, &b.2, &b.3), "{a:?} vs {b:?}");
        }
    }
}
