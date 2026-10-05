//! The caret in a converted TPS preedit
//! (`docs/architecture/desktop-tps-hanji-conversion-roadmap.md` H3): it steps
//! over a converted word and by glyph between the words, a reading typed
//! inside the tail keeps the words on both sides until it closes, and a step
//! left from the start of the tail re-opens the last nailed segment.
//!
//! The fixture of `tps_hanji_conversion.rs` (its strict-prefix controls
//! included); the non-BMP case adds 𪜶 (in) with its strict-prefix control
//! 伊 (i).

use composing::api::{CaretDirection, CommitScript};
use composing::{Engine, Intent, Phase};
use protos::engine::{AppConfig, ComposingResponse, DictionarySourceToggles, HanjiConversion};
use test_support::engine_install_lock;

use crate::common::{config, config_converting, converted_words, effect_kinds};
use crate::tps_hanji_conversion::{
    caret_utf16, composing_engine, conversion_of, display, install_fixture_with, preedit_writes,
    raw_input, tps_key,
};

fn step(engine: &mut Engine, direction: CaretDirection, config: &AppConfig) -> ComposingResponse {
    engine.apply(
        Intent::MoveCaret {
            direction: Some(direction),
        },
        config,
    )
}

/// The raw caret the engine holds.
fn raw_caret(engine: &Engine) -> usize {
    match engine.snapshot_state().phase {
        Phase::Continuous { caret, .. } => caret,
        Phase::Idle => 0,
    }
}

fn nailed_count(engine: &Engine) -> usize {
    match engine.snapshot_state().phase {
        Phase::Continuous { nailed, .. } => nailed.len(),
        Phase::Idle => 0,
    }
}

/// A `CommitContinuous` of `hanji` (one syllable, TL `tl`) over the first
/// `consumed_bytes` of the tail.
fn pick(hanji: &str, tl: &str, consumed_bytes: usize) -> Intent {
    Intent::CommitContinuous {
        canonical_text: hanji.into(),
        association_tl: tl.into(),
        hanji: Some(hanji.into()),
        consumed_bytes,
        syllable_count: 1,
        script: Some(CommitScript::Lead),
        roman: tl.into(),
    }
}

// trace: "ㄒㄧˋㄒㄧ " → 死 (0, 8) + 詩 (8, 15), caret 15. Left: a word ends at
// 15 → its start 8 (display caret 1); again → 0; at 0 with nothing nailed →
// no-op, no effects. Right: a word starts at 0 → 8, then 15; at the end →
// no-op. The display and the words never change; a step answers one
// UpdatePreedit and no RefreshCandidates.
#[test]
fn the_caret_steps_over_a_word_and_the_words_stay() {
    let _lock = engine_install_lock();
    install_fixture_with(&[]);
    let config = config_converting("tps");
    let (mut engine, _) = composing_engine("ㄒㄧˋㄒㄧ ", &config);
    let words = converted_words(&engine);

    for (direction, raw, caret) in [
        (CaretDirection::Left, 8, 1),
        (CaretDirection::Left, 0, 0),
        (CaretDirection::Right, 8, 1),
        (CaretDirection::Right, 15, 2),
    ] {
        let response = step(&mut engine, direction, &config);
        assert_eq!(
            effect_kinds(&response.effect),
            vec!["UpdatePreedit"],
            "{direction:?}"
        );
        assert_eq!(display(&response), "死詩");
        assert_eq!(caret_utf16(&response), caret, "{direction:?}");
        assert_eq!(raw_caret(&engine), raw, "{direction:?}");
        assert_eq!(converted_words(&engine), words);
    }
    let response = step(&mut engine, CaretDirection::Right, &config);
    assert!(response.effect.is_empty(), "the end is an edge");
    step(&mut engine, CaretDirection::Left, &config);
    step(&mut engine, CaretDirection::Left, &config);
    let response = step(&mut engine, CaretDirection::Left, &config);
    assert!(response.effect.is_empty(), "the start is an edge");
    assert_eq!(raw_caret(&engine), 0);
}

