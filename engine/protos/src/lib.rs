//! Generated protobuf bindings for the Taigi engine wire types.
//!
//! Hand-written shell crate: every type is `include!`'d from the prost-build
//! output (`$OUT_DIR/taigi.engine.rs`), regenerated each build by `build.rs`
//! from `engine/protos/proto/{envelope,phonetics,composing,lexicon,nextword,
//! case}.proto`. Platform-side bindings (iOS `.pb.swift`, Android `.java`)
//! are NOT generated here — see `engine/scripts/gen-platform-protos.sh`.
//!
//! `clippy::all` + `clippy::pedantic` are silenced because the included
//! prost output lints noisily and is regenerated on every build.

#![allow(clippy::all, clippy::pedantic)]

pub mod engine {
    include!(concat!(env!("OUT_DIR"), "/taigi.engine.rs"));
}

impl engine::AppConfig {
    /// Whether candidate cells render romanization only (Candidate Display = Romanization Only).
    ///
    /// The single normalisation point for `candidate_display_mode`: the
    /// proto3 default `0`, an unknown value from a newer platform, and
    /// `SIDE_BY_SIDE` all answer `false` (legacy behaviour), so the two
    /// engine readers can never drift on the fallback.
    pub fn is_roman_only_display(&self) -> bool {
        // prost's accessor already maps an unknown value to `Unspecified`.
        self.candidate_display_mode() == engine::CandidateDisplayMode::RomanOnly
    }

    /// Whether every candidate cell shows ONE script (Romanization Only, or Hanji with Romanization's
    /// split cells) — the displays under which a cell that reads like an
    /// earlier one is collapsed into it (§42 / §44), so the collapsed row's
    /// identity has to survive on the survivor. For Hanji with Romanization the collapse is the
    /// platform's (§42 split) and the engine relies on it keeping the FIRST
    /// roman cell — the slot-0 §34 literal — as the survivor. Pairing (and the
    /// same fallbacks as [`Self::is_roman_only_display`]) answer `false`.
    pub fn is_single_script_display(&self) -> bool {
        matches!(
            self.candidate_display_mode(),
            engine::CandidateDisplayMode::RomanOnly | engine::CandidateDisplayMode::Combined
        )
    }

    /// Whether the keyboard is on the TPS (Bopomofo) layout: `input_mode` is
    /// `"tps"` (either spelling `phonetics::api::parse_input_mode` accepts). A
    /// pre-R6 platform sends TPS as `"tl"` with the fold already applied to the
    /// swap / hyphenless flags, which the two readers below then pass through.
    pub fn is_tps_layout(&self) -> bool {
        matches!(self.input_mode.as_str(), "tps" | "TPS")
    }

    /// Whether the composition renders Hanji first: the swap, or the TPS
    /// layout (which shows Hanji / Bopomofo, never the romanization).
    pub fn renders_hanji_first(&self) -> bool {
        self.is_hanji_first || self.is_tps_layout()
    }

    /// Whether No Hyphens applies: never on the TPS layout, whose platform
    /// re-splits the candidate `roman` on `-` to render Bopomofo (§49).
    pub fn renders_hyphenless(&self) -> bool {
        self.hyphenless_roman && !self.is_tps_layout()
    }
}

#[cfg(test)]
mod tests {
    use super::engine::AppConfig;

    fn config(input_mode: &str, swapped: bool, hyphenless: bool) -> AppConfig {
        AppConfig {
            input_mode: input_mode.to_owned(),
            is_hanji_first: swapped,
            hyphenless_roman: hyphenless,
            ..AppConfig::default()
        }
    }

    #[test]
    fn tps_layout_is_the_tps_input_mode_only() {
        assert!(config("tps", false, false).is_tps_layout());
        assert!(config("TPS", false, false).is_tps_layout());
        for mode in ["tl", "poj", "english", "", "Tps"] {
            assert!(!config(mode, false, false).is_tps_layout(), "{mode:?}");
        }
    }

    #[test]
    fn tps_layout_renders_hanji_first_whatever_the_stored_swap() {
        assert!(config("tps", false, false).renders_hanji_first());
        assert!(config("tps", true, false).renders_hanji_first());
        assert!(config("tl", true, false).renders_hanji_first());
        assert!(!config("tl", false, false).renders_hanji_first());
        assert!(!config("poj", false, false).renders_hanji_first());
    }

    #[test]
    fn tps_layout_never_renders_hyphenless() {
        assert!(!config("tps", false, true).renders_hyphenless());
        assert!(!config("tps", false, false).renders_hyphenless());
        assert!(config("tl", false, true).renders_hyphenless());
        assert!(config("poj", true, true).renders_hyphenless());
        assert!(!config("tl", false, false).renders_hyphenless());
    }
}
