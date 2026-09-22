//! `include/taigikeyboard.h` is hand-written; this pins two things about it
//! on every host that has a C compiler: it parses as C (and as C++, the
//! addon's language), and every function it declares is exported by this
//! crate under exactly that name — a declaration the library does not
//! provide would only show as a link error in the addon's CMake build,
//! which the macOS host never runs.

use std::path::PathBuf;
use std::process::Command;

fn header() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("include/taigikeyboard.h")
}

fn compiler_available(compiler: &str) -> bool {
    Command::new(compiler)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

#[test]
fn the_header_parses_as_c_and_as_cxx() {
    for (compiler, language) in [("cc", "c"), ("c++", "c++")] {
        if !compiler_available(compiler) {
            eprintln!("skipping: no `{compiler}` on this host");
            continue;
        }
        let output = Command::new(compiler)
            .args([
                "-fsyntax-only",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-x",
                language,
            ])
            .arg(header())
            .output()
            .expect("compiler runs");
        assert!(
            output.status.success(),
            "{compiler} rejected the header:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn every_declared_function_is_exported_by_this_crate() {
    // trace: the declarations are `<type> taigi_<name>(...)`; the exports are
    // the `#[no_mangle]` items in src/lib.rs, matched by name.
    let header = std::fs::read_to_string(header()).expect("header readable");
    let declared: Vec<&str> = header
        .lines()
        .filter_map(|line| {
            let start = line.find("taigi_")?;
            let rest = &line[start..];
            let end = rest.find('(')?;
            // Skip the enum / macro lines: a declaration has a return type
            // before the name and a `(` right after it.
            if line.trim_start().starts_with("#define") || line.contains("TAIGI_") {
                return None;
            }
            Some(&rest[..end])
        })
        .collect();
    assert!(
        declared.len() >= 20,
        "found {} declarations",
        declared.len()
    );
    let source =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
            .expect("source readable");
    for name in declared {
        assert!(
            source.contains(&format!("fn {name}(")),
            "{name} is declared in the header but not exported"
        );
    }
}
