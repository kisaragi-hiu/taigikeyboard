//! Learned phrases (§50) — a phrase the user composed segment by segment
//! in continuous input becomes a whole-buffer candidate next time.
//!
//! Two halves, pinned together here:
//!
//! 1. **Learning** — the final `CommitContinuous` of a composition whose
//!    every nailed segment was a hanji pick emits `Effect.PhraseLearned`
//!    with the concatenated hanji and the `-`-joined canonical TL. Nothing
//!    is emitted mid-commit, for a single segment, for a hanji-less
//!    segment, or past six syllables.
//! 2. **Recall** — a `FetchAtPos.learned_entries` row whose key equals the
//!    typed buffer leads the hanji candidates when the dictionary has no
//!    word under that key, competes (and loses) against a dictionary
//!    homophone unless `user_frequency` prefers it, dedupes against the
//!    same pair from the dictionary, and never outranks a manual custom
//!    row (Codex 2026-09-20 F5: learned = competitor, custom = override).
//!
//! Fixture mirrors the USER report (2026-09-20): 記起來 `kì--khí-lâi` is a
//! 台日-only dictionary word (default off), so a default user composes it as
//! 記 `ki` + 起來 `khilai`; the dictionary here carries 機/ki, 記/kì, 起來 and
//! NOT 記起來.

use composing::{Engine, Intent, Phase};
use protos::engine::effect::Kind;
use protos::engine::{CustomDictEntry, FetchAtPos, FrequencyEntry, LearnedEntry, PhraseLearned};

mod common;
use common::{
    build_dictionary_fst, build_syllables_fst, build_tkdb_v3, config_tl, effect_kinds,
    empty_association_bin, engine_install_lock, fetch_hanji, install_lexicon, write_temp, Row,
};

// ---- Learning ---------------------------------------------------------------

fn engine_in_continuous(raw: &str) -> Engine {
    let mut e = Engine::new();
    e.apply(
        Intent::Start {
            text: raw.to_string(),
        },
        &config_tl(),
    );
    e.apply(Intent::EnterContinuous, &config_tl());
    e
}

fn pick(hanji: Option<&str>, tl: &str, consumed_bytes: usize, syllable_count: u8) -> Intent {
    Intent::CommitContinuous {
        display_text: hanji.unwrap_or(tl).to_string(),
        canonical_text: hanji.unwrap_or(tl).to_string(),
        association_tl: tl.to_string(),
        hanji: hanji.map(str::to_string),
        consumed_bytes,
        syllable_count,
    }
}

fn learned_effect(resp: &protos::engine::ComposingResponse) -> Option<PhraseLearned> {
    resp.effect.iter().find_map(|e| match e.kind.as_ref() {
        Some(Kind::PhraseLearned(p)) => Some(p.clone()),
        _ => None,
    })
}

#[test]
fn final_commit_of_hanji_picks_learns_the_joined_phrase() {
    let mut e = engine_in_continuous("kikhilai");
    let mid = e.apply(pick(Some("記"), "kì", 2, 1), &config_tl());
    assert!(
        learned_effect(&mid).is_none(),
        "mid-commit must not learn; got {:?}",
        effect_kinds(&mid.effect)
    );
    assert!(matches!(e.snapshot_state().phase, Phase::Continuous { .. }));

    let fin = e.apply(pick(Some("起來"), "khí-lâi", 6, 2), &config_tl());
    assert_eq!(
        effect_kinds(&fin.effect),
        vec![
            "CommitTextReplacingPreedit",
            "ResetAutocomplete",
            "ResetAutocompleteContext",
            "NextWordWordSelected",
            "PhraseLearned",
        ]
    );
    assert_eq!(
        learned_effect(&fin),
        Some(PhraseLearned {
            hanji: "記起來".into(),
            canonical_tl: "kì-khí-lâi".into(),
            syllable_count: 3,
        })
    );
    assert!(matches!(e.snapshot_state().phase, Phase::Idle));
}