// The open reading is stepped glyph by glyph. trace: "ㄒㄧˋㄒㄧ" → 死 (0, 8) +
// the open "ㄒㄧ" (8, 14), caret 14 (display 3). Left: no word ends at 14 →
// one glyph, 11 (display 2); → 8 (display 1); a word ends at 8 → 0.
#[test]
fn the_caret_steps_by_glyph_inside_the_open_reading() {
    let _lock = engine_install_lock();
    install_fixture_with(&[]);
    let config = config_converting("tps");
    let (mut engine, response) = composing_engine("ㄒㄧˋㄒㄧ", &config);
    assert_eq!(caret_utf16(&response), 3);

    for (raw, caret) in [(11, 2), (8, 1), (0, 0)] {
        let response = step(&mut engine, CaretDirection::Left, &config);
        assert_eq!(display(&response), "死ㄒㄧ");
        assert_eq!(caret_utf16(&response), caret);
        assert_eq!(raw_caret(&engine), raw);
    }
    for (raw, caret) in [(8, 1), (11, 2), (14, 3)] {
        let response = step(&mut engine, CaretDirection::Right, &config);
        assert_eq!(caret_utf16(&response), caret);
        assert_eq!(raw_caret(&engine), raw);
    }
}

// A separator in the open text has no glyph to show: stepping over it moves
// the raw caret and not the drawn one, as in a glyph preedit. trace:
// "ㄒㄧˋ- " → 死 (0, 8) + the open "- " (8, 10), caret 10 (display 2); Left →
// 9, still display 2; → 8 (display 1).
#[test]
fn a_hidden_separator_takes_a_step_without_moving_the_drawn_caret() {
    let _lock = engine_install_lock();
    install_fixture_with(&[]);
    let config = config_converting("tps");
    let (mut engine, response) = composing_engine("ㄒㄧˋ- ", &config);
    assert_eq!((display(&response), caret_utf16(&response)), ("死-", 2));

    let response = step(&mut engine, CaretDirection::Left, &config);
    assert_eq!((raw_caret(&engine), caret_utf16(&response)), (9, 2));
    let response = step(&mut engine, CaretDirection::Left, &config);
    assert_eq!((raw_caret(&engine), caret_utf16(&response)), (8, 1));
}

// A reading typed between two words keeps both until it closes (H3).
// trace: "ㄒㄧˋㄒㄧ " caret 8 (between 死 and 詩). "ㄒ" → raw "ㄒㄧˋㄒㄒㄧ ",
// caret 11: 死 is before the edit, 詩 after it, shifted to (11, 18); no
// lattice edge ends at 11 (`ㄒ` is no syllable) → kept: "死ㄒ詩". "ㄧ" →
// "死ㄒㄧ詩": the reading ends on no mark and no separator. "ˋ" → the edge
// (8, 16) ends on a mark → the whole tail is walked: 死 + 死 + 詩, the caret
// on the new boundary 16 (display 2).
#[test]
fn a_reading_typed_between_words_keeps_them_until_it_closes() {
    let _lock = engine_install_lock();
    install_fixture_with(&[]);
    let config = config_converting("tps");
    let (mut engine, _) = composing_engine("ㄒㄧˋㄒㄧ ", &config);
    step(&mut engine, CaretDirection::Left, &config);

    let response = tps_key(&mut engine, "ㄒ", &config);
    assert_eq!(raw_input(&response), "ㄒㄧˋㄒㄒㄧ ");
    assert_eq!(preedit_writes(&response), vec!["死ㄒ詩"]);
    assert_eq!(caret_utf16(&response), 2);
    assert_eq!(
        converted_words(&engine),
        vec![((0, 8), "死".to_string()), ((11, 18), "詩".to_string())]
    );

    let response = tps_key(&mut engine, "ㄧ", &config);
    assert_eq!(display(&response), "死ㄒㄧ詩");
    assert_eq!(caret_utf16(&response), 3);

    let response = tps_key(&mut engine, "ˋ", &config);
    assert_eq!(display(&response), "死死詩");
    assert_eq!(caret_utf16(&response), 2);
    assert_eq!(raw_caret(&engine), 16);
    assert_eq!(
        converted_words(&engine),
        vec![
            ((0, 8), "死".to_string()),
            ((8, 16), "死".to_string()),
            ((16, 23), "詩".to_string())
        ]
    );
}

