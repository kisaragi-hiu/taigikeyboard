//! Helpers the user-data integration suites share.

use prost::Message;
use protos::engine::{
    request, response, user_data_request, AppConfig, OpenUserData, Platform, Request, Response,
    UserDataJournal, UserDataRequest,
};
use userdata::UserDataPaths;

/// An iOS TL request config; `swapped` is `is_translate_swapped`.
pub fn tl_config(swapped: bool) -> AppConfig {
    AppConfig {
        platform_id: Platform::Ios as i32,
        input_mode: "tl".to_owned(),
        is_translate_swapped: swapped,
        ..AppConfig::default()
    }
}

/// One request through `process_request`.
pub fn roundtrip(config: AppConfig, generation: u64, payload: request::Payload) -> Response {
    let request = Request {
        id: 1,
        config_snapshot: Some(config),
        generation,
        payload: Some(payload),
    };
    Response::decode(dispatch::process_request(&request.encode_to_vec()).as_slice())
        .expect("response decodes")
}

/// Opens the engine's user data at `paths`, rollback-journaled as on iOS,
/// and asserts it answered.
pub fn open_user_data(paths: &UserDataPaths) {
    let opened = roundtrip(
        tl_config(false),
        0,
        request::Payload::UserData(UserDataRequest {
            method: Some(user_data_request::Method::Open(OpenUserData {
                frequency_path: paths.frequency.display().to_string(),
                association_path: paths.association.display().to_string(),
                custom_dictionary_path: paths.custom_dictionary.display().to_string(),
                learned_phrases_path: paths.learned_phrases.display().to_string(),
                journal: UserDataJournal::Delete as i32,
                ..OpenUserData::default()
            })),
        }),
    );
    assert!(
        matches!(opened.payload, Some(response::Payload::UserData(_))),
        "open answered {opened:?}"
    );
}

/// Installs the production lexicon (`dictionaries/`) once per test process;
/// `false` (callers soft-skip) when the artifacts are absent — run `make dict`.
// Not every suite in this directory needs the lexicon.
#[allow(dead_code)]
pub fn production_lexicon_ready() -> bool {
    static READY: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *READY.get_or_init(|| {
        let artifact = |name: &str| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../dictionaries")
                .join(name)
                .to_str()
                .expect("artifact path UTF-8")
                .to_owned()
        };
        if !std::path::Path::new(&artifact("association.bin")).exists() {
            eprintln!("production artifacts absent — run `make dict`; skipping.");
            return false;
        }
        let paths = lexicon::LexiconPaths::validated(
            &artifact("dictionary.fst"),
            &artifact("dictionary.bin"),
            &artifact("association.bin"),
            &artifact("syllables.fst"),
            0,
        )
        .expect("validate production LexiconPaths");
        lexicon::EngineHandle::install(paths).is_ok()
    })
}
