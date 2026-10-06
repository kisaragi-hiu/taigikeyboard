//! Input classification primitives.
//!
//! - `is_hanji` — the Tab3 search short-circuit predicate (lexicon proto
//!   dispatch) and the Continuous fetch's Hanji check. Lives in
//!   `phonetics` (`engine/phonetics/src/hanji.rs`) so the user-data stores
//!   read the same predicate without depending on the lexicon; re-exported
//!   here for the lexicon's callers.

pub use phonetics::api::{is_hanji, CJK_RANGES};
