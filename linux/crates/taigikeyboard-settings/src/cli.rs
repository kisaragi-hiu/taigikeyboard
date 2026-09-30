//! The command line the engine spawns this window with (`settings::launch`,
//! one contract and one parser for both desktops): which pane to open on.
//! The Windows-only flags are refused with a readable error rather than
//! ignored (roadmap L8).

use std::fmt;
use taigi_desktop_core::settings::launch::{LaunchOptions, PANE_FLAG};

/// A flag this platform has no behaviour for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnsupportedFlag(pub &'static str);

impl fmt::Display for UnsupportedFlag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "`{}` is not supported on Linux (updates are the package manager's); only `{PANE_FLAG} <pane>` is accepted",
            self.0
        )
    }
}

impl std::error::Error for UnsupportedFlag {}

/// The launch the command line asks for, or the first Windows-only flag
/// it carried.
pub fn parse(
    arguments: impl IntoIterator<Item = String>,
) -> Result<LaunchOptions, UnsupportedFlag> {
    let options = LaunchOptions::parse(arguments);
    match options.first_windows_only_flag {
        Some(flag) => Err(UnsupportedFlag(flag)),
        None => Ok(options),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use taigi_desktop_core::settings::SettingsPane;

    fn parse(arguments: &[&str]) -> Result<LaunchOptions, UnsupportedFlag> {
        super::parse(arguments.iter().map(|s| (*s).to_owned()))
    }

    #[test]
    fn a_pane_launch_is_accepted() {
        // trace: launcher::open_settings(Some("about")) spawns `--pane about`.
        assert_eq!(
            parse(&["--pane", "about"]).unwrap().pane,
            Some(SettingsPane::About)
        );
    }

    #[test]
    fn the_windows_only_flags_are_refused_by_name() {
        let error = parse(&["--check-now"]).unwrap_err();
        assert_eq!(error, UnsupportedFlag("--check-now"));
        assert_eq!(
            error.to_string(),
            "`--check-now` is not supported on Linux (updates are the package manager's); only `--pane <pane>` is accepted"
        );
        assert!(parse(&["--prewarm"]).is_err());
        assert!(parse(&["--check-updates"]).is_err());
    }
}
