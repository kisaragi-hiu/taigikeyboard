//! Shared fixture builders for lexicon integration tests.
//!
//! The `tests/it.rs` binary root declares `mod common;` once and its
//! test-file modules reach the helpers as `crate::common` — they never leak
//! into the production crate.

#![allow(dead_code)] // Different test files use different subsets.

use std::path::PathBuf;

use lexicon::{
    fetch_candidates_for_keys_with_barriers, ConsumedSpan, ContinuousFetchCtx, RawCandidate,
};
use phonetics::InputMode;
use test_support::{build_tkdb, fst_entry, write_fst_set, TkdbRow};

/// `(key, rowid)` → FST `key || 0xFF || rowid_le_4` (`key` carries its
/// family prefix, `tl:tsua`). `lookup_exact` resolves `rowid`;
/// `DictionaryReader::record` is 1-based, so FST rowid `N` maps to
/// `build_tkdb_v3` row index `N-1`.
pub fn write_synthetic_fst(name: &str, pairs: &[(&str, u32)]) -> PathBuf {
    write_fst_set(
        name,
        pairs
            .iter()
            .map(|(key, rowid)| fst_entry(b"", key, *rowid))
            .collect(),
    )
}

/// 4-tuple convenience for v2 fixtures: `(bitmask, frequency, syllable_count,
/// hanzi, tl)`. Emits a VERSION-2 binary (no subtag) — used only by the
/// v2-loud-reject test now that the reader requires v3.
pub fn build_tkdb_v2(magic: &[u8; 4], rows: &[(u16, u32, u8, &str, &str)]) -> Vec<u8> {
    let dict_rows: Vec<TkdbRow<'_>> = rows
        .iter()
        .map(|(bm, freq, syll, hanzi, tl)| TkdbRow {
            bitmask: *bm,
            frequency: *freq,
            syllable_count: Some(*syll),
            kautian_subtag: None,
            hanzi,
            tl,
        })
        .collect();
    build_tkdb(magic, 2, &dict_rows)
}

/// 5-tuple convenience for v3 fixtures: `(bitmask, frequency, syllable_count,
/// hanzi, tl)` with `kautian_subtag = 0` on every row. The default for tests
/// that don't exercise subcollection provenance. Delegates to
/// `build_tkdb_v3_subtag` (mirrors the `build_tkdb_v2` → `build_tkdb`
/// thin-wrapper pattern).
pub fn build_tkdb_v3(magic: &[u8; 4], rows: &[(u16, u32, u8, &str, &str)]) -> Vec<u8> {
    let with_subtag: Vec<(u16, u32, u8, u16, &str, &str)> = rows
        .iter()
        .map(|(bm, freq, syll, hanzi, tl)| (*bm, *freq, *syll, 0u16, *hanzi, *tl))
        .collect();
    build_tkdb_v3_subtag(magic, &with_subtag)
}

/// 6-tuple convenience for v3 fixtures with explicit kautian subtags:
/// `(bitmask, frequency, syllable_count, kautian_subtag, hanzi, tl)`.
pub fn build_tkdb_v3_subtag(magic: &[u8; 4], rows: &[(u16, u32, u8, u16, &str, &str)]) -> Vec<u8> {
    let dict_rows: Vec<TkdbRow<'_>> = rows
        .iter()
        .map(|(bm, freq, syll, subtag, hanzi, tl)| TkdbRow {
            bitmask: *bm,
            frequency: *freq,
            syllable_count: Some(*syll),
            kautian_subtag: Some(*subtag),
            hanzi,
            tl,
        })
        .collect();
    build_tkdb(magic, 3, &dict_rows)
}

