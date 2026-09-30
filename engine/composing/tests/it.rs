//! Hermetic integration tests — one binary, one link (`Cargo.toml`
//! `autotests = false`); each `tests/<file>.rs` stays in place as a module
//! of this root. A module that installs a fixture lexicon holds
//! `common::engine_install_lock` from install through its last assertion;
//! the others never depend on what the process-wide lexicon holds. A test
//! on the production lexicon goes to `prod.rs`; one that needs no lexicon
//! installed at all goes to `no_lexicon.rs`.

mod common;

mod build_keys_tl_hyphen;
mod build_keys_tl_lattice;
mod build_keys_tl_poj_diacritic;
mod build_keys_tps;
mod composing_caret;
mod continuous_abbrev;
mod continuous_explicit_tone;
mod continuous_learned_phrase;
mod continuous_partial_tone;
mod continuous_phase;
mod continuous_slot0_dict_roman;
mod continuous_slot0_user_selection;
mod continuous_typed_boundary;
mod continuous_typed_separator;
mod golden_fetch_at_pos;
mod hyphenless_roman_display;
mod intent_coverage;
mod invariants;
mod nasal_marker_case;
mod proptest_sequences;
mod raw_input_pending_tail;
mod roman_only_display_dedup;
mod syllabifier_tl;
mod tps_display_dedup;
mod tps_space_pinned_tone;

/// Every `tests/*.rs` is a `[[test]]` root in `Cargo.toml` or a `mod` of one,
/// so a new test file cannot silently stop running under `autotests = false`.
#[test]
fn every_test_file_belongs_to_a_test_binary() {
    use std::collections::BTreeSet;
    let crate_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = std::fs::read_to_string(crate_dir.join("Cargo.toml")).expect("read Cargo.toml");
    let mut declared = BTreeSet::new();
    for root in manifest.lines().filter_map(|line| {
        line.trim()
            .strip_prefix("path = \"tests/")?
            .strip_suffix(".rs\"")
    }) {
        let source = std::fs::read_to_string(crate_dir.join(format!("tests/{root}.rs")))
            .expect("read test root");
        declared.extend(
            source
                .lines()
                .filter_map(|line| line.strip_prefix("mod ")?.strip_suffix(';'))
                .map(str::to_owned),
        );
        declared.insert(root.to_owned());
    }
    let files: BTreeSet<String> = std::fs::read_dir(crate_dir.join("tests"))
        .expect("read tests/")
        .map(|entry| entry.expect("tests/ entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .map(|path| {
            path.file_stem()
                .expect("file stem")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    let orphans: Vec<_> = files.difference(&declared).collect();
    assert!(
        orphans.is_empty(),
        "tests/<name>.rs not declared by any [[test]] root in Cargo.toml: {orphans:?}"
    );
}
