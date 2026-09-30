//! Public API for the bridge layer. Each method takes proto-shaped inputs
//! and returns proto-shaped outputs; the dispatch module wraps these into
//! envelope responses.

use protos::engine::{
    AssocLookupRequest, AssocLookupResponse, DictionaryFiltersRequest, DictionaryFiltersResponse,
    InstallRequest, InstallResponse, IsHanziRequest, IsHanziResponse, LexiconAssocEntry,
    SearchByHanziRequest, SearchByHanziResponse, SearchWithSourcesRequest,
    SearchWithSourcesResponse, TaigiWord,
};

use crate::classification;
// The source filter alone, for an engine fetch that carries the toggles
// itself (`FetchAtPos.toggles`).
use crate::dictionary_filters::compute_filters;
pub use crate::dictionary_filters::dictionary_filter_bitmask;
use crate::error::LexiconError;
use crate::handle::EngineHandle;
use crate::paths::LexiconPaths;
use crate::search::{self, LexiconAssocOut, LexiconRowOut, SearchInputMode, SearchParams};

// Validates paths, opens FST/TKDB/TKWA (+ optional syllables.fst), atomically swaps the handle.
pub fn install(req: InstallRequest) -> Result<InstallResponse, LexiconError> {
    let paths = LexiconPaths::validated(
        &req.trie_path,
        &req.dictionary_bin_path,
        &req.association_bin_path,
        &req.syllable_inventory_path,
        req.dictionary_version,
    )?;
    let stats = EngineHandle::install(paths)?;
    Ok(InstallResponse {
        dictionary_record_count: stats.dictionary_record_count,
        prefix_index_entry_count: stats.prefix_index_entry_count,
    })
}

// Tab3 romanization lookup, filtered by the enabled-source bitmask.
pub fn search_with_sources(
    req: SearchWithSourcesRequest,
) -> Result<SearchWithSourcesResponse, LexiconError> {
    let params = SearchParams {
        input: req.input,
        input_mode: proto_input_mode(req.input_mode),
        limit: req.limit,
        enabled_sources_bitmask: req.enabled_sources_bitmask,
    };
    EngineHandle::with_state(|state| {
        let prefix_index = state
            .prefix_index
            .as_ref()
            .ok_or_else(|| LexiconError::Internal("prefix_index unavailable".into()))?;
        let dict = state
            .dictionary
            .as_ref()
            .ok_or_else(|| LexiconError::Internal("dictionary reader unavailable".into()))?;
        let rows = search::search(&params, prefix_index, dict)?;
        Ok(SearchWithSourcesResponse {
            rows: rows.into_iter().map(row_out_to_taigi_word).collect(),
        })
    })
}

// Tab3 Hanji lookup: scans the index under the `hanzi:` prefix, filtered by the source bitmask.
pub fn search_by_hanzi(req: SearchByHanziRequest) -> Result<SearchByHanziResponse, LexiconError> {
    EngineHandle::with_state(|state| {
        let prefix_index = state
            .prefix_index
            .as_ref()
            .ok_or_else(|| LexiconError::Internal("prefix_index unavailable".into()))?;
        let dict = state
            .dictionary
            .as_ref()
            .ok_or_else(|| LexiconError::Internal("dictionary reader unavailable".into()))?;
        let rows = search::search_by_hanzi(
            &req.query,
            req.limit,
            req.enabled_sources_bitmask,
            prefix_index,
            dict,
        )?;
        Ok(SearchByHanziResponse {
            rows: rows.into_iter().map(row_out_to_taigi_word).collect(),
        })
    })
}

// NextWord bigram lookup for the committed word (word key, character-key backoff), source-filtered.
pub fn assoc_lookup(req: AssocLookupRequest) -> Result<AssocLookupResponse, LexiconError> {
    EngineHandle::with_state(|state| {
        let assoc = state
            .association
            .as_ref()
            .ok_or_else(|| LexiconError::Internal("association reader unavailable".into()))?;
        let entries = search::assoc_lookup(
            &req.previous_word,
            &req.previous_tl,
            req.limit,
            req.enabled_sources_bitmask,
            assoc,
        )?;
        Ok(AssocLookupResponse {
            entries: entries.into_iter().map(assoc_out_to_proto).collect(),
        })
    })
}

// True when the text contains any CJK Hanji, Extensions A-E included.
pub fn is_hanzi(req: IsHanziRequest) -> Result<IsHanziResponse, LexiconError> {
    Ok(IsHanziResponse {
        is_hanzi: classification::is_hanzi(&req.text),
    })
}

// Maps the user's dictionary toggles to a source bitmask plus the enabled source-code list.
pub fn dictionary_filters(
    req: DictionaryFiltersRequest,
) -> Result<DictionaryFiltersResponse, LexiconError> {
    let toggles = req.toggles.unwrap_or_default();
    Ok(compute_filters(&toggles))
}

fn proto_input_mode(value: i32) -> SearchInputMode {
    use protos::engine::InputMode;
    match InputMode::try_from(value).unwrap_or(InputMode::Unspecified) {
        InputMode::Poj => SearchInputMode::Poj,
        InputMode::Tps => SearchInputMode::Tps,
        // Unspecified + Tl default to TL.
        _ => SearchInputMode::Tl,
    }
}

fn row_out_to_taigi_word(row: LexiconRowOut) -> TaigiWord {
    TaigiWord {
        id: row.id,
        roman: row.roman,
        hanji: row.hanji,
        length_score: row.length_score,
        source_bitmask: row.source_bitmask,
    }
}

fn assoc_out_to_proto(entry: LexiconAssocOut) -> LexiconAssocEntry {
    LexiconAssocEntry {
        previous_word: entry.previous_word,
        candidate_word: entry.candidate_word,
        count: entry.count,
        candidate_tl: entry.candidate_tl,
    }
}