/// Test-only span-local fetch: every dictionary candidate whose toneless
/// key matches `input[pos..end]` for some `end` in `endings`. Production
/// never uses this shape — `composing::continuous::assemble_candidates`
/// builds its `(consumed_span, "<prefix>:<toneless>")` pairs itself and
/// calls `lexicon::fetch_candidates_for_keys_with_barriers` directly. This
/// wrapper lets the tests here hand over pre-computed syllabifier endings
/// without rebuilding the pairs inline.
///
/// `prefix ∈ {tl, poj, tps}` follows `mode` (English shares `tl:`). ASCII
/// digits are stripped from the span — the digit half of the upstream
/// `dictionary/common/notone.py::remove_tone` regex `[\d\-]` — so numeric-
/// tone input (`tai1bak4`) still hits the fused toneless FST key
/// (`tl:taipak`). Hyphens are NOT stripped: production folds them upstream
/// (`composing::shadow::build_hyphen_shadow`), so a hyphen reaching this fn
/// is an upstream contract violation and the FST lookup correctly misses.
///
/// Always forces `custom = &[]` (the lexicon integration tests never carry
/// custom-dict matches; the Item 12 tests in `span_local_fetch.rs` call the
/// production entry directly for that). Pass `&FrequencyMap::new()` +
/// `now_ms = 0` for cold-start neutral ranking (user_weight = 0.0
/// everywhere).
pub fn fetch_candidates_for_endings(
    input: &str,
    pos: usize,
    endings: &[usize],
    mode: InputMode,
    ctx: &ContinuousFetchCtx<'_>,
) -> Vec<RawCandidate> {
    if endings.is_empty() || pos >= input.len() || !input.is_char_boundary(pos) {
        return Vec::new();
    }

    let lower = input.to_ascii_lowercase();
    let mut keys: Vec<(ConsumedSpan, String)> = Vec::with_capacity(endings.len());
    let prefix = match mode {
        InputMode::Poj => "poj",
        InputMode::Tps => "tps",
        InputMode::Tl | InputMode::English => "tl",
    };
    for &end in endings {
        if end <= pos || end > lower.len() || !lower.is_char_boundary(end) {
            continue;
        }
        let segment = &lower[pos..end];
        let toneless: String = segment.chars().filter(|c| !c.is_ascii_digit()).collect();
        if toneless.is_empty() {
            continue;
        }
        keys.push(((pos as u32, end as u32), format!("{prefix}:{toneless}")));
    }

    // `raw_len` is the full `input.len()` even when `pos != 0`: Tier 1 is
    // full-buffer coverage, not `input.len() - pos`. Every ctx field is
    // listed explicitly (not `..*ctx`) so the forced-empty-custom contract
    // stays loud if a non-`Copy` field is ever added.
    let inner = ContinuousFetchCtx {
        enabled_sources_bitmask: ctx.enabled_sources_bitmask,
        freq_map: ctx.freq_map,
        now_ms: ctx.now_ms,
        custom: &[],
        learned: &[],
        prefix_index: ctx.prefix_index,
        dict: ctx.dict,
        mode: ctx.mode,
        context: ctx.context,
        tone_pin: ctx.tone_pin.clone(),
    };
    fetch_candidates_for_keys_with_barriers(&keys, &[], &[], input.len() as u32, &inner)
}

/// [`write_synthetic_fst`] opened as a `PrefixIndex`.
pub fn build_wire_index(name: &str, entries: &[(&str, u32)]) -> lexicon::prefix_index::PrefixIndex {
    lexicon::prefix_index::PrefixIndex::open(&write_synthetic_fst(name, entries))
        .expect("open index")
}

/// A [`ContinuousFetchCtx`] with every source on, no user frequency, no
/// custom entries and no tone pin — the neutral shape most lexicon tests
/// start from; mutate the fields a test cares about after.
pub fn neutral_ctx<'a>(
    prefix_index: &'a lexicon::prefix_index::PrefixIndex,
    dict: &'a lexicon::dictionary_reader::DictionaryReader,
    freq_map: &'a ranking::FrequencyMap,
    mode: InputMode,
) -> ContinuousFetchCtx<'a> {
    ContinuousFetchCtx {
        enabled_sources_bitmask: u32::MAX,
        freq_map,
        now_ms: 0,
        custom: &[],
        learned: &[],
        prefix_index,
        dict,
        mode,
        context: ranking::ContextRanks::empty(),
        tone_pin: lexicon::TonePin::None,
    }
}

/// The hanji of `candidates` in order (roman-only rows dropped).
pub fn hanji_of(candidates: &[lexicon::RawCandidate]) -> Vec<&str> {
    candidates
        .iter()
        .filter_map(|c| c.hanji.as_deref())
        .collect()
}

/// One `user_frequency.db` row, as the engine ranks it.
pub struct FrequencyFixture {
    pub display_text_key: String,
    pub count: i32,
    pub last_used_ms: i64,
    pub canonical_tl: String,
}

/// The ranking map for `rows`; a later row for the same pair wins.
pub fn frequency_map(rows: &[FrequencyFixture]) -> ranking::FrequencyMap {
    rows.iter()
        .map(|row| {
            let data = ranking::FrequencyData {
                count: row.count,
                last_used_ms: row.last_used_ms,
            };
            (row.display_text_key.clone(), row.canonical_tl.clone(), data)
        })
        .collect()
}
