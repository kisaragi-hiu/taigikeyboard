//! Integration tests over the production lexicon (`dictionaries/`),
//! installed once per process — never next to a hermetic fixture install.

mod common;

mod candidate_dump;
mod continuous_context_rerank;
mod cross_mode_parity;
mod tps_wire_equivalence;
