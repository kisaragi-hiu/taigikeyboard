//! Hanji conversion in the TPS preedit over the production lexicon
//! (`docs/architecture/desktop-tps-hanji-conversion-roadmap.md` H1): the
//! sentence the plan was grounded on (`candidate_dump`, `DUMP_MODE=tps`,
//! 2026-10-05) converts as its readings close.

use composing::{requests, Engine, Intent};
use protos::engine::composing_request::Method;
use protos::engine::TpsKey;

use crate::common::{config_converting, converted_words, production_lexicon_ready, req};

// trace (candidate_dump slot 0, the same neutral walk): `ㄍㄧㄣ ㄚˋ` → roman
// `kin á`, two edges: 今 (0, 10) with the separator that closes it, and
// 仔 (10, 15). Closing ㆢㄧㆵ˙ (11 bytes) →
// `kin-á-ji̍t`, one edge (0, 26): the two words become 今仔日. The rest of
// the sentence, typed key by key, ends on 今仔日天氣真好; before ㄏㄛ's tone
// mark the reading is open and shows as glyphs.
#[test]
fn a_production_sentence_converts_and_resegments_as_readings_close() {
    if !production_lexicon_ready() {
        eprintln!("production artifacts absent — run `make dict`; skipping.");
        return;
    }
    let config = config_converting("tps");
    let mut engine = Engine::new();
    let type_keys = |engine: &mut Engine, keys: &str| {
        keys.chars()
            .map(|key| {
                let request = req(Method::TpsKey(TpsKey {
                    key: key.to_string(),
                }));
                requests::handle(&request, engine, &config)
                    .expect("TpsKey")
                    .preedit
                    .expect("preedit")
            })
            .last()
            .expect("at least one key")
    };

    engine.apply(
        Intent::Start {
            text: "ㄍㄧㄣ ㄚˋ".into(),
        },
        &config,
    );
    assert_eq!(
        converted_words(&engine),
        vec![((0, 10), "今".to_string()), ((10, 15), "仔".to_string())]
    );

    let preedit = type_keys(&mut engine, "ㆢㄧㆵ˙");
    assert_eq!(preedit.raw_input, "ㄍㄧㄣ ㄚˋㆢㄧㆵ˙");
    assert_eq!(
        converted_words(&engine),
        vec![((0, 26), "今仔日".to_string())]
    );

    let preedit = type_keys(&mut engine, "ㄊㆪ ㄎㄧ˪ㄐㄧㄣ ㄏㄛ");
    assert_eq!(preedit.raw_input, "ㄍㄧㄣ ㄚˋㆢㄧㆵ˙ㄊㆪ ㄎㄧ˪ㄐㄧㄣ ㄏㄛ");
    assert_eq!(preedit.display_text, "今仔日天氣真ㄏㄛ");

    let preedit = type_keys(&mut engine, "ˋ");
    assert_eq!(preedit.display_text, "今仔日天氣真好");
    assert_eq!(preedit.caret_utf16, 7);
}
