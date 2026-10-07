//! ㆳ (U+31B3) is ㆪ (U+31AA, `inn`) encoded a second time — Unicode keeps
//! one code point per writing direction for nasalized `i` (L2/18-052). The
//! dictionary holds only ㆪ, and the mobile TPS keyboards offer ㆳ on ㆪ's
//! long-press, so every lookup folds ㆳ onto ㆪ
//! (`phonetics::fold_tps_glyph_alias`) while the preedit keeps the typed
//! glyph. Checked over the production lexicon.

use composing::{requests, Engine, Intent};
use protos::engine::composing_request::Method;
use protos::engine::{CandidateMessage, TpsKey};

use crate::common::{
    config, config_converting, converted_words, fetch_at_pos_response, production_lexicon_ready,
    req, Fetch,
};

/// Whole candidate messages, not `common::fetch_cells`: parity covers spans,
/// syllable counts and scores too.
fn candidates(raw: &str) -> Vec<CandidateMessage> {
    fetch_at_pos_response(&config("tps"), raw, Fetch::default())
        .continuous
        .map(|continuous| continuous.candidates)
        .unwrap_or_default()
}

#[test]
fn either_glyph_fetches_the_same_candidates() {
    if !production_lexicon_ready() {
        eprintln!("production artifacts absent — run `make dict`; skipping.");
        return;
    }
    // trace (candidate_dump, DUMP_MODE=tps): bare ㆪ → 23 (the §35 bare-glyph
    // inventory check reads the folded glyph); ㄒㆪ → 14 (生 姓 性 …, sinn);
    // ㄊㆪˊ → 瞪 thînn (toned key); ㄏㆪㄍㄚ → 耳共 hīnn kā (multi-syllable
    // lattice); ㄍㄠ␣ㄒㆪ → 交生 (separator space); ㆪㄚ → 嬰仔 inn-á (ㆪ
    // opening the buffer); ㄊㆪ␣ㄎㄧ˪ → 天氣. Each glyph is 3 bytes, so the
    // consumed spans agree byte for byte.
    for inn in [
        "ㆪ",
        "ㄒㆪ",
        "ㄊㆪˊ",
        "ㄏㆪㄍㄚ",
        "ㄍㄠ ㄒㆪ",
        "ㆪㄚ",
        "ㄊㆪ ㄎㄧ˪",
    ] {
        let innn = inn.replace('ㆪ', "ㆳ");
        let expected = candidates(inn);
        assert!(!expected.is_empty(), "{inn:?} has no candidates");
        assert_eq!(candidates(&innn), expected, "{innn:?} vs {inn:?}");
    }
}

#[test]
fn the_preedit_keeps_the_typed_glyph_while_palatalization_sees_through_it() {
    if !production_lexicon_ready() {
        return;
    }
    // trace: tps_adjust palatalization — ㄙ before ㄧ / ㆪ turns ㄒ; ㆳ folds to
    // ㆪ for the trigger, and the incoming ㆳ itself is typed as is.
    let config = config("tps");
    let mut engine = Engine::new();
    let mut preedit = None;
    for key in ["ㄙ", "ㆳ"] {
        let request = req(Method::TpsKey(TpsKey { key: key.into() }));
        preedit = requests::handle(&request, &mut engine, &config)
            .expect("TpsKey")
            .preedit;
    }
    assert_eq!(preedit.expect("preedit").raw_input, "ㄒㆳ");
}

#[test]
fn the_hanji_conversion_reads_either_glyph() {
    if !production_lexicon_ready() {
        return;
    }
    // trace: tps_hanji_conversion_prod — ㄊㆪ␣ㄎㄧ˪ closes as 天氣, one edge
    // over the whole 15-byte buffer.
    let config = config_converting("tps");
    let convert = |raw: &str| {
        let mut engine = Engine::new();
        engine.apply(Intent::Start { text: raw.into() }, &config);
        converted_words(&engine)
    };
    let expected = vec![((0, 15), "天氣".to_string())];
    assert_eq!(convert("ㄊㆪ ㄎㄧ˪"), expected);
    assert_eq!(convert("ㄊㆳ ㄎㄧ˪"), expected);
}
