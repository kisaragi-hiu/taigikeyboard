//! Lexicon slice of the engine bridge: loading the dictionary data and
//! resolving the user's source toggles into the engine's bitmask, and the
//! dictionary-search page's two lookups. Twin of iOS
//! `RustEngineBridge+Lexicon.swift`.

use std::collections::BTreeSet;

use protos::engine::{
    lexicon_request, lexicon_response, request, response, DictionaryFiltersRequest,
    DictionarySourceCode, DictionarySourceToggles as WireDictionarySourceToggles,
    InputMode as WireInputMode, InstallRequest, IsHanjiRequest, KautianSubcollectionToggles,
    LexiconRequest, LexiconResponse, SearchByHanjiRequest, SearchWithSourcesRequest, TaigiWord,
};

use super::bridge::{record_failure, roundtrip};
use crate::dictionary_artifacts::DictionaryArtifacts;
use crate::settings::{DictionarySourceToggles, InputMode};
use crate::strings::StringKey;

/// Record counts the engine reports after loading. Their only job is to make
/// a successful install self-evident in the log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LexiconInstallStats {
    pub dictionary_record_count: u64,
    pub prefix_index_entry_count: u64,
}

/// A dictionary source, in the platform's own vocabulary. Decoded from the
/// wire's `DictionarySourceCode` through an explicit match — the wire codes
/// are stable numbers and this enum has no numeric contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DictionarySource {
    Kautian,
    Taigitv,
    Itaigi,
    Sitbut,
    Taihoa,
    Taijit,
    Kungge,
    Stti,
    Khpoo,
    Khiin,
    Lkk,
    Dev,
    Custom,
}

/// The sources a record's `source_bitmask` names, in bit order — which is
/// the order the badges are drawn in (iOS `LexiconBitmask.swift`). Bit 12
/// (variant) is a filter, not a source a record wears a badge for.
const SOURCE_BITS: [(u32, DictionarySource); 12] = [
    (1 << 0, DictionarySource::Kautian),
    (1 << 1, DictionarySource::Taigitv),
    (1 << 2, DictionarySource::Itaigi),
    (1 << 3, DictionarySource::Sitbut),
    (1 << 4, DictionarySource::Taihoa),
    (1 << 5, DictionarySource::Taijit),
    (1 << 6, DictionarySource::Kungge),
    (1 << 7, DictionarySource::Stti),
    (1 << 8, DictionarySource::Khpoo),
    (1 << 9, DictionarySource::Khiin),
    (1 << 10, DictionarySource::Dev),
    (1 << 11, DictionarySource::Lkk),
];

impl DictionarySource {
    /// The sources a record belongs to, in bit order.
    pub fn from_bitmask(bitmask: u32) -> Vec<Self> {
        SOURCE_BITS
            .iter()
            .filter(|(bit, _)| bitmask & bit != 0)
            .map(|(_, source)| *source)
            .collect()
    }

    /// The badge a search result wears for this source: the three
    /// supplements share one word, the custom dictionary its pane's name.
    pub fn badge_key(self) -> StringKey {
        match self {
            Self::Kautian => StringKey::DictionaryKautianTag,
            Self::Taigitv => StringKey::DictionaryTaigitvTag,
            Self::Itaigi => StringKey::DictionaryITaigiTag,
            Self::Sitbut => StringKey::DictionarySitbutTag,
            Self::Taihoa => StringKey::DictionaryTaihoaTag,
            Self::Taijit => StringKey::DictionaryTaijitTag,
            Self::Kungge => StringKey::DictionaryKunggeTag,
            Self::Stti => StringKey::DictionarySttiTag,
            Self::Lkk => StringKey::DictionaryLkkTag,
            Self::Khpoo | Self::Khiin | Self::Dev => StringKey::DictionarySupplementSectionTitle,
            Self::Custom => StringKey::DictionaryCustomDictionary,
        }
    }

