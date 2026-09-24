//! Opening the settings window and the browser from the engine process
//! (roadmap L8). The command line is the shared
//! `taigi_desktop_core::settings::launch` contract, so the engine and the
//! settings binary cannot drift on the flag spelling.

use crate::paths::settings_binary;
use std::path::Path;
use std::process::{Command, Stdio};
use taigi_desktop_core::settings::launch::{CHECK_NOW_FLAG, CHECK_UPDATES_FLAG, PANE_FLAG};
use taigi_desktop_core::settings::{SettingChoice, SettingsPane};

/// Spawns the settings window, on `pane` (the persisted raw spelling of a
/// `SettingsPane`) or wherever the user left it. Detached: no pipe, no wait —
/// the engine must not block the daemon's key path on a window. `false`
/// when the binary could not be started (logged with the path, which is
/// the first thing a broken install shows).
pub fn open_settings(pane: Option<&str>) -> bool {
    open_settings_at(&settings_binary(), pane)
}

pub fn open_settings_at(binary: &Path, pane: Option<&str>) -> bool {
    match pane {
        Some(pane) => spawn_settings(binary, &[PANE_FLAG, pane]),
        None => spawn_settings(binary, &[]),
    }
}

/// The menu's 檢查更新: the window on 一般, running the manual check there
/// (Windows `settings_launcher::check_for_updates`).
pub fn check_for_updates() -> bool {
    check_for_updates_at(&settings_binary())
}

pub fn check_for_updates_at(binary: &Path) -> bool {
    spawn_settings(
        binary,
        &[PANE_FLAG, SettingsPane::General.raw(), CHECK_NOW_FLAG],
    )
}

/// The automatic daily check (roadmap L10): the settings binary with no
/// window, `--check-updates` — it claims the due window, fetches, and posts
/// a desktop notification for a version not announced before.
pub fn check_for_updates_in_background() -> bool {
    spawn_settings(&settings_binary(), &[CHECK_UPDATES_FLAG])
}

fn spawn_settings(binary: &Path, arguments: &[&str]) -> bool {
    let mut command = Command::new(binary);
    command.args(arguments);
    spawn_detached(command, "settings")
}

/// Opens `url` with the desktop's handler (`xdg-open`, the freedesktop
/// portal-aware launcher every desktop ships). `false` when it could not be
/// started, so the caller can say so rather than do nothing.
pub fn open_url(url: &str) -> bool {
    let mut command = Command::new("xdg-open");
    command.arg(url);
    spawn_detached(command, "xdg-open")
}

fn spawn_detached(mut command: Command, what: &str) -> bool {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    match command.spawn() {
        Ok(mut child) => {
            // Reaped on its own thread, for every launch (the settings
            // window and `xdg-open` too, which were left unreaped before the
            // daily check): the engine and the Fcitx5 daemon live for the
            // whole session, and a child never waited on stays a zombie
            // until they exit.
            let reaped = std::thread::Builder::new()
                .name("taigi-reap".to_owned())
                .spawn(move || {
                    let _ = child.wait();
                });
            if let Err(error) = reaped {
                log::warn!("launch.{what}_unreaped error={error}");
            }
            true
        }
        Err(error) => {
            log::error!(
                "launch.{what}_failed program={:?} error={error}",
                command.get_program()
            );
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_binary_answers_false_rather_than_panicking() {
        assert!(!open_settings_at(
            Path::new("/nonexistent/taigikeyboard-settings"),
            Some("general")
        ));
        assert!(!check_for_updates_at(Path::new(
            "/nonexistent/taigikeyboard-settings"
        )));
    }
}
