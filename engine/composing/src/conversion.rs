//! Hanji conversion of a TPS pending tail
//! (`docs/architecture/desktop-tps-hanji-conversion-roadmap.md` H1, H2): the
//! walker's best path over the closed readings, kept in `Phase::Continuous`
//! and shown in the preedit in front of the glyphs of the reading still being
//! typed.
//!
//! A reading is closed by its tone mark, or by Space for tones 1 and 4. The
//! closed part is the longest prefix of the tail ending on such a boundary
//! that the syllabifier reaches from the start of the tail; it is walked as
//! a buffer of its own, so its trailing Space keeps the §41 tone pin and a
//! tail the whole-buffer walk cannot span still converts what is closed.
//!
//! Only a tail edited at its end converts: with the caret inside the tail
//! the preedit shows the glyphs, one displayed character per raw character,
//! so the caret the host draws is the place the next key edits.

use lexicon::{ContinuousFetchCtx, EngineHandle as LexiconHandle, SyllableInventory};
use phonetics::InputMode;
use protos::engine::AppConfig;

use crate::continuous::{synth_consumed_span, walk_buffer};
use crate::derived::{buffer_input_mode, tps_slice_display};
use crate::shadow::{
    build_continuous_keys, build_shadow_lattice_with_barriers, whole_buffer_tone_pin, ShadowLattice,
};

/// The converted closed part of a pending tail. Present only while the
/// caret is at the end of the tail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conversion {
    /// The walker's words, contiguous over the closed part and never empty.
    pub segments: Vec<ConvertedSegment>,
    /// The dictionary source filter the walk ran with. A request asking for
    /// another filter neither shows nor re-uses this conversion.
    pub enabled_sources_bitmask: u32,
}

/// One word of the walker's path over the closed part.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConvertedSegment {
    /// Byte offsets in the pending raw. A separator typed before the word
    /// belongs to it, as a nailed segment's `raw_text` holds it.
    pub raw_span: (usize, usize),
    /// The word's Hanji, or its glyphs when the dictionary has none.
    pub display_text: String,
}

impl Conversion {
    /// Byte offset in the pending raw where the closed part ends; what
    /// follows is the open reading.
    pub fn closed_end(&self) -> usize {
        self.segments.last().map_or(0, |segment| segment.raw_span.1)
    }

    /// Whether `config` asks for the conversion this one was walked for.
    fn is_for(&self, config: &AppConfig) -> bool {
        requested_sources(config) == Some(self.enabled_sources_bitmask)
    }

    /// The pending tail as the preedit shows it: the words, then the glyphs
    /// of the open reading. `None` when `config` does not ask for this
    /// conversion.
    pub(crate) fn tail_display(&self, raw: &str, config: &AppConfig) -> Option<String> {
        if !self.is_for(config) {
            return None;
        }
        let mut display: String = self
            .segments
            .iter()
            .map(|segment| segment.display_text.as_str())
            .collect();
        display.push_str(&tps_slice_display(&raw[self.closed_end()..]));
        Some(display)
    }
}

/// The source filter `config` asks a conversion with — every source when it
/// names no toggles, as a `FetchAtPos` without them — or `None` when it asks
/// for none.
fn requested_sources(config: &AppConfig) -> Option<u32> {
    config
        .hanji_conversion
        .as_ref()
        .map(|conversion| crate::requests::source_filter_bitmask(conversion.toggles.as_ref()))
}

