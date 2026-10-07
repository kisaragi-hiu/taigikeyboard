//! A custom-dictionary word and the same dictionary word are ONE candidate
//! whatever Unicode form the custom roman was stored in (Core Principle #6:
//! word identity = `(hanji, canonical-TL)`).
//!
//! Reported symptom (2026-10-07 desktop pre-release run, macOS): typing
//! `li2` listed 李 `lí` twice. The user's imported custom dictionary stores
//! `lí` decomposed (`li` + U+0301 COMBINING ACUTE ACCENT, NFD) while
//! `dictionary.bin` stores it precomposed (U+00ED, NFC); the candidate
//! dedupes compared the raw roman bytes, so the pair never collided.

use crate::common::{
    build_dictionary_fst, build_syllables_fst, build_tkdb_v3, config, empty_association_bin,
    fetch_cells, install_lexicon, Fetch, Row,
};
use lexicon::CustomEntry;
use test_support::{engine_install_lock, write_temp};

/// 李/lí plus a second `li` word so the list is never a single candidate.
fn fixture_rows() -> Vec<Row> {
    vec![
        Row {
            toneless_key: "li",
            hanji: "李",
            tl: "lí",
            syll: 1,
            freq: 5000,
        },
        Row {
            toneless_key: "li",
            hanji: "你",
            tl: "lí",
            syll: 1,
            freq: 30000,
        },
        Row {
            toneless_key: "li",
            hanji: "利",
            tl: "lī",
            syll: 1,
            freq: 3000,
        },
        // trace: abbrev "ss" (só + sî), toneless "sosi".
        Row {
            toneless_key: "sosi",
            hanji: "鎖匙",
            tl: "só-sî",
            syll: 2,
            freq: 800,
        },
    ]
}

fn install() {
    let rows = fixture_rows();
    let dict_path = write_temp("dictionary.bin", &build_tkdb_v3(&rows));
    let fst_path = build_dictionary_fst(&rows);
    let association_path = write_temp("association.bin", &empty_association_bin());
    let syllables_path = build_syllables_fst(&["li2", "li7", "so2", "si5"]);
    install_lexicon(&fst_path, &dict_path, &association_path, &syllables_path);
}

fn with_custom(roman: &str, hanji: &str) -> Fetch {
    Fetch {
        custom: vec![CustomEntry {
            roman: roman.into(),
            hanji: Some(hanji.into()),
        }],
        ..Default::default()
    }
}

fn count_in(
    input_mode: &str,
    raw: &str,
    fetch: Fetch,
    hanji: &str,
) -> (usize, Vec<Option<String>>) {
    let cells = fetch_cells(&config(input_mode), raw, fetch);
    let all: Vec<Option<String>> = cells.iter().map(|c| c.0.clone()).collect();
    let n = cells
        .iter()
        .filter(|c| c.0.as_deref() == Some(hanji))
        .count();
    (n, all)
}

#[test]
fn decomposed_custom_roman_collapses_with_the_precomposed_dictionary_word() {
    let _lock = engine_install_lock();
    install();
    for raw in ["li2", "li"] {
        let (n, all) = count_in("tl", raw, with_custom("li\u{301}", "李"), "李");
        assert_eq!(n, 1, "{raw}: NFD custom 李 listed once; got {all:?}");
    }
}

#[test]
fn precomposed_custom_roman_collapses_too() {
    // Negative control: the NFC custom row always collided byte for byte.
    let _lock = engine_install_lock();
    install();
    for raw in ["li2", "li"] {
        let (n, all) = count_in("tl", raw, with_custom("l\u{ed}", "李"), "李");
        assert_eq!(n, 1, "{raw}: NFC custom 李 listed once; got {all:?}");
    }
}

#[test]
fn a_different_tone_is_still_a_different_word() {
    // Identity keeps the tone: custom 李/lī (tone 7) is not dictionary 李/lí.
    let _lock = engine_install_lock();
    install();
    let (n, all) = count_in("tl", "li", with_custom("li\u{304}", "李"), "李");
    assert_eq!(n, 2, "lī and lí are two words; got {all:?}");
}

#[test]
fn decomposed_custom_roman_collapses_under_caps_and_poj() {
    // The recase pass rewrites `roman` after the lexicon dedupe and POJ
    // renders it; the pair must stay one candidate through both.
    let _lock = engine_install_lock();
    install();
    for (input_mode, raw) in [("tl", "LI2"), ("tl", "LI"), ("poj", "li2"), ("poj", "li")] {
        let (n, all) = count_in(input_mode, raw, with_custom("li\u{301}", "李"), "李");
        assert_eq!(
            n, 1,
            "{input_mode} {raw}: NFD custom 李 listed once; got {all:?}"
        );
    }
}

#[test]
fn decomposed_custom_roman_collapses_on_the_abbreviation_path() {
    // The abbreviation block (Step 4c) does not merge custom rows again;
    // its dictionary hit meets the custom row only through
    // `retain_absent_from`'s identity half.
    let _lock = engine_install_lock();
    install();
    let custom = || with_custom("so\u{301}-si\u{302}", "鎖匙");
    for raw in ["ss", "SS"] {
        let (n, all) = count_in("tl", raw, custom(), "鎖匙");
        assert_eq!(n, 1, "{raw}: NFD custom 鎖匙 listed once; got {all:?}");
    }
}

#[test]
fn decomposed_custom_roman_collapses_on_the_no_key_partial_path() {
    let _lock = engine_install_lock();
    install();
    for raw in ["l", "L"] {
        let (n, all) = count_in("tl", raw, with_custom("li\u{301}", "李"), "李");
        assert_eq!(n, 1, "{raw}: NFD custom 李 listed once; got {all:?}");
    }
}