    /// An unrecognised code is dropped — a newer engine naming a source this
    /// build has never heard of is not a reason to fail a search.
    fn from_code(code: i32) -> Option<Self> {
        Some(match DictionarySourceCode::try_from(code).ok()? {
            DictionarySourceCode::DictSourceKautian => Self::Kautian,
            DictionarySourceCode::DictSourceTaigitv => Self::Taigitv,
            DictionarySourceCode::DictSourceItaigi => Self::Itaigi,
            DictionarySourceCode::DictSourceSitbut => Self::Sitbut,
            DictionarySourceCode::DictSourceTaihoa => Self::Taihoa,
            DictionarySourceCode::DictSourceTaijit => Self::Taijit,
            DictionarySourceCode::DictSourceKungge => Self::Kungge,
            DictionarySourceCode::DictSourceStti => Self::Stti,
            DictionarySourceCode::DictSourceKhpoo => Self::Khpoo,
            DictionarySourceCode::DictSourceKhiin => Self::Khiin,
            DictionarySourceCode::DictSourceLkk => Self::Lkk,
            DictionarySourceCode::DictSourceDev => Self::Dev,
            DictionarySourceCode::DictSourceCustom => Self::Custom,
            DictionarySourceCode::DictSourceUnspecified => return None,
        })
    }
}

/// What one resolve of the user's dictionary toggles answered. Both halves
/// come from the same round-trip on purpose: the mask decides which rows the
/// engine returns and the source set decides which badges those rows carry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DictionaryFilters {
    /// The engine's own answer, verbatim.
    pub dictionary_filter_bitmask: u32,
    /// The sources the user has switched on, for labelling results.
    pub enabled_sources: BTreeSet<DictionarySource>,
}

/// What a SEARCH sends when the toggles could not be resolved. The search
/// path takes `u32::MAX` as its "filter disabled" sentinel
/// (`engine/lexicon/src/dictionary_reader.rs:147`); sending `0` there would
/// be fail-CLOSED.
pub const ALL_SOURCES_ENABLED_SEARCH_BITMASK: u32 = u32::MAX;

/// Points the engine at the dictionary data. Sent once per process; the
/// files are read-only and outlive every composing session.
///
/// `None` means the engine has no lexicon installed. Nothing retries or falls
/// back: a search against an uninstalled engine returns no candidates, the
/// same graceful degradation any other empty result produces.
pub fn install(
    artifacts: &DictionaryArtifacts,
    dictionary_version: u32,
) -> Option<LexiconInstallStats> {
    let op = "lexiconInstall";
    let path = |path: &std::path::Path| {
        DictionaryArtifacts::wire(path).or_else(|| {
            record_failure(op, &format!("non-UTF-8 artefact path {}", path.display()));
            None
        })
    };
    let install = InstallRequest {
        trie_path: path(&artifacts.trie_path)?,
        dictionary_bin_path: path(&artifacts.dictionary_bin_path)?,
        association_bin_path: path(&artifacts.association_bin_path)?,
        dictionary_version,
        syllable_inventory_path: path(&artifacts.syllable_inventory_path)?,
    };
    let response = lexicon_response(lexicon_request::Method::Install(install), op)?;
    match response.result {
        Some(lexicon_response::Result::InstallResult(result)) => Some(LexiconInstallStats {
            dictionary_record_count: result.dictionary_record_count,
            prefix_index_entry_count: result.prefix_index_entry_count,
        }),
        _ => {
            record_failure(op, "response carried no install result");
            None
        }
    }
}

/// Resolves the user's dictionary toggles into the bitmask the engine filters
/// candidates by. The bit layout — including the kautian subcollection region
/// in bits 13-25 — belongs to Rust (`engine/lexicon/src/dictionary_filters.rs`);
/// this asks for it rather than reproducing it (no redundant fallback,
/// `AGENTS.md` § Design principles).
///
/// `None` means the round-trip failed. Callers resolve ONCE per query and pass
/// the answer down, so mask and badge set describe one instant.
pub fn dictionary_filters(toggles: &DictionarySourceToggles) -> Option<DictionaryFilters> {
    let op = "lexiconDictionaryFilters";
    let response = lexicon_response(
        lexicon_request::Method::DictionaryFilters(DictionaryFiltersRequest {
            toggles: Some(dictionary_toggles(toggles)),
        }),
        op,
    )?;
    match response.result {
        Some(lexicon_response::Result::DictionaryFiltersResult(result)) => {
            Some(DictionaryFilters {
                dictionary_filter_bitmask: result.dictionary_filter_bitmask,
                enabled_sources: result
                    .enabled_source_codes
                    .iter()
                    .filter_map(|code| DictionarySource::from_code(*code))
                    .collect(),
            })
        }
        _ => {
            record_failure(op, "response carried no dictionary-filters result");
            None
        }
    }
}

