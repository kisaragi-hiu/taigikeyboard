// PredictNext through `process_request` against the production lexicon: bundled rows, source
// toggles, and the rows the engine's own `user_association.db` holds.
#![cfg(feature = "user-data")]

mod common;

use std::sync::{Mutex, OnceLock, PoisonError};

use protos::engine::{
    next_word_request::Method, next_word_response, request, response, DictionaryToggles,
    EnginePrediction, NextWordRequest, PredictNext,
};
use userdata::{AssociationPair, JournalMode, UserDataPaths, UserDataStores};

fn all_sources(enabled: bool) -> DictionaryToggles {
    DictionaryToggles {
        kautian: enabled,
        taigitv: enabled,
        itaigi: enabled,
        sitbut: enabled,
        taihoa: enabled,
        taijit: enabled,
        kungge: enabled,
        stti: enabled,
        khpoo: enabled,
        ..DictionaryToggles::default()
    }
}

/// Opens the engine's user data once, with one learned bigram after each
/// word the tests predict from: `→ 𫝛`.
fn learned_rows_open() {
    static DIRECTORY: OnceLock<tempfile::TempDir> = OnceLock::new();
    DIRECTORY.get_or_init(|| {
        let directory = tempfile::tempdir().unwrap();
        let paths = UserDataPaths::in_directory(directory.path());
        let stores = UserDataStores::at(paths.clone(), JournalMode::Delete);
        stores.open_blocking();
        for previous in ["臺台", "台"] {
            stores.association.record(&[AssociationPair {
                previous: previous.to_owned(),
                previous_tl: String::new(),
                next: "𫝛".to_owned(),
                next_tl: String::new(),
            }]);
        }
        stores.association.all_rows(); // flush the queued writes
        drop(stores);
        common::open_user_data(directory.path());
        directory
    });
}

/// One PredictNext round trip. Envelope generation 0 matches the fresh
/// nextword handle, so `query_generation: 0` is never stale.
fn predict(word: &str, toggles: DictionaryToggles) -> Vec<EnginePrediction> {
    predict_after(word, "", toggles)
}

/// [`predict`] for a committed word whose canonical TL reading is `roman`.
fn predict_after(word: &str, roman: &str, toggles: DictionaryToggles) -> Vec<EnginePrediction> {
    // One prediction at a time: a store read never waits on the key path
    // (`try_lock`), so two parallel tests would each find the other reading.
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(PoisonError::into_inner);
    learned_rows_open();
    // The store stamps its rows with the wall clock, in whole seconds.
    let now_ms = i64::try_from(userdata::unix_seconds_now() * 1000).expect("epoch-ms fits i64");
    let response = common::roundtrip(
        common::tl_config(true),
        0,
        request::Payload::Nextword(NextWordRequest {
            method: Some(Method::PredictNext(PredictNext {
                word: word.to_owned(),
                roman: roman.to_owned(),
                toggles: Some(toggles),
                query_generation: 0,
                now_ms,
                limit: 30,
            })),
        }),
    );
    let Some(response::Payload::Nextword(nextword)) = response.payload else {
        panic!("expected Nextword payload, got {response:?}");
    };
    let Some(next_word_response::Result::Filter(filter)) = nextword.result else {
        panic!("expected FilterResult, got {nextword:?}");
    };
    assert!(!filter.was_stale);
    filter.predictions
}

fn hanzi_of(predictions: &[EnginePrediction]) -> Vec<&str> {
    predictions.iter().map(|p| p.hanzi.as_str()).collect()
}

// 台 → 灣 is the top bundled pair (association.bin, count 2137); the key is
// the last character, so 臺台 predicts from 台.
#[test]
fn bundled_rows_join_learned_rows_when_sources_enabled() {
    if !common::production_lexicon_ready() {
        return;
    }
    let predictions = predict("臺台", all_sources(true));
    let hanzi = hanzi_of(&predictions);
    assert!(
        hanzi.contains(&"灣"),
        "bundled 台→灣 expected, got {hanzi:?}"
    );
    assert!(hanzi.contains(&"𫝛"), "learned row expected, got {hanzi:?}");
    assert!(predictions.len() <= 30);
}

// INVARIANT_NEXTWORD_LOOKUP_KEY_LAST_GRAPHEME: on the character-key path
// (empty roman here), 𣍐 (U+2334D) is one key, so
// 𣍐使 in the dictionary predicts 使 after a word ending in 𣍐.
#[test]
fn supplementary_plane_hanji_is_one_lookup_key() {
    if !common::production_lexicon_ready() {
        return;
    }
    let predictions = predict("袂𣍐", all_sources(true));
    let hanzi = hanzi_of(&predictions);
    assert!(
        hanzi.contains(&"使"),
        "bundled 𣍐→使 expected, got {hanzi:?}"
    );
}

#[test]
fn disabled_sources_leave_only_learned_rows() {
    if !common::production_lexicon_ready() {
        return;
    }
    let predictions = predict("台", all_sources(false));
    assert_eq!(hanzi_of(&predictions), vec!["𫝛"]);
}

// INVARIANT_NEXTWORD_WORD_KEY_BACKOFF: a committed word with a known reading
// predicts from its word key — corpus pairs across the word boundary. 食/tsia̍h
// leads with 酒 (word key, count 165); the character key 食 leads with its
// intra-word continuation 飯 (count 276).
#[test]
fn known_reading_predicts_from_word_key() {
    if !common::production_lexicon_ready() {
        return;
    }
    let by_word = predict_after("食", "tsia̍h", all_sources(true));
    let by_character = predict("食", all_sources(true));
    assert_eq!(by_word.first().map(|p| p.hanzi.as_str()), Some("酒"));
    assert_eq!(by_character.first().map(|p| p.hanzi.as_str()), Some("飯"));
}

// INVARIANT_NEXTWORD_WORD_KEY_BACKOFF: a reading the word namespace does not
// hold backs off to the character key — the same list an empty roman gets.
#[test]
fn unknown_reading_backs_off_to_character_key() {
    if !common::production_lexicon_ready() {
        return;
    }
    let unknown = predict_after("食", "tsiah", all_sources(true));
    assert_eq!(unknown, predict("食", all_sources(true)));
}

// INVARIANT_NEXTWORD_WORD_KEY_BACKOFF: a word key whose every continuation
// comes from disabled sources counts as absent. 䆀/bái has no taigitv entry;
// the character key 䆀 has 球 from taigitv.
#[test]
fn word_key_emptied_by_source_filter_backs_off() {
    if !common::production_lexicon_ready() {
        return;
    }
    let taigitv_only = DictionaryToggles {
        taigitv: true,
        ..all_sources(false)
    };
    let predictions = predict_after("䆀", "bái", taigitv_only);
    let hanzi = hanzi_of(&predictions);
    assert!(
        hanzi.contains(&"球"),
        "character-key 球 expected, got {hanzi:?}"
    );
}

// INVARIANT_NEXTWORD_WORD_KEY_BACKOFF: backoff is word → character, never a
// merge — a word key with 3 rows yields those 3, not padded from 中's 479.
#[test]
fn word_key_hit_is_not_padded_from_character_key() {
    if !common::production_lexicon_ready() {
        return;
    }
    let hanzi = hanzi_of(&predict_after("中", "tìng", all_sources(true))).join(",");
    assert_eq!(hanzi, "阮,伊,眾人");
}
