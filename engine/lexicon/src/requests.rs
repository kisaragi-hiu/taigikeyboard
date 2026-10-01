//! Dispatch: route every `LexiconRequest.method` oneof variant to the
//! per-method API — the read-path variants
//! (Install/SearchWithSources/SearchByHanji), the
//! IsHanji predicate, and the v3.5.8
//! DictionaryFilters variant.

use protos::engine::lexicon_request::Method;
use protos::engine::lexicon_response::Result as LexResult;
use protos::engine::{
    DictionaryFiltersRequest, InstallRequest, IsHanjiRequest, LexiconResponse,
    SearchByHanjiRequest, SearchWithSourcesRequest,
};

use crate::api;
use crate::error::LexiconError;

pub fn handle_install(req: InstallRequest) -> Result<LexiconResponse, LexiconError> {
    let resp = api::install(req)?;
    Ok(LexiconResponse {
        result: Some(LexResult::InstallResult(resp)),
    })
}

pub fn handle_search_with_sources(
    req: SearchWithSourcesRequest,
) -> Result<LexiconResponse, LexiconError> {
    let resp = api::search_with_sources(req)?;
    Ok(LexiconResponse {
        result: Some(LexResult::SearchWithSourcesResult(resp)),
    })
}

pub fn handle_search_by_hanji(req: SearchByHanjiRequest) -> Result<LexiconResponse, LexiconError> {
    let resp = api::search_by_hanji(req)?;
    Ok(LexiconResponse {
        result: Some(LexResult::SearchByHanjiResult(resp)),
    })
}

pub fn handle_is_hanji(req: IsHanjiRequest) -> Result<LexiconResponse, LexiconError> {
    let resp = api::is_hanji(req)?;
    Ok(LexiconResponse {
        result: Some(LexResult::IsHanjiResult(resp)),
    })
}

pub fn handle_dictionary_filters(
    req: DictionaryFiltersRequest,
) -> Result<LexiconResponse, LexiconError> {
    let resp = api::dictionary_filters(req)?;
    Ok(LexiconResponse {
        result: Some(LexResult::DictionaryFiltersResult(resp)),
    })
}

/// Dispatch `LexiconRequest.method` to the matching handler.
pub fn handle(method: Method) -> Result<LexiconResponse, LexiconError> {
    match method {
        Method::Install(req) => handle_install(req),
        Method::SearchWithSources(req) => handle_search_with_sources(req),
        Method::SearchByHanji(req) => handle_search_by_hanji(req),
        Method::IsHanji(req) => handle_is_hanji(req),
        Method::DictionaryFilters(req) => handle_dictionary_filters(req),
    }
}
