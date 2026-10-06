//! A learned phrase moved into the custom dictionary over the wire
//! (learning-records-page-roadmap P7); a frequency row is refused with
//! `FAIL_INVARIANT` and no payload.
//! Its own process: the user-data handle is process-wide.
#![cfg(feature = "user-data")]

mod common;

use common::{open_user_data, user_data};

use protos::engine::{
    response, user_data_request, user_data_response, CustomDictionaryRefusal, ErrorCode,
    LearningRecord, LearningRecordKind, ListLearningRecords, MoveLearningRecordToCustomDictionary,
    RecordUsage, Response, SearchCustomEntries,
};
use userdata::{JournalMode, UserDataPaths, UserDataStores};

fn answer(response: Response) -> user_data_response::Result {
    assert_eq!(response.error, ErrorCode::Ok as i32, "{response:?}");
    match response.payload {
        Some(response::Payload::UserData(user_data)) => user_data.result.expect("a result"),
        other => panic!("expected a user-data payload, got {other:?}"),
    }
}

/// Every row of `kind`; the pick is queued, the list waits behind it.
fn listed(kind: LearningRecordKind) -> Vec<LearningRecord> {
    match answer(user_data(user_data_request::Method::ListLearningRecords(
        ListLearningRecords {
            kind: kind as i32,
            limit: 50,
            ..ListLearningRecords::default()
        },
    ))) {
        user_data_response::Result::LearningRecords(records) => records.records,
        other => panic!("expected records, got {other:?}"),
    }
}

fn move_to_custom_dictionary(record: LearningRecord) -> Response {
    user_data(
        user_data_request::Method::MoveLearningRecordToCustomDictionary(
            MoveLearningRecordToCustomDictionary {
                record: Some(record),
            },
        ),
    )
}

fn custom_words(query: &str) -> Vec<String> {
    match answer(user_data(user_data_request::Method::SearchCustomEntries(
        SearchCustomEntries {
            query: query.into(),
            input_mode: "tl".into(),
            limit: 5,
        },
    ))) {
        user_data_response::Result::CustomEntryMatches(matches) => matches
            .entries
            .into_iter()
            .map(|entry| entry.hanji)
            .collect(),
        other => panic!("expected matches, got {other:?}"),
    }
}

#[test]
fn a_learned_phrase_moves_and_a_counted_word_stays() {
    let directory = tempfile::tempdir().unwrap();
    // A phrase the keyboard learned from a segment-by-segment commit.
    {
        let stores = UserDataStores::at(
            UserDataPaths::in_directory(directory.path()),
            JournalMode::Delete,
        );
        stores.open_blocking();
        stores.learned_phrases.learn_phrase("台灣", "tâi-uân");
        stores.learned_phrases.all_rows(); // flush the queued write
    }
    open_user_data(directory.path());
    // Picked again: the word is counted and the phrase touched.
    answer(user_data(user_data_request::Method::RecordUsage(
        RecordUsage {
            display_text: "台灣".into(),
            canonical_tl: "tâi-uân".into(),
            hanji: Some("台灣".into()),
        },
    )));
    let [word] = listed(LearningRecordKind::Frequency).try_into().unwrap();
    let [phrase] = listed(LearningRecordKind::LearnedPhrase)
        .try_into()
        .unwrap();

    // A frequency row keeps weighting its word: it never moves.
    let refused = move_to_custom_dictionary(word);
    assert_eq!(refused.error, ErrorCode::FailInvariant as i32);
    assert!(refused.payload.is_none());

    match answer(move_to_custom_dictionary(phrase)) {
        user_data_response::Result::LearningRecordMoved(moved) => {
            assert_eq!(moved.refusal(), CustomDictionaryRefusal::None);
        }
        other => panic!("expected a move, got {other:?}"),
    }
    assert!(listed(LearningRecordKind::LearnedPhrase).is_empty());
    assert_eq!(listed(LearningRecordKind::Frequency).len(), 1);
    assert_eq!(custom_words("taiuan"), ["台灣"]);
}
