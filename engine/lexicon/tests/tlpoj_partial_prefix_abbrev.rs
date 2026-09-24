//! Regression: TL/POJ continuous partial-prefix must surface single-char
//! readings for a single-initial input (e.g. `s`), not only the `sa`-family +
//! multi-syllable phrases (surfaced 2026-06-16 while building the
//! cross-mode parity test: typing `s` in TL/POJ buried 是/sī, the highest-
//! freq `s` word, behind a wall of `tl:sb`-keyed 2-syllable phrases).
//!
//! Two mechanisms keep the singles alive. `lookup_prefix_shortest_first`
//! spends the hydrate budget on the SHORTEST matched keys first (the `0xFF`
//! wire separator byte-sorts a short exact key AFTER its longer
//! extensions), and — since §46 — the acronym `*_abbrev` keys live in their
//! own `tl-abbrev:` / `poj-abbrev:` family, so a `tl:s` scan never meets
//! the `sb` flood at all (the earlier `skip` predicate is gone).

use lexicon::dictionary_reader::DictionaryReader;
use lexicon::fetch_partial_prefix_candidates;
use phonetics::InputMode;
use ranking::FrequencyMap;

mod common;
use common::{build_tkdb_v3, build_wire_index, hanji_of, neutral_ctx, write_temp};

/// Shortest-first ordering: the short exact key comes before its longer
/// extension even though plain byte order puts it last.
#[test]
fn lookup_prefix_shortest_first_orders_short_before_long() {
    let idx = build_wire_index("tlpoj-mech", &[("tl:si", 2), ("tl:sigi", 3)]);
    assert_eq!(
        idx.lookup_prefix("tl:s"),
        vec![3, 2],
        "byte order: long first"
    );
    assert_eq!(
        idx.lookup_prefix_shortest_first("tl:s", 100),
        vec![2, 3],
        "short `si` ordered before long `sigi`"
    );
    assert_eq!(idx.lookup_prefix_shortest_first("tl:s", 1), vec![2]);
}

/// End-to-end cap regression: a flood of 2-syllable rows whose acronym is
/// `sb` must NOT starve the single-char `si` readings out of the
/// `PARTIAL_PREFIX_HYDRATE_CAP` budget. The acronym keys sit under
/// `<family>-abbrev:` and are invisible to the `<family>:s` scan; the
/// phrases' own readings (`sabu`) are longer than `si` and lose the
/// shortest-first budget. Parametrised over TL and POJ.
#[test]
fn tlpoj_partial_prefix_surfaces_single_chars_past_acronym_flood() {
    for (family, mode) in [("tl", InputMode::Tl), ("poj", InputMode::Poj)] {
        // > PARTIAL_PREFIX_HYDRATE_CAP (500).
        const FLOOD: usize = 600;

        let phrase_hanji: Vec<String> = (0..FLOOD).map(|i| format!("詞{i}")).collect();
        let mut dict_rows: Vec<(u16, u32, u8, &str, &str)> = Vec::with_capacity(FLOOD + 3);
        for hanji in &phrase_hanji {
            dict_rows.push((1u16 << 11, 10, 2, hanji.as_str(), "sa-bu"));
        }
        dict_rows.push((1u16 << 11, 9000, 7, "是", "si"));
        dict_rows.push((1u16 << 11, 8000, 5, "時", "si"));
        dict_rows.push((1u16 << 11, 7000, 2, "四", "si"));

        let dict_path = write_temp(
            "tlpoj-abbrev-flood.dict.bin",
            &build_tkdb_v3(b"TKDB", &dict_rows),
        );
        let dict = DictionaryReader::open(&dict_path).expect("dict.bin opens");

        let toneless = format!("{family}:sabu");
        let acronym = format!("{family}-abbrev:sb");
        let single = format!("{family}:si");
        let mut keys: Vec<(&str, u32)> = Vec::with_capacity(FLOOD * 2 + 3);
        for i in 0..FLOOD {
            let rowid = (i + 1) as u32;
            keys.push((acronym.as_str(), rowid));
            keys.push((toneless.as_str(), rowid));
        }
        for off in 0..3 {
            keys.push((single.as_str(), (FLOOD + off + 1) as u32));
        }
        let idx = build_wire_index("tlpoj-flood", &keys);

        let freq = FrequencyMap::new();
        let ctx = neutral_ctx(&idx, &dict, &freq, mode);
        let key = ((0u32, 1u32), format!("{family}:s"));
        let out = fetch_partial_prefix_candidates(&key, 1, &ctx);

        let hanji = hanji_of(&out);
        for want in ["是", "時", "四"] {
            assert!(
                hanji.contains(&want),
                "{mode:?}: single-char {want} must surface past the acronym flood (got {hanji:?})"
            );
        }
    }
}