#[test]
fn khinsiann_segment_keeps_its_double_hyphen() {
    // 記 + --起來: the second piece already opens with the neutral-tone
    // marker, so the join must not add a third hyphen.
    let mut e = engine_in_continuous("kikhilai");
    e.apply(pick(Some("記"), "kì", 2, 1), &config_tl());
    let fin = e.apply(pick(Some("起來"), "--khí-lâi", 6, 2), &config_tl());
    assert_eq!(
        learned_effect(&fin).map(|p| p.canonical_tl),
        Some("kì--khí-lâi".to_string())
    );
}

#[test]
fn three_single_picks_learn_too() {
    let mut e = engine_in_continuous("kikhilai");
    e.apply(pick(Some("記"), "kì", 2, 1), &config_tl());
    e.apply(pick(Some("起"), "khí", 3, 1), &config_tl());
    let fin = e.apply(pick(Some("來"), "lâi", 3, 1), &config_tl());
    assert_eq!(
        learned_effect(&fin),
        Some(PhraseLearned {
            hanji: "記起來".into(),
            canonical_tl: "kì-khí-lâi".into(),
            syllable_count: 3,
        })
    );
}

#[test]
fn single_segment_commit_learns_nothing() {
    let mut e = engine_in_continuous("khilai");
    let fin = e.apply(pick(Some("起來"), "khí-lâi", 6, 2), &config_tl());
    assert!(learned_effect(&fin).is_none());
    assert!(matches!(e.snapshot_state().phase, Phase::Idle));
}

#[test]
fn a_hanji_less_segment_blocks_learning() {
    // §34 literal / OOV pick in the middle: the platform sends no `hanji`.
    let mut e = engine_in_continuous("kikhilai");
    e.apply(pick(None, "kì", 2, 1), &config_tl());
    let fin = e.apply(pick(Some("起來"), "khí-lâi", 6, 2), &config_tl());
    assert!(
        learned_effect(&fin).is_none(),
        "got {:?}",
        effect_kinds(&fin.effect)
    );
    // Empty hanji is treated as absent, not as a pick.
    let mut e = engine_in_continuous("kikhilai");
    e.apply(pick(Some(""), "kì", 2, 1), &config_tl());
    let fin = e.apply(pick(Some("起來"), "khí-lâi", 6, 2), &config_tl());
    assert!(learned_effect(&fin).is_none());
}

#[test]
fn missing_canonical_tl_blocks_learning() {
    // Legacy caller / TPS-OOV: `association_tl` empty → no key to learn under.
    let mut e = engine_in_continuous("kikhilai");
    e.apply(pick(Some("記"), "", 2, 1), &config_tl());
    let fin = e.apply(pick(Some("起來"), "khí-lâi", 6, 2), &config_tl());
    assert!(learned_effect(&fin).is_none());
}

#[test]
fn seven_syllables_is_a_clause_not_a_word() {
    let raw = "abcdefg";
    let mut e = engine_in_continuous(raw);
    for (i, ch) in raw.chars().enumerate() {
        let hanji = format!("字{i}");
        let is_last = i + 1 == raw.len();
        let fin = e.apply(pick(Some(&hanji), &ch.to_string(), 1, 1), &config_tl());
        if is_last {
            assert!(learned_effect(&fin).is_none(), "7 syllables must not learn");
        }
    }
    // Six syllables still learn.
    let raw = "abcdef";
    let mut e = engine_in_continuous(raw);
    let mut last = None;
    for (i, ch) in raw.chars().enumerate() {
        let hanji = format!("字{i}");
        last = Some(e.apply(pick(Some(&hanji), &ch.to_string(), 1, 1), &config_tl()));
    }
    let learned = learned_effect(last.as_ref().unwrap()).expect("6 syllables learn");
    assert_eq!(learned.syllable_count, 6);
    assert_eq!(learned.canonical_tl, "a-b-c-d-e-f");
}

// ---- Recall -----------------------------------------------------------------

const NOW_MS: i64 = 1_700_000_000_000;