// Space closes a reading typed inside the tail, with its tone pinned (§41).
// trace: caret 8 of "ㄒㄧˋㄒㄧ "; "ㄒ", "ㄧ", Space → raw "ㄒㄧˋㄒㄧ ㄒㄧ ", the
// separator barrier ends the reading at 15 → walk: 死 + 詩 (si1, not the
// higher-frequency 是) + 詩.
#[test]
fn space_closes_a_reading_typed_between_words_with_its_tone_pinned() {
    let _lock = engine_install_lock();
    install_fixture_with(&[]);
    let config = config_converting("tps");
    let (mut engine, _) = composing_engine("ㄒㄧˋㄒㄧ ", &config);
    step(&mut engine, CaretDirection::Left, &config);
    for key in ["ㄒ", "ㄧ"] {
        tps_key(&mut engine, key, &config);
    }
    let response = tps_key(&mut engine, " ", &config);
    assert_eq!(raw_input(&response), "ㄒㄧˋㄒㄧ ㄒㄧ ");
    assert_eq!(display(&response), "死詩詩");
    assert_eq!(raw_caret(&engine), 15);
}

// A reading that closes into a longer word moves the caret to that word's
// end, so it never sits inside a word. trace: "ㄍㄧㄣ ㆢㄧㆵ˙" → 今 (0, 10) +
// 日 (10, 21) (no row spans kin-ji̍t). Caret 10; "ㄚ" → "今ㄚ日"; "ˋ" closes
// ㄚˋ at 15 → the whole tail walks to the one edge 今仔日 (0, 26), and the
// caret 15 inside it goes to 26 (display 3). The controls 機 and 字 never
// appear.
#[test]
fn a_reading_that_joins_a_longer_word_puts_the_caret_after_it() {
    let _lock = engine_install_lock();
    install_fixture_with(&[]);
    let config = config_converting("tps");
    let (mut engine, response) = composing_engine("ㄍㄧㄣ ㆢㄧㆵ˙", &config);
    assert_eq!(display(&response), "今日");
    assert_eq!(
        converted_words(&engine),
        vec![((0, 10), "今".to_string()), ((10, 21), "日".to_string())]
    );
    step(&mut engine, CaretDirection::Left, &config);

    let response = tps_key(&mut engine, "ㄚ", &config);
    assert_eq!(display(&response), "今ㄚ日");
    let response = tps_key(&mut engine, "ˋ", &config);
    assert_eq!(display(&response), "今仔日");
    assert_eq!(caret_utf16(&response), 3);
    assert_eq!(raw_caret(&engine), 26);
    assert_eq!(
        converted_words(&engine),
        vec![((0, 26), "今仔日".to_string())]
    );
}

// Backspace inside the tail takes one glyph; the word it took it from shows
// as glyphs, the caret stays in that reading, and the other words stay.
// trace: caret 8 of "ㄒㄧˋㄒㄧ " → the mark goes: raw "ㄒㄧㄒㄧ ", caret 6; 死
// is dropped, 詩 shifts to (6, 13); "ㄒㄧ" ends on no mark → "ㄒㄧ詩", caret
// display 2. Typing the mark back closes the reading: 死詩 again.
#[test]
fn backspace_between_words_reopens_the_word_before_the_caret() {
    let _lock = engine_install_lock();
    install_fixture_with(&[]);
    let config = config_converting("tps");
    let (mut engine, _) = composing_engine("ㄒㄧˋㄒㄧ ", &config);
    step(&mut engine, CaretDirection::Left, &config);

    let response = engine.apply(Intent::DeleteBackward, &config);
    assert_eq!(raw_input(&response), "ㄒㄧㄒㄧ ");
    assert_eq!(display(&response), "ㄒㄧ詩");
    assert_eq!(caret_utf16(&response), 2);
    assert_eq!(converted_words(&engine), vec![((6, 13), "詩".to_string())]);

    let response = tps_key(&mut engine, "ˋ", &config);
    assert_eq!(display(&response), "死詩");
    assert_eq!(caret_utf16(&response), 1);
}

