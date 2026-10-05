//! Document-text policies the controller applies around a commit:
//! auto-space and full-width punctuation. The decisions are the one copy the
//! three desktops run; the auto-space attaching set is the engine's, and
//! macOS keeps a Swift twin of the full-width map for the width-flip check
//! its controller makes itself.

mod auto_space;
mod full_width;

pub use auto_space::{
    augment_insert, is_gate_active, raw_preedit_writes_romanization, should_append_space,
    AugmentedInsert,
};
pub use full_width::{document_punctuation, full_width_mapped, is_width_flip_key};
