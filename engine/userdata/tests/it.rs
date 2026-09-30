//! User-data store integration tests — one binary, one link (`Cargo.toml`
//! `autotests = false`); each `tests/<file>.rs` stays in place as a module
//! of this root. Every test opens its stores in its own temp directory.

mod common;

mod backup;
mod stores;
mod takeover;

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