// Words left with glyphs between them are not re-used once the edit is at
// the end: the tail is walked again from its start. trace: as above, "ㄒㄧ詩";
// Right → 13 (the end); "ㄒ" → raw "ㄒㄧㄒㄧ ㄒ": the closed part runs to the
// separator (13) and is walked whole — the unmarked first "ㄒㄧ" takes the
// most frequent si, 是 (1000), the second is pinned to tone 1, 詩.
#[test]
fn glyphs_left_between_words_are_walked_again_from_the_end() {
    let _lock = engine_install_lock();
    install_fixture_with(&[]);
    let config = config_converting("tps");
    let (mut engine, _) = composing_engine("ㄒㄧˋㄒㄧ ", &config);
    step(&mut engine, CaretDirection::Left, &config);
    engine.apply(Intent::DeleteBackward, &config);
    step(&mut engine, CaretDirection::Right, &config);
    assert_eq!(raw_caret(&engine), 13);

    let response = tps_key(&mut engine, "ㄒ", &config);
    assert_eq!(display(&response), "是詩ㄒ");
    assert_eq!(
        converted_words(&engine),
        vec![((0, 6), "是".to_string()), ((6, 13), "詩".to_string())]
    );
}

// A step left from the start of the tail re-opens the last nailed segment
// (H3) and answers as Backspace's un-nail does. trace: "ㄒㄧˋㄒㄧ " → pick 是
// over 8 bytes → nailed 是, tail "ㄒㄧ " → 詩 (0, 7); display "是詩". Left →
// 0 (display 1, after the nailed 是). Left → the glyphs "ㄒㄧˋ" go back in
// front: raw "ㄒㄧˋㄒㄧ ", caret 0, walked again → 死詩 (the pick is
// dropped), nothing nailed. A further Left is an edge.
#[test]
fn a_step_left_from_the_start_reopens_the_last_nailed_segment() {
    let _lock = engine_install_lock();
    install_fixture_with(&[]);
    let config = config_converting("tps");
    let (mut engine, _) = composing_engine("ㄒㄧˋㄒㄧ ", &config);
    let response = engine.apply(pick("是", "sī", 8), &config);
    assert_eq!(display(&response), "是詩");

    let response = step(&mut engine, CaretDirection::Left, &config);
    assert_eq!(caret_utf16(&response), 1);
    assert_eq!(nailed_count(&engine), 1);

    let response = step(&mut engine, CaretDirection::Left, &config);
    assert_eq!(
        effect_kinds(&response.effect),
        vec![
            "NextWordClearForNewComposing",
            "UpdatePreedit",
            "RefreshCandidates"
        ]
    );
    assert_eq!(raw_input(&response), "ㄒㄧˋㄒㄧ ");
    assert_eq!(display(&response), "死詩");
    assert_eq!(caret_utf16(&response), 0);
    assert_eq!(nailed_count(&engine), 0);
    assert_eq!(
        converted_words(&engine),
        vec![((0, 8), "死".to_string()), ((8, 15), "詩".to_string())]
    );

    let response = step(&mut engine, CaretDirection::Left, &config);
    assert!(response.effect.is_empty());
}