fn fixture_rows() -> Vec<Row> {
    vec![
        Row {
            toneless_key: "ki",
            hanzi: "機",
            tl: "ki",
            syll: 1,
            freq: 6318,
        },
        Row {
            toneless_key: "ki",
            hanzi: "記",
            tl: "kì",
            syll: 1,
            freq: 2000,
        },
        Row {
            toneless_key: "khilai",
            hanzi: "起來",
            tl: "khí-lâi",
            syll: 2,
            freq: 3000,
        },
        Row {
            toneless_key: "khi",
            hanzi: "起",
            tl: "khí",
            syll: 1,
            freq: 16425,
        },
        Row {
            toneless_key: "lai",
            hanzi: "來",
            tl: "lâi",
            syll: 1,
            freq: 30000,
        },
    ]
}

fn install(rows: &[Row]) {
    let dict_path = write_temp("dictionary.bin", &build_tkdb_v3(rows));
    let fst_path = build_dictionary_fst(rows);
    let assoc_path = write_temp("association.bin", &empty_association_bin());
    let syllables_path = build_syllables_fst(&["ki1", "ki3", "khi2", "lai5"]);
    install_lexicon(&fst_path, &dict_path, &assoc_path, &syllables_path);
}

fn learned(hanji: &str, canonical_tl: &str) -> LearnedEntry {
    LearnedEntry {
        hanji: hanji.into(),
        canonical_tl: canonical_tl.into(),
        learn_count: 1,
    }
}

fn selected(hanji: &str, canonical_tl: &str, count: u32) -> FrequencyEntry {
    FrequencyEntry {
        display_text_key: hanji.into(),
        count,
        last_used_ms: NOW_MS - 1_000,
        canonical_tl: canonical_tl.into(),
    }
}

fn hanji_with(raw: &str, mode: &str, fetch: FetchAtPos) -> Vec<String> {
    fetch_hanji(raw, mode, fetch)
}

#[test]
fn learned_phrase_leads_when_the_dictionary_has_no_word_under_the_key() {
    let _lock = engine_install_lock();
    install(&fixture_rows());
    let cold = hanji_with("kikhilai", "tl", FetchAtPos::default());
    assert_eq!(
        cold[0], "機起來",
        "cold start synthesizes the split; got {cold:?}"
    );
    assert!(!cold.contains(&"記起來".to_string()));

    let hanji = hanji_with(
        "kikhilai",
        "tl",
        FetchAtPos {
            learned_entries: vec![learned("記起來", "kì-khí-lâi")],
            ..Default::default()
        },
    );
    assert_eq!(hanji[0], "記起來", "learned phrase leads; got {hanji:?}");
    assert_eq!(
        hanji.iter().filter(|h| *h == "記起來").count(),
        1,
        "walker slot 0 and the span-local row collapse to one; got {hanji:?}"
    );
}

#[test]
fn learned_phrase_with_khinsiann_key_matches_the_toneless_buffer() {
    let _lock = engine_install_lock();
    install(&fixture_rows());
    let hanji = hanji_with(
        "kikhilai",
        "tl",
        FetchAtPos {
            learned_entries: vec![learned("記起來", "kì--khí-lâi")],
            ..Default::default()
        },
    );
    assert_eq!(hanji[0], "記起來", "got {hanji:?}");
}

#[test]
fn learned_phrase_matches_a_poj_typed_buffer() {
    // The learned row is canonical TL; the POJ user types the POJ spelling.
    let _lock = engine_install_lock();
    let rows = vec![
        Row {
            toneless_key: "tshia",
            hanzi: "車",
            tl: "tshia",
            syll: 1,
            freq: 5000,
        },
        Row {
            toneless_key: "thau",
            hanzi: "頭",
            tl: "thâu",
            syll: 1,
            freq: 9000,
        },
    ];
    let dict_path = write_temp("dictionary.bin", &build_tkdb_v3(&rows));
    let fst_path = build_dictionary_fst(&rows);
    let assoc_path = write_temp("association.bin", &empty_association_bin());
    let syllables_path = build_syllables_fst(&["tshia1", "thau5"]);
    install_lexicon(&fst_path, &dict_path, &assoc_path, &syllables_path);

    let hanji = hanji_with(
        "chhiathau",
        "poj",
        FetchAtPos {
            learned_entries: vec![learned("車頭", "tshia-thâu")],
            ..Default::default()
        },
    );
    assert_eq!(hanji[0], "車頭", "got {hanji:?}");
}