/// The user's dictionary toggles on the wire — what `DictionaryFilters` and
/// `FetchAtPos` carry; the engine resolves them into its source filter.
pub(crate) fn dictionary_toggles(toggles: &DictionarySourceToggles) -> WireDictionarySourceToggles {
    let subcollections = &toggles.kautian_subcollections;
    WireDictionarySourceToggles {
        kautian: toggles.kautian,
        taigitv: toggles.taigitv,
        itaigi: toggles.itaigi,
        sitbut: toggles.sitbut,
        taihoa: toggles.taihoa,
        taijit: toggles.taijit,
        kungge: toggles.kungge,
        stti: toggles.stti,
        khpoo: toggles.khpoo,
        variant: toggles.variant,
        khiin: toggles.khiin,
        lkk: toggles.lkk,
        dev: toggles.dev,
        // Always sent: an absent subcollection message tells the engine to
        // skip the gate and treat every subcollection as on.
        kautian_subcollections: Some(KautianSubcollectionToggles {
            accent_lukang: subcollections.accent_lukang,
            accent_sansia: subcollections.accent_sansia,
            accent_taipak: subcollections.accent_taipak,
            accent_gilan: subcollections.accent_gilan,
            accent_tainan: subcollections.accent_tainan,
            accent_kaohsiung: subcollections.accent_kaohsiung,
            accent_kinmen: subcollections.accent_kinmen,
            accent_makung: subcollections.accent_makung,
            accent_sintik: subcollections.accent_sintik,
            accent_taichung: subcollections.accent_taichung,
            name_appendix: subcollections.name_appendix,
        }),
    }
}

/// One dictionary record as the Dictionary Search page lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LexiconRow {
    pub id: i64,
    pub roman: String,
    pub hanji: Option<String>,
    pub length_score: Option<i32>,
    pub source_bitmask: Option<u32>,
}

impl LexiconRow {
    fn from_wire(word: TaigiWord) -> Self {
        Self {
            id: word.id,
            roman: word.roman,
            hanji: word.hanji,
            length_score: word.length_score,
            source_bitmask: word.source_bitmask,
        }
    }

    /// The search page's order (`DictionarySearchService.Ordering`): MOE dictionary
    /// records first, then by length score descending, then as the engine
    /// listed them.
    pub fn sorted_for_search(rows: Vec<LexiconRow>) -> Vec<LexiconRow> {
        let is_kautian = |row: &LexiconRow| {
            DictionarySource::from_bitmask(row.source_bitmask.unwrap_or(0))
                .contains(&DictionarySource::Kautian)
        };
        let mut indexed: Vec<(usize, LexiconRow)> = rows.into_iter().enumerate().collect();
        indexed.sort_by(|(first_index, first), (second_index, second)| {
            is_kautian(second)
                .cmp(&is_kautian(first))
                .then_with(|| {
                    second
                        .length_score
                        .unwrap_or(0)
                        .cmp(&first.length_score.unwrap_or(0))
                })
                .then_with(|| first_index.cmp(second_index))
        });
        indexed.into_iter().map(|(_, row)| row).collect()
    }
}

fn wire_input_mode(mode: InputMode) -> i32 {
    match mode {
        InputMode::Tl => WireInputMode::Tl as i32,
        InputMode::Poj => WireInputMode::Poj as i32,
    }
}

/// The Dictionary Search page's all-source romanization lookup.
pub fn search_with_sources(
    input: &str,
    mode: InputMode,
    limit: u32,
    enabled_sources_bitmask: u32,
) -> Vec<LexiconRow> {
    let op = "lexiconSearchWithSources";
    let request = SearchWithSourcesRequest {
        input: input.to_owned(),
        input_mode: wire_input_mode(mode),
        limit,
        enabled_sources_bitmask,
    };
    let Some(response) = lexicon_response(lexicon_request::Method::SearchWithSources(request), op)
    else {
        return Vec::new();
    };
    match response.result {
        Some(lexicon_response::Result::SearchWithSourcesResult(result)) => {
            result.rows.into_iter().map(LexiconRow::from_wire).collect()
        }
        _ => {
            record_failure(op, "response carried no search result");
            Vec::new()
        }
    }
}