// With two segments nailed, the re-open keeps the first and the handshake
// names it. trace: "ㄒㄧˋㄒㄧˋㄒㄧ " → pick 是 (8), pick 是 (8) → tail "ㄒㄧ " →
// "是是詩"; Left, Left → the second 是's glyphs re-open: "是" + 死詩, caret
// after the nailed 是 (display 1).
#[test]
fn a_reopen_keeps_the_segments_before_the_last() {
    let _lock = engine_install_lock();
    install_fixture_with(&[]);
    let config = config_converting("tps");
    let (mut engine, _) = composing_engine("ㄒㄧˋㄒㄧˋㄒㄧ ", &config);
    engine.apply(pick("是", "sī", 8), &config);
    let response = engine.apply(pick("是", "sī", 8), &config);
    assert_eq!(display(&response), "是是詩");

    step(&mut engine, CaretDirection::Left, &config);
    let response = step(&mut engine, CaretDirection::Left, &config);
    assert_eq!(
        effect_kinds(&response.effect)[0],
        "NextWordUpdateLastSelectedWord"
    );
    assert_eq!(display(&response), "是死詩");
    assert_eq!(caret_utf16(&response), 1);
    assert_eq!(nailed_count(&engine), 1);
}

// Without the switch, and on a romanization buffer with it, the start of the
// tail stays an edge (MoveCaret's contract): nothing re-opens.
#[test]
fn without_a_converted_tps_tail_the_start_stays_an_edge() {
    let _lock = engine_install_lock();
    install_fixture_with(&[]);
    for (config, raw, picked) in [
        (config("tps"), "ㄒㄧˋㄒㄧ ", 8),
        (config_converting("tl"), "sisi", 2),
    ] {
        let mut engine = Engine::new();
        engine.apply(Intent::Start { text: raw.into() }, &config);
        engine.apply(pick("是", "sī", picked), &config);
        while raw_caret(&engine) > 0 {
            step(&mut engine, CaretDirection::Left, &config);
        }
        let response = step(&mut engine, CaretDirection::Left, &config);
        assert!(response.effect.is_empty(), "{raw:?}");
        assert_eq!(nailed_count(&engine), 1, "{raw:?}");
    }
}

// A request without the switch is shown the glyphs and steps by glyph; when
// the switch is back, a walk never leaves the caret inside a word. trace:
// caret 8 of "ㄒㄧˋㄒㄧ "; off: the glyph preedit, caret after 3 glyphs; a
// glyph step left → 6 drops the conversion. On: no conversion to step over →
// glyph step to 3, then the walk makes 死 (0, 8) around it → caret 8.
#[test]
fn a_walk_after_the_switch_returns_keeps_the_caret_out_of_words() {
    let _lock = engine_install_lock();
    install_fixture_with(&[]);
    let on = config_converting("tps");
    let off = config("tps");
    let (mut engine, _) = composing_engine("ㄒㄧˋㄒㄧ ", &on);
    step(&mut engine, CaretDirection::Left, &on);
    assert_eq!(caret_utf16(&engine.snapshot(&off)), 3);

    step(&mut engine, CaretDirection::Left, &off);
    assert_eq!(raw_caret(&engine), 6);
    assert_eq!(conversion_of(&engine), None);

    let response = step(&mut engine, CaretDirection::Left, &on);
    assert_eq!(display(&response), "死詩");
    assert_eq!(raw_caret(&engine), 8);
    assert_eq!(caret_utf16(&response), 1);
}