/// The conversion of the pending `raw` under `config`, or `None`: the
/// request does not ask for one, the buffer is not TPS, the caret is inside
/// the tail, no reading is closed, or the walk finds no path (no lexicon
/// installed included). `previous` is the pending raw and conversion being
/// replaced; its words are kept when the closed text and the source filter
/// are the same, so a glyph of an open reading walks nothing.
pub(crate) fn convert(
    previous: Option<(&str, &Conversion)>,
    raw: &str,
    caret: usize,
    config: &AppConfig,
) -> Option<Conversion> {
    let enabled_sources_bitmask = requested_sources(config)?;
    if caret != raw.len() || buffer_input_mode(raw, config) != InputMode::Tps {
        return None;
    }
    // One read scope for the boundary and the walk: both read the same
    // inventory and dictionary.
    LexiconHandle::with_state(|state| {
        let Some(inventory) = state.syllable_inventory.as_ref() else {
            return Ok(None);
        };
        let Some(closed_end) = closed_boundary(raw, inventory) else {
            return Ok(None);
        };
        let closed = &raw[..closed_end];
        if let Some((previous_raw, previous)) = previous {
            if previous.is_for(config) && previous_raw.get(..previous.closed_end()) == Some(closed)
            {
                return Ok(Some(previous.clone()));
            }
        }
        let (Some(prefix_index), Some(dict)) =
            (state.prefix_index.as_ref(), state.dictionary.as_ref())
        else {
            return Ok(None);
        };
        // Neutral ranking: no user rows and no previous-word context.
        let ctx = ContinuousFetchCtx {
            enabled_sources_bitmask,
            freq_map: &ranking::FrequencyMap::new(),
            context: ranking::ContextRanks::empty(),
            now_ms: 0,
            custom: &[],
            learned: &[],
            prefix_index,
            dict,
            mode: InputMode::Tps,
            tone_pin: whole_buffer_tone_pin(closed, InputMode::Tps),
        };
        Ok(
            walk_closed_part(closed, inventory, &ctx).map(|segments| Conversion {
                segments,
                enabled_sources_bitmask,
            }),
        )
    })
    .ok()
    .flatten()
}

/// Where the closed part of `raw` ends: the furthest syllable boundary the
/// lattice reaches from the start that closes a reading — a tone mark ends
/// the syllable before it, or the separator was typed right after it — with
/// that separator run, which belongs to the reading it closes. A separator
/// typed after a hyphen (`ㄒㄧ- `) closes nothing: the hyphen run stays
/// pending, and a closed part cut before it would lose the separator and
/// with it the §41 tone pin.
/// `None` when no reading is closed: an open reading only, an orphan tone
/// mark, a tail the syllabifier rejects.
///
/// Every lattice edge starts at an offset reachable from 0, so its end is
/// reachable too. The lattice is the whole tail's: a boundary it rejects
/// because of what follows (a tone mark typed after the separator) is not
/// closed, even though the prefix alone would span.
fn closed_boundary(raw: &str, inventory: &SyllableInventory) -> Option<usize> {
    let ShadowLattice {
        shadow,
        shadow_to_raw_end,
        lattice,
        space_barriers,
        ..
    } = build_shadow_lattice_with_barriers(raw, inventory, InputMode::Tps);
    // The raw end of the reading ending at shadow offset `end`, its
    // separator run included. An interior run follows the mapped offset; a
    // trailing one is already inside it (`shadow_to_raw_end`'s full-span end).
    let closed_end_at = |end: usize| {
        let reading_end = shadow_to_raw_end[end];
        let separator_run = raw[reading_end..]
            .bytes()
            .take_while(|&byte| byte == b' ')
            .count();
        let closed_end = reading_end + separator_run;
        let has_tone_mark = shadow[..end]
            .chars()
            .next_back()
            .is_some_and(phonetics::is_tps_tone_mark);
        let has_separator = space_barriers.contains(&end) && raw[..closed_end].ends_with(' ');
        (has_tone_mark || has_separator).then_some(closed_end)
    };
    lattice
        .edges()
        .iter()
        .filter_map(|&(_, end)| closed_end_at(end))
        .max()
}

/// The walker's best path over `closed`, as words in raw coordinates. `None`
/// when no edge chain spans it.
fn walk_closed_part(
    closed: &str,
    inventory: &SyllableInventory,
    ctx: &ContinuousFetchCtx<'_>,
) -> Option<Vec<ConvertedSegment>> {
    let continuous_keys = build_continuous_keys(closed, inventory, InputMode::Tps);
    let shadow_to_raw_end = &continuous_keys.shadow_to_raw_end;
    // The path spans the shadow; it covers the closed part only when the
    // shadow's end maps to its end — the slot-0 synthesis's own gate.
    synth_consumed_span(
        shadow_to_raw_end,
        continuous_keys.shadow.len(),
        closed.len() as u32,
    )?;
    let path = walk_buffer(&continuous_keys, inventory, ctx)?;
    Some(
        path.edges
            .iter()
            .zip(path.choices)
            .map(|(&(start, end), choice)| {
                let raw_span = (shadow_to_raw_end[start], shadow_to_raw_end[end]);
                let display_text = choice
                    .hanji
                    .unwrap_or_else(|| tps_slice_display(&closed[raw_span.0..raw_span.1]));
                ConvertedSegment {
                    raw_span,
                    display_text,
                }
            })
            .collect(),
    )
}
