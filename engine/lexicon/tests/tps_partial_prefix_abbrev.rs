//! Regression: TPS continuous partial-prefix must surface single-char
//! readings for a single-initial input (e.g. `ㄍ`), not only multi-
//! syllable phrases (user-reported 2026-06-15: typing `ㄍ` showed only
//! multi-syllable words, starting at 姑不將).
//!
//! Root cause (now fixed): the FST wire separator `0xFF` is greater than
//! any UTF-8 byte, so a short exact key (`tps:ㄍㄚ` = 家/ka) byte-sorts
//! AFTER all its longer extensions (`tps:ㄍㄚㄅㄧ`…). A plain byte-ordered
//! `lookup_prefix(..).take(cap)` front-loaded the longest (rarest) words
//! and buried the short high-frequency single-syllable readings past the
//! `PARTIAL_PREFIX_HYDRATE_CAP`. The fix spends the hydrate budget on the
//! SHORTEST matched keys first via `PrefixIndex::lookup_prefix_shortest_first`
//! (all modes); the downstream `SortKey` then ranks the pool by frequency.
//! The initial-only `tps_abbrev` acronym keys (Bopomofo orders all initials
//! ahead of all vowels, so they would otherwise win the shortest-first
//! budget) live in their own `tps-abbrev:` family since §46 and never enter
//! a `tps:` scan.

use lexicon::dictionary_reader::DictionaryReader;
use lexicon::fetch_partial_prefix_candidates;
use phonetics::InputMode;
use ranking::FrequencyMap;

use crate::common::{build_tkdb_v3, build_wire_index, hanji_of, neutral_ctx};
use test_support::write_temp;

/// Shortest-first ordering for Bopomofo keys: the short exact key comes
/// before its longer extension even though plain byte order puts it last;
/// the cap is honoured against that order.
#[test]
fn lookup_prefix_shortest_first_orders_short_first() {
    let idx = build_wire_index("tps-mech", &[("tps:ㄍㄚ", 2), ("tps:ㄍㄚㄅㄧ", 3)]);
    assert_eq!(
        idx.lookup_prefix("tps:ㄍ"),
        vec![3, 2],
        "byte order: long first"
    );
    assert_eq!(idx.lookup_prefix_shortest_first("tps:ㄍ", 100), vec![2, 3]);
    assert_eq!(idx.lookup_prefix_shortest_first("tps:ㄍ", 1), vec![2]);
}

/// The matched key is reconstructed by fixed offset (`entry.len() - 5`),
/// NOT by searching for `0xFF` — a rowid whose little-endian bytes contain
/// `0xFF` (255 → `FF 00 00 00`) must not corrupt key parsing.
#[test]
fn lookup_prefix_shortest_first_parses_key_with_0xff_in_rowid() {
    let idx = build_wire_index("tps-rowid-ff", &[("tps:ㄍㄚ", 255)]);
    assert_eq!(idx.lookup_prefix_shortest_first("tps:ㄍ", 100), vec![255]);
    assert_eq!(
        idx.lookup_prefix_shortest_first_tps_readings("tps:ㄍ", 100),
        vec![("tps:ㄍㄚ".to_string(), 255)],
        "the matched key is intact despite 0xFF in the rowid bytes"
    );
}

/// End-to-end cap regression: a flood of LONGER `ka-pi` rows (plus their
/// `ㄍㄅ` acronym keys, under `tps-abbrev:`) must NOT starve the
/// single-char `ㄍㄚ` readings out of the `PARTIAL_PREFIX_HYDRATE_CAP`
/// budget. Pre-fix, `家/加/交` were absent (only long phrases survived).
#[test]
fn tps_partial_prefix_surfaces_single_chars_past_abbrev_flood() {
    // > PARTIAL_PREFIX_HYDRATE_CAP (500).
    const FLOOD: usize = 600;

    let phrase_hanji: Vec<String> = (0..FLOOD).map(|i| format!("詞{i}")).collect();
    let mut dict_rows: Vec<(u16, u32, u8, &str, &str)> = Vec::with_capacity(FLOOD + 3);
    for hanji in &phrase_hanji {
        dict_rows.push((1u16 << 11, 10, 2, hanji.as_str(), "ka-pi"));
    }
    dict_rows.push((1u16 << 11, 9000, 1, "家", "ka"));
    dict_rows.push((1u16 << 11, 8000, 1, "加", "ka"));
    dict_rows.push((1u16 << 11, 7000, 1, "交", "ka"));

    let dict_path = write_temp(
        "tps-abbrev-flood.dict.bin",
        &build_tkdb_v3(b"TKDB", &dict_rows),
    );
    let dict = DictionaryReader::open(&dict_path).expect("dict.bin opens");

    let mut keys: Vec<(&str, u32)> = Vec::with_capacity(FLOOD * 2 + 3);
    for i in 0..FLOOD {
        let rowid = (i + 1) as u32;
        keys.push(("tps-abbrev:ㄍㄅ", rowid));
        keys.push(("tps:ㄍㄚㄅㄧ", rowid));
    }
    for off in 0..3 {
        keys.push(("tps:ㄍㄚ", (FLOOD + off + 1) as u32));
    }
    let idx = build_wire_index("tps-flood", &keys);

    let freq = FrequencyMap::new();
    let ctx = neutral_ctx(&idx, &dict, &freq, InputMode::Tps);
    let key = ((0u32, "ㄍ".len() as u32), "tps:ㄍ".to_string());
    let out = fetch_partial_prefix_candidates(&key, "ㄍ".len() as u32, &ctx);

    let hanji = hanji_of(&out);
    for want in ["家", "加", "交"] {
        assert!(
            hanji.contains(&want),
            "single-char {want} must surface past the abbrev flood (got {hanji:?})"
        );
    }
}