// A changed source filter walks again and keeps the caret out of a word,
// both ways. trace: caret 8 of "ㄒㄧˋㄒㄧ " (死 | 詩). Every dictionary off (the
// filter 0, §57): the held words are not for it → a glyph step to 6; the
// fresh walk finds no dictionary word, and its path is one glyph edge over
// both readings, (0, 15) (read by running the walk) → the caret 6 inside it
// goes to 15. Back to every source: a glyph step to 14, the walk makes 死
// (0, 8) + 詩 (8, 15), 14 is inside 詩 → 15. Word steps work again from there.
#[test]
fn a_changed_source_filter_walks_again_and_keeps_the_caret_out_of_words() {
    let _lock = engine_install_lock();
    install_fixture_with(&[]);
    let every_source = config_converting("tps");
    let every_dictionary_off = AppConfig {
        hanji_conversion: Some(HanjiConversion {
            toggles: Some(DictionarySourceToggles::default()),
        }),
        ..config("tps")
    };
    let (mut engine, _) = composing_engine("ㄒㄧˋㄒㄧ ", &every_source);
    step(&mut engine, CaretDirection::Left, &every_source);

    let response = step(&mut engine, CaretDirection::Left, &every_dictionary_off);
    assert_eq!(display(&response), "ㄒㄧˋㄒㄧ");
    assert_eq!((raw_caret(&engine), caret_utf16(&response)), (15, 5));
    assert_eq!(
        converted_words(&engine),
        vec![((0, 15), "ㄒㄧˋㄒㄧ".to_string())]
    );

    let response = step(&mut engine, CaretDirection::Left, &every_source);
    assert_eq!(display(&response), "死詩");
    assert_eq!((raw_caret(&engine), caret_utf16(&response)), (15, 2));
    step(&mut engine, CaretDirection::Left, &every_source);
    assert_eq!(raw_caret(&engine), 8);
    step(&mut engine, CaretDirection::Right, &every_source);
    assert_eq!(raw_caret(&engine), 15);
}

// A separator typed between two words is a reading's close on its own.
// trace: "ㄒㄧㄒㄧ " → the unmarked first reading takes 是 (0, 6), the second
// is pinned to 詩 (6, 13). Caret 6; Space → raw "ㄒㄧ ㄒㄧ ", caret 7: 是 is
// before the edit, 詩 shifts to (7, 14), the gap (6, 7) is the separator
// alone, and 7 is a closing end → the whole tail is walked: the first reading
// is pinned to tone 1 too, 詩 (0, 7) with its separator, + 詩 (7, 14); the
// caret is on their boundary (display 1).
#[test]
fn a_separator_typed_between_words_closes_the_reading_before_it() {
    let _lock = engine_install_lock();
    install_fixture_with(&[]);
    let config = config_converting("tps");
    let (mut engine, response) = composing_engine("ㄒㄧㄒㄧ ", &config);
    assert_eq!(display(&response), "是詩");
    step(&mut engine, CaretDirection::Left, &config);
    assert_eq!(raw_caret(&engine), 6);

    let response = tps_key(&mut engine, " ", &config);
    assert_eq!(raw_input(&response), "ㄒㄧ ㄒㄧ ");
    assert_eq!(display(&response), "詩詩");
    assert_eq!((raw_caret(&engine), caret_utf16(&response)), (7, 1));
    assert_eq!(
        converted_words(&engine),
        vec![((0, 7), "詩".to_string()), ((7, 14), "詩".to_string())]
    );
}

// The caret is projected in UTF-16 units: 𪜶 (U+2A736) is two. trace:
// "ㄧㄣ ㄒㄧˋ" → 𪜶 (0, 7) + 死 (7, 15); caret 15 → 3; Left → 7 → 2; Left → 0.
// The control 伊 (i, `ㄧ`) is a strict prefix of ㄧㄣ and never appears.
#[test]
fn the_caret_counts_a_non_bmp_word_in_utf16_units() {
    let _lock = engine_install_lock();
    install_fixture_with(&[("𪜶", "in", 800), ("伊", "i", 2000)]);
    let config = config_converting("tps");
    let (mut engine, response) = composing_engine("ㄧㄣ ㄒㄧˋ", &config);
    assert_eq!(display(&response), "𪜶死");
    assert_eq!(caret_utf16(&response), 3);
    assert!(!display(&response).contains('伊'));

    let response = step(&mut engine, CaretDirection::Left, &config);
    assert_eq!((raw_caret(&engine), caret_utf16(&response)), (7, 2));
    let response = step(&mut engine, CaretDirection::Left, &config);
    assert_eq!((raw_caret(&engine), caret_utf16(&response)), (0, 0));
}
