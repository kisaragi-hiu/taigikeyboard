//! §52 one-syllable segments over the production lexicon — the reported
//! inputs (USER 2026-10-08) and the controls the mechanism predicts, so a
//! dictionary rebuild that reshapes the `sia` / `ai` key families cannot
//! silently reopen the cut. Hermetic layer tests:
//! `continuous_one_syllable_segment.rs`.

use crate::common::{fetch_hanji, production_lexicon_ready, Fetch};

fn tl_hanji(raw: &str) -> Vec<String> {
    fetch_hanji(raw, "tl", Fetch::default())
}

#[test]
fn a_one_syllable_segment_offers_no_inner_cut() {
    if !production_lexicon_ready() {
        eprintln!("production artifacts absent — run `make dict`; skipping.");
        return;
    }
    // trace (candidate_dump, before the fix): `siam-tioh` → 寫 siá / 匙仔
    // sî-á at span (0,3); `siam2-tioh8` the same through the toned
    // segment; `iam-tioh` → 也 iā (0,2) via `i`+`a`, then `m`; `ai-` → 阿姨
    // a-î under the one key `ai`.
    // `siam-2`: a digit after the `-` is no tone of `siam` (Codex post-impl
    // 2026-10-08 P2 — it read `siam` as a false toneless boundary).
    let cases: [(&str, &str, &[&str]); 5] = [
        ("siam-tioh", "閃", &["寫", "匙仔"]),
        ("siam2-tioh8", "閃", &["寫", "匙仔"]),
        ("siam-2", "閃", &["寫", "匙仔"]),
        ("iam-tioh", "鹽", &["也"]),
        ("ai-", "愛", &["阿姨", "阿依"]),
    ];
    for (raw, kept, cut) in cases {
        let hanji = tl_hanji(raw);
        assert!(
            hanji.iter().any(|h| h == kept),
            "{raw}: {kept} missing; got {hanji:?}"
        );
        for word in cut {
            assert!(
                !hanji.iter().any(|h| h == word),
                "{raw}: {word} cuts inside; got {hanji:?}"
            );
        }
    }
}

#[test]
fn controls_keep_their_inner_cuts() {
    if !production_lexicon_ready() {
        return;
    }
    // No hyphen: `sia|m|tioh` is a genuine reading. `taigi` is no single
    // syllable, so the `-` after it leaves 台 tâi at (0,3).
    for (raw, kept) in [("siamtioh", "寫"), ("ai", "阿姨"), ("taigi-bun", "台")] {
        let hanji = tl_hanji(raw);
        assert!(
            hanji.iter().any(|h| h == kept),
            "{raw}: {kept} missing; got {hanji:?}"
        );
    }
}