/// The Dictionary Search page's hanji-prefix lookup.
pub fn search_by_hanji(
    query: &str,
    mode: InputMode,
    limit: u32,
    enabled_sources_bitmask: u32,
) -> Vec<LexiconRow> {
    let op = "lexiconSearchByHanji";
    let request = SearchByHanjiRequest {
        query: query.to_owned(),
        input_mode: wire_input_mode(mode),
        limit,
        enabled_sources_bitmask,
    };
    let Some(response) = lexicon_response(lexicon_request::Method::SearchByHanji(request), op)
    else {
        return Vec::new();
    };
    match response.result {
        Some(lexicon_response::Result::SearchByHanjiResult(result)) => {
            result.rows.into_iter().map(LexiconRow::from_wire).collect()
        }
        _ => {
            record_failure(op, "response carried no search result");
            Vec::new()
        }
    }
}

/// Whether `text` is a hanji query.
pub fn is_hanji(text: &str) -> bool {
    let op = "isHanji";
    let request = IsHanjiRequest {
        text: text.to_owned(),
    };
    let Some(response) = lexicon_response(lexicon_request::Method::IsHanji(request), op) else {
        return false;
    };
    match response.result {
        Some(lexicon_response::Result::IsHanjiResult(result)) => result.is_hanji,
        _ => {
            record_failure(op, "response carried no is-hanji result");
            false
        }
    }
}

