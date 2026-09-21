//! Typed word separator in the continuous-input best reading (USER
//! 2026-09-21). The walker's slot-0 synth joins its segments with the
//! separator the user typed between them — `-` 連字 or `--` 輕聲 — and with
//! a space only where nothing was typed. What is dictionary-owned stays
//! dictionary-owned: a `-` inside one dictionary word, and a whole-buffer
//! reading that IS a dictionary word (`hoo-gua` → 予我 `hōo--guá`, the
//! `continuous_slot0_dict_roman` promotion), keep the record's own form.
//!
//! The typed join is presentation only: `canonical_tl` / `display_text`
//! (the `(hanji, canonical_tl)` identity) read the space join, so the same
//! phrase typed three ways keys `user_frequency.db` once.
//!
//! Fixture: 我/guá + 是/sī are high-freq singles with no `guasi` word, so
//! the walker path is the genuine two-word split; 予/hōo + 我/guá share the
//! same shape but 予我/hōo--guá exists as a word; `lai` is a syllable with
//! no dictionary row (the all-OOV branch).

use protos::engine::{AppConfig, FetchAtPos};

mod common;
use common::{
    build_dictionary_fst, build_syllables_fst, build_tkdb_v3, cell_with_hanji, config,
    empty_association_bin, engine_install_lock, fetch_cells, install_lexicon, write_temp, Cell,
    Row,
};

fn fixture_rows() -> Vec<Row> {
    vec![
        Row {
            toneless_key: "hoo",
            hanzi: "予",
            tl: "hōo",
            syll: 1,
            freq: 80_000,
        },
        Row {
            toneless_key: "gua",
            hanzi: "我",
            tl: "guá",
            syll: 1,
            freq: 80_000,
        },
        Row {
            toneless_key: "si",
            hanzi: "是",
            tl: "sī",
            syll: 1,
            freq: 80_000,
        },
        Row {
            toneless_key: "hoogua",
            hanzi: "予我",
            tl: "hōo--guá",
            syll: 2,
            freq: 16,
        },
    ]
}

fn install_fixture() {
    let rows = fixture_rows();
    let dict_path = write_temp("dictionary.bin", &build_tkdb_v3(&rows));
    let fst_path = build_dictionary_fst(&rows);
    let assoc_path = write_temp("association.bin", &empty_association_bin());
    let syllables_path = build_syllables_fst(&["hoo7", "gua2", "si7", "lai5"]);
    install_lexicon(&fst_path, &dict_path, &assoc_path, &syllables_path);
}

fn fetch(raw: &str, input_mode: &str, hyphenless: bool) -> Vec<Cell> {
    let cfg = AppConfig {
        hyphenless_roman: hyphenless,
        ..config(input_mode)
    };
    fetch_cells(&cfg, raw, FetchAtPos::default())
}

#[test]
fn typed_separator_joins_the_two_word_reading() {
    let _lock = engine_install_lock();
    install_fixture();
    for (raw, roman) in [
        ("guasi", "guá sī"),
        ("gua-si", "guá-sī"),
        ("gua--si", "guá--sī"),
    ] {
        let cells = fetch(raw, "tl", false);
        let cell = cell_with_hanji(&cells, "我是");
        assert_eq!(cell.1, roman, "{raw}: rendered roman");
        assert_eq!(cell.2, "我是", "{raw}: display_text (詞頻 key)");
        assert_eq!(
            cell.3, "guá sī",
            "{raw}: canonical_tl (identity) never follows the typed separator"
        );
        assert_eq!(cells[0].1, raw, "{raw}: the §34 literal is what was typed");
    }
}

#[test]
fn typed_separator_never_overrides_a_dictionary_word() {
    let _lock = engine_install_lock();
    install_fixture();
    // The untyped `hoogua` promotion is pinned by `continuous_slot0_dict_roman`.
    // §52: the typed `--` is the khinsiann 予我 carries, so the word (and its
    // own form) wins; a plain `-` is a different boundary kind and the word
    // is not offered under it — the typed join stands.
    let cells = fetch("hoo--gua", "tl", false);
    let cell = cell_with_hanji(&cells, "予我");
    assert_eq!(cell.1, "hōo--guá", "the record's own khinsiann form");
    assert_eq!(cell.3, "hōo--guá", "the record's identity");
    assert!(
        !cells.iter().any(|c| c.1 == "hōo guá" || c.1 == "hōo-guá"),
        "no synth join beside the dictionary word; got {cells:?}"
    );
    let cells = fetch("hoo-gua", "tl", false);
    assert!(
        !cells.iter().any(|c| c.3 == "hōo--guá"),
        "a plain `-` never reads the khinsiann record; got {cells:?}"
    );
    // The walker's 予 + 我 keeps the pair's hanji with the typed join.
    assert_eq!(cells[1].0.as_deref(), Some("予我"), "got {cells:?}");
    assert_eq!(cells[1].1, "hōo-guá", "the typed join; got {cells:?}");
}

#[test]
fn typed_separator_joins_the_all_oov_reading() {
    let _lock = engine_install_lock();
    install_fixture();
    let cells = fetch("lailai", "tl", false);
    assert!(
        cells.iter().any(|c| c.0.is_none() && c.1 == "lai lai"),
        "untyped OOV join stays the space; got {cells:?}"
    );
    let cells = fetch("lai-lai", "tl", false);
    // The typed join reads exactly like the §34 literal, which absorbs the
    // identical bare-roman synth (`dispatch::handle_fetch_at_pos`) — one
    // `lai-lai` cell, no `lai lai` cell.
    assert_eq!(
        cells
            .iter()
            .filter(|c| c.0.is_none() && c.1 == "lai-lai")
            .count(),
        1,
        "one typed-join cell; got {cells:?}"
    );
    assert!(
        !cells.iter().any(|c| c.1 == "lai lai"),
        "no space join once a separator was typed; got {cells:?}"
    );
}

#[test]
fn typed_separator_renders_under_poj() {
    let _lock = engine_install_lock();
    install_fixture();
    let cells = fetch("goa--si", "poj", false);
    let cell = cell_with_hanji(&cells, "我是");
    assert_eq!(cell.1, "góa--sī", "POJ spelling, typed khinsiann");
    assert_eq!(cell.3, "guá sī", "canonical TL keeps the space");
}

#[test]
fn typed_separator_follows_the_hyphenless_setting() {
    let _lock = engine_install_lock();
    install_fixture();
    let cells = fetch("gua--si", "tl", true);
    assert_eq!(cell_with_hanji(&cells, "我是").1, "guá\u{00b7}sī");
    let cells = fetch("gua-si", "tl", true);
    assert_eq!(cell_with_hanji(&cells, "我是").1, "guásī");
    assert_eq!(cells[0].1, "gua-si", "the literal keeps the typed hyphen");
}
