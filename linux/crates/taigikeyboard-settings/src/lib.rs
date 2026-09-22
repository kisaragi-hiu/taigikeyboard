//! The settings window as a library, so the pane-mount test can build the
//! pages without a running application (roadmap L12).

pub mod cli;
pub mod pages;
pub mod presentation;
pub mod window;
pub mod writer;

use adw::prelude::*;
use cli::LaunchOptions;
use std::cell::RefCell;
use std::rc::Rc;
use taigi_desktop_core::settings::{keys, SettingsPane};
use window::SettingsWindow;

/// The application id: one instance per session (`gio::Application`
/// forwards a second launch's command line to the first, roadmap L8).
pub const APPLICATION_ID: &str = "tw.taigikeyboard.Settings";

/// Runs the application to its exit code.
pub fn run() -> gtk::glib::ExitCode {
    let application = adw::Application::builder()
        .application_id(APPLICATION_ID)
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    let window: Rc<RefCell<Option<Rc<SettingsWindow>>>> = Rc::new(RefCell::new(None));
    application.connect_command_line(move |application, command_line| {
        let arguments: Vec<String> = command_line
            .arguments()
            .into_iter()
            .skip(1)
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();
        let launch = match LaunchOptions::parse(arguments) {
            Ok(launch) => launch,
            Err(error) => {
                // To the CALLER's stderr: a second launch's command line is
                // handled in the first process.
                log::error!("cli.refused error={error}");
                command_line.printerr_literal(&format!("taigikeyboard-settings: {error}\n"));
                return gtk::glib::ExitCode::from(2);
            }
        };
        // The first launch builds the window; a later one (the menu row
        // pressed again, another `--pane`) re-activates it on that pane —
        // the Windows single-instance mutex's contract, native here.
        let existing = window.borrow().clone();
        let shell = match existing {
            Some(shell) => shell,
            None => {
                let shell = SettingsWindow::build(application, writer::SettingsWriter::at_launch());
                *window.borrow_mut() = Some(Rc::clone(&shell));
                shell
            }
        };
        let pane = launch.pane.unwrap_or_else(|| {
            shell
                .writer()
                .borrow()
                .document()
                .choice(&keys::SELECTED_SETTINGS_PANE)
        });
        shell.show(pane);
        gtk::glib::ExitCode::SUCCESS
    });
    application.run()
}

/// The panes the sidebar lists on Linux, in order: the Mac's roster minus
/// 字型管理 (the panel draws with the desktop's font, roadmap L4), and —
/// until their PRs land — minus 快捷鍵 / 詞庫來源 / 自訂詞庫 (PR7, PR8).
pub const SIDEBAR: [SettingsPane; 2] = [SettingsPane::General, SettingsPane::Appearance];
