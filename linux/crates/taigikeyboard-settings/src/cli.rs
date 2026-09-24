//! The command line the engine spawns this window with (`settings::launch`,
//! one contract for both sides): which pane to open on, and whether to run
//! the manual update check on arrival. The flags with no Linux behaviour
//! are refused with a readable error rather than ignored (roadmap L8).

use std::fmt;
use taigi_desktop_core::settings::launch::{
    CHECK_NOW_FLAG, CHECK_UPDATES_FLAG, PANE_FLAG, PREWARM_FLAG,
};
use taigi_desktop_core::settings::{SettingChoice, SettingsPane};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LaunchOptions {
    /// `--pane <raw>`; an unknown raw value is ignored (the stored pane wins).
    pub pane: Option<SettingsPane>,
    /// `--check-now`: the panel menu's 檢查更新.
    pub check_now: bool,
}

/// A flag this platform has no behaviour for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnsupportedFlag(pub String);

impl fmt::Display for UnsupportedFlag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "`{}` is not supported on Linux; only `{PANE_FLAG} <pane>` and `{CHECK_NOW_FLAG}` are accepted",
            self.0
        )
    }
}

impl std::error::Error for UnsupportedFlag {}

impl LaunchOptions {
    pub fn parse(arguments: impl IntoIterator<Item = String>) -> Result<Self, UnsupportedFlag> {
        let mut options = Self::default();
        let mut arguments = arguments.into_iter();
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                PANE_FLAG => {
                    options.pane = arguments
                        .next()
                        .and_then(|raw| SettingsPane::from_raw(&raw));
                }
                CHECK_NOW_FLAG => options.check_now = true,
                CHECK_UPDATES_FLAG | PREWARM_FLAG => {
                    return Err(UnsupportedFlag(argument));
                }
                other => log::warn!("cli.unknown_argument argument={other}"),
            }
        }
        Ok(options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(arguments: &[&str]) -> Result<LaunchOptions, UnsupportedFlag> {
        LaunchOptions::parse(arguments.iter().map(|s| (*s).to_owned()))
    }

    #[test]
    fn the_launcher_contract_round_trips() {
        // trace: launcher::open_settings(Some("about")) spawns `--pane about`.
        assert_eq!(
            parse(&["--pane", "about"]).unwrap().pane,
            Some(SettingsPane::About)
        );
        assert_eq!(parse(&[]).unwrap(), LaunchOptions::default());
        assert_eq!(parse(&["--pane", "bogus"]).unwrap().pane, None);
        assert_eq!(parse(&["--pane"]).unwrap().pane, None);
    }

    #[test]
    fn the_menu_check_arrives_on_general() {
        // trace: launcher::check_for_updates() spawns
        // `--pane general --check-now`.
        let options = parse(&["--pane", "general", "--check-now"]).unwrap();
        assert_eq!(options.pane, Some(SettingsPane::General));
        assert!(options.check_now);
        assert!(!parse(&["--pane", "about"]).unwrap().check_now);
    }

    #[test]
    fn the_flags_without_linux_behaviour_are_refused_by_name() {
        let error = parse(&["--check-updates"]).unwrap_err();
        assert_eq!(error, UnsupportedFlag("--check-updates".into()));
        assert!(error.to_string().contains("--check-updates"));
        assert!(parse(&["--prewarm"]).is_err());
    }
}