#[test]
fn dictionary_homophone_beats_a_learned_row_until_the_user_prefers_it() {
    // Same key, different word: the dictionary's 機起來-free fixture gains
    // a real 2-syllable word under `kikhilai`; the learned pair must not
    // override it (Codex F5), only compete.
    let _lock = engine_install_lock();
    let mut rows = fixture_rows();
    rows.push(Row {
        toneless_key: "kikhilai",
        hanzi: "機器來",
        tl: "ki-khì-lâi",
        syll: 3,
        freq: 12,
    });
    install(&rows);

    let hanji = hanji_with(
        "kikhilai",
        "tl",
        FetchAtPos {
            learned_entries: vec![learned("記起來", "kì-khí-lâi")],
            ..Default::default()
        },
    );
    assert_eq!(
        hanji[0], "機器來",
        "dictionary word keeps the edge; got {hanji:?}"
    );
    assert!(
        hanji.contains(&"記起來".to_string()),
        "learned row still listed; got {hanji:?}"
    );

    // One pick of the learned phrase → it leads (user weight is the
    // leading SortKey dimension, same as #69).
    let hanji = hanji_with(
        "kikhilai",
        "tl",
        FetchAtPos {
            learned_entries: vec![learned("記起來", "kì-khí-lâi")],
            frequency_entries: vec![selected("記起來", "kì-khí-lâi", 1)],
            now_ms: NOW_MS,
            ..Default::default()
        },
    );
    assert_eq!(hanji[0], "記起來", "got {hanji:?}");
    assert_eq!(hanji[1], "機器來", "got {hanji:?}");
}

#[test]
fn learned_pair_the_dictionary_also_carries_is_listed_once_from_the_dictionary() {
    let _lock = engine_install_lock();
    let mut rows = fixture_rows();
    rows.push(Row {
        toneless_key: "kikhilai",
        hanzi: "記起來",
        tl: "kì-khí-lâi",
        syll: 3,
        freq: 12,
    });
    install(&rows);
    let hanji = hanji_with(
        "kikhilai",
        "tl",
        FetchAtPos {
            learned_entries: vec![learned("記起來", "kì-khí-lâi")],
            ..Default::default()
        },
    );
    assert_eq!(hanji[0], "記起來", "got {hanji:?}");
    assert_eq!(
        hanji.iter().filter(|h| *h == "記起來").count(),
        1,
        "got {hanji:?}"
    );
}

#[test]
fn manual_custom_row_outranks_a_learned_row_under_the_same_key() {
    let _lock = engine_install_lock();
    install(&fixture_rows());
    let hanji = hanji_with(
        "kikhilai",
        "tl",
        FetchAtPos {
            custom_entries: vec![CustomDictEntry {
                roman: "kì-khí-lâi".into(),
                hanji: Some("既起來".into()),
            }],
            learned_entries: vec![learned("記起來", "kì-khí-lâi")],
            ..Default::default()
        },
    );
    assert_eq!(
        hanji[0], "既起來",
        "custom override wins the edge; got {hanji:?}"
    );
    assert!(
        hanji.contains(&"記起來".to_string()),
        "learned row still listed; got {hanji:?}"
    );
}

#[test]
fn empty_or_half_empty_learned_rows_are_ignored() {
    let _lock = engine_install_lock();
    install(&fixture_rows());
    let hanji = hanji_with(
        "kikhilai",
        "tl",
        FetchAtPos {
            learned_entries: vec![learned("", "kì-khí-lâi"), learned("記起來", "")],
            ..Default::default()
        },
    );
    assert_eq!(hanji[0], "機起來", "got {hanji:?}");
}
