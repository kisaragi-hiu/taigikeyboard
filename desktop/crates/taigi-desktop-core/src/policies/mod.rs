//! Pure document-text policies the controller applies around a commit:
//! auto-space and full-width punctuation, the one copy the three desktops run.

mod auto_space;
mod full_width;

pub use auto_space::{
    augment_insert, is_attaching_punctuation, is_gate_active, raw_preedit_writes_romanization,
    should_append_space, AugmentedInsert,
};
pub use full_width::{document_punctuation, full_width_mapped};
