//! The fontconfig side of 字型管理 on Linux (roadmap L4): which bundled
//! typefaces this install carries, and taking a user's font file into the
//! per-user font directory fontconfig scans, where every program of this
//! user — the framework's panel included — can fall back to it. fontconfig
//! itself (`fc-query` / `fc-list`, `Depends: fontconfig`) answers whether a
//! file is a typeface and which families are installed: the same authority
//! the panel draws through.
//!
//! Blocking: the functions that run a process or copy a file are for a
//! background job (`jobs::spawn`).

use std::collections::HashSet;
use std::path::Path;
use std::process::Command;
use taigi_desktop_core::settings::{CandidateFontChoice, SettingChoice};
use taigi_desktop_storage as storage;
use taigi_linux_platform::{InstallLayout, UserDirectories};

/// One family name per line, every alias of every face: `%{family}` alone
/// joins them with `,` and leaves a comma inside a name unescaped.
const FAMILY_PER_LINE: &str = "--format=%{[]family{%{family}\n}}";

/// The bundled typefaces whose installed file is present, in roster order.
/// A development tree run without `make install` has none, and the pane
/// then lists none rather than claiming them.
pub fn installed_bundled_fonts(layout: &InstallLayout) -> Vec<CandidateFontChoice> {
    CandidateFontChoice::ALL
        .iter()
        .copied()
        .filter(|choice| {
            choice
                .file_name()
                .is_some_and(|file_name| layout.bundled_font_file(file_name).is_file())
        })
        .collect()
}

#[derive(Debug, PartialEq, Eq)]
pub enum ImportOutcome {
    /// Stored under this file name.
    Added(String),
    /// fontconfig already has one of the file's families; nothing was kept.
    AlreadyInstalled,
}

/// Takes `source` into the font directory and checks the COPY — the bytes
/// fontconfig will read, not a file that may change after the check (the
/// Windows pane's `take_in`). A copy fontconfig cannot read, or whose family
/// is installed already, is taken back out. The installed families are read
/// BEFORE the copy exists: a scan afterwards would find the copy itself.
pub fn import(source: &Path, directories: &UserDirectories) -> Result<ImportOutcome, String> {
    let installed = installed_families()?;
    let fonts_directory =
        storage::created(directories.fonts.clone()).map_err(|error| error.to_string())?;
    let stored = storage::copy_in(&fonts_directory, source).map_err(|error| error.to_string())?;
    let discard = |outcome: Result<ImportOutcome, String>| match storage::remove_stored(
        &fonts_directory,
        &stored,
    ) {
        Ok(()) => outcome,
        Err(removal) => Err(format!("the copy could not be removed: {removal}")),
    };
    match file_families(&fonts_directory.join(&stored)) {
        Err(error) => discard(Err(error)),
        Ok(families) if families.iter().any(|family| installed.contains(family)) => {
            discard(Ok(ImportOutcome::AlreadyInstalled))
        }
        Ok(_) => {
            refresh_cache(&fonts_directory);
            Ok(ImportOutcome::Added(stored))
        }
    }
}

/// Deletes one added typeface.
pub fn remove(file_name: &str, directories: &UserDirectories) -> Result<(), String> {
    storage::remove_stored(&directories.fonts, file_name).map_err(|error| error.to_string())?;
    refresh_cache(&directories.fonts);
    Ok(())
}

/// Every family name of every face in `path`; `Err` when fontconfig cannot
/// read it as a typeface.
fn file_families(path: &Path) -> Result<Vec<String>, String> {
    let families = family_lines(&run_fontconfig(
        Command::new("fc-query").arg(FAMILY_PER_LINE).arg(path),
    )?);
    if families.is_empty() {
        return Err("fontconfig found no typeface in the file".to_owned());
    }
    Ok(families)
}

/// Every family name fontconfig lists for this user.
fn installed_families() -> Result<HashSet<String>, String> {
    Ok(family_lines(&run_fontconfig(
        Command::new("fc-list").arg(FAMILY_PER_LINE),
    )?)
    .into_iter()
    .collect())
}

fn family_lines(output: &str) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|family| !family.is_empty())
        .map(str::to_owned)
        .collect()
}

fn run_fontconfig(command: &mut Command) -> Result<String, String> {
    let program = command.get_program().to_string_lossy().into_owned();
    let output = command
        .output()
        .map_err(|error| format!("{program} could not run: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "{program} refused the file: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Rebuilds fontconfig's cache for `directory`, best effort: a process that
/// asks fontconfig later finds the change without rescanning; one already
/// running may keep its own font list until it restarts (the pane's note).
fn refresh_cache(directory: &Path) {
    if let Err(error) = Command::new("fc-cache").arg(directory).status() {
        log::warn!("fonts.cache_not_refreshed error={error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_with_a_comma_stays_one_family() {
        // trace: `%{[]family{%{family}\n}}` over families ["A, B", "C"]
        // prints "A, B\nC\n" — one name per line, the comma untouched.
        assert_eq!(family_lines("A, B\nC\n\n"), vec!["A, B", "C"]);
    }

    #[test]
    fn only_bundled_files_present_under_the_prefix_are_listed() {
        let prefix = tempfile::tempdir().unwrap();
        let layout = InstallLayout::new(prefix.path());
        assert!(installed_bundled_fonts(&layout).is_empty());

        let iansui = layout.bundled_font_file("iansui_regular.ttf");
        std::fs::create_dir_all(iansui.parent().unwrap()).unwrap();
        std::fs::write(&iansui, b"").unwrap();

        assert_eq!(
            installed_bundled_fonts(&layout),
            vec![CandidateFontChoice::Iansui]
        );
    }
}