fn lexicon_response(method: lexicon_request::Method, op: &str) -> Option<LexiconResponse> {
    let payload = request::Payload::Lexicon(LexiconRequest {
        method: Some(method),
    });
    match roundtrip(payload, op, 0, None)? {
        response::Payload::Lexicon(response) => Some(response),
        other => {
            record_failure(op, &format!("expected a lexicon payload, got {other:?}"));
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(index: i64, score: Option<i32>, bitmask: u32) -> LexiconRow {
        LexiconRow {
            id: index,
            roman: format!("r{index}"),
            hanji: None,
            length_score: score,
            source_bitmask: Some(bitmask),
        }
    }

    /// One toggle: its name, reading it, flipping it, and reading its wire
    /// field.
    type ToggleField = (
        &'static str,
        fn(&DictionarySourceToggles) -> bool,
        fn(&mut DictionarySourceToggles),
        fn(&WireDictionarySourceToggles) -> bool,
    );

    /// Every toggle `dictionary_toggles` copies. A field added to the
    /// settings fails to compile in the destructure below — give it a row
    /// there too.
    fn toggle_fields() -> Vec<ToggleField> {
        macro_rules! source {
            ($field:ident) => {
                (
                    stringify!($field),
                    |toggles: &DictionarySourceToggles| toggles.$field,
                    |toggles: &mut DictionarySourceToggles| toggles.$field = !toggles.$field,
                    |wire: &WireDictionarySourceToggles| wire.$field,
                )
            };
        }
        macro_rules! subcollection {
            ($field:ident) => {
                (
                    stringify!($field),
                    |toggles: &DictionarySourceToggles| toggles.kautian_subcollections.$field,
                    |toggles: &mut DictionarySourceToggles| {
                        toggles.kautian_subcollections.$field =
                            !toggles.kautian_subcollections.$field
                    },
                    |wire: &WireDictionarySourceToggles| {
                        wire.kautian_subcollections
                            .as_ref()
                            .is_some_and(|subcollections| subcollections.$field)
                    },
                )
            };
        }
        let DictionarySourceToggles {
            kautian: _,
            taigitv: _,
            itaigi: _,
            sitbut: _,
            taihoa: _,
            taijit: _,
            kungge: _,
            stti: _,
            khpoo: _,
            variant: _,
            khiin: _,
            lkk: _,
            dev: _,
            kautian_subcollections:
                crate::settings::KautianSubcollections {
                    accent_lukang: _,
                    accent_sansia: _,
                    accent_taipak: _,
                    accent_gilan: _,
                    accent_tainan: _,
                    accent_kaohsiung: _,
                    accent_kinmen: _,
                    accent_makung: _,
                    accent_sintik: _,
                    accent_taichung: _,
                    name_appendix: _,
                },
        } = DictionarySourceToggles::DEFAULT;
        vec![
            source!(kautian),
            source!(taigitv),
            source!(itaigi),
            source!(sitbut),
            source!(taihoa),
            source!(taijit),
            source!(kungge),
            source!(stti),
            source!(khpoo),
            source!(variant),
            source!(khiin),
            source!(lkk),
            source!(dev),
            subcollection!(accent_lukang),
            subcollection!(accent_sansia),
            subcollection!(accent_taipak),
            subcollection!(accent_gilan),
            subcollection!(accent_tainan),
            subcollection!(accent_kaohsiung),
            subcollection!(accent_kinmen),
            subcollection!(accent_makung),
            subcollection!(accent_sintik),
            subcollection!(accent_taichung),
            subcollection!(name_appendix),
        ]
    }

    /// The 24 flags are copied field by field, and a swapped or inverted
    /// pair compiles and filters the wrong dictionary: every wire field
    /// carries its own toggle's value, at the defaults and with any one
    /// toggle flipped. A subcollection read from an absent message is off,
    /// so this also pins that the message is always sent (absent, the engine
    /// treats every subcollection as on). Ported from the Swift encoder's tests
    /// (`RustEngineBridgeDictionaryTogglesTests`) when macOS stopped
    /// encoding the toggles itself (roadmap P13).
    #[test]
    fn each_toggle_lands_on_its_own_wire_field() {
        let fields = toggle_fields();
        let defaults = DictionarySourceToggles::DEFAULT;
        let one_flipped = fields.iter().map(|(_, _, flip, _)| {
            let mut toggles = defaults.clone();
            flip(&mut toggles);
            toggles
        });
        for toggles in std::iter::once(defaults.clone()).chain(one_flipped) {
            let wire = dictionary_toggles(&toggles);
            for (name, read, _, wire_field) in &fields {
                assert_eq!(wire_field(&wire), read(&toggles), "{name} in {toggles:?}");
            }
        }
    }

    #[test]
    fn a_bitmask_decodes_in_bit_order_and_skips_the_non_source_bits() {
        // trace: bits 0 (kautian), 9 (khiin), 11 (lkk), 12 (variant: not a
        // badge source) → [Kautian, Khiin, Lkk].
        assert_eq!(
            DictionarySource::from_bitmask((1 << 0) | (1 << 9) | (1 << 11) | (1 << 12)),
            vec![
                DictionarySource::Kautian,
                DictionarySource::Khiin,
                DictionarySource::Lkk
            ]
        );
        assert!(DictionarySource::from_bitmask(0).is_empty());
    }

    #[test]
    fn search_order_puts_kautian_first_then_length_score_then_engine_order() {
        // trace: DictionarySearchService.Ordering — kautian beats a higher
        // score; among kautian rows the higher score wins; ties keep order.
        let rows = vec![
            row(0, Some(9), 1 << 1),
            row(1, Some(2), 1 << 0),
            row(2, Some(5), 1 << 0),
            row(3, Some(5), 1 << 0),
            row(4, None, 1 << 2),
        ];
        let ids: Vec<i64> = LexiconRow::sorted_for_search(rows)
            .into_iter()
            .map(|row| row.id)
            .collect();
        assert_eq!(ids, vec![2, 3, 1, 0, 4]);
    }

    #[test]
    fn is_hanji_answers_through_the_engine() {
        assert!(is_hanji("台語"));
        assert!(!is_hanji("tai"));
    }

    #[test]
    fn source_codes_decode_by_explicit_match() {
        assert_eq!(
            DictionarySource::from_code(1),
            Some(DictionarySource::Kautian)
        );
        assert_eq!(
            DictionarySource::from_code(13),
            Some(DictionarySource::Custom)
        );
        assert_eq!(DictionarySource::from_code(0), None);
        assert_eq!(DictionarySource::from_code(999), None);
    }
}
