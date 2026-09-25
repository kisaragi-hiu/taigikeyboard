//! The settings window as a library, so the pane-mount test can build the
//! pages without a running application (roadmap L12).

pub mod cli;
pub mod jobs;
pub mod pages;
pub mod presentation;
pub mod recorder;
pub mod search;
pub mod updates;
pub mod user_data;
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

type WindowSlot = Rc<RefCell<Option<Rc<SettingsWindow>>>>;

/// Runs the application to its exit code.
pub fn run() -> gtk::glib::ExitCode {
    let application = adw::Application::builder()
        .application_id(APPLICATION_ID)
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    let window: WindowSlot = Rc::new(RefCell::new(None));

    let slot = Rc::clone(&window);
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
        // The engine's daily spawn: no window, the check and at most one
        // notification, then the process ends (unless a window is open).
        if launch.check_updates {
            #[cfg(not(feature = "disable-updates"))]
            updates::check_in_background(application);
            return gtk::glib::ExitCode::SUCCESS;
        }
        let shell = present(application, &slot, launch.pane);
        // The panel menu's 檢查更新 (`launcher::check_for_updates`): the
        // window on 一般, then the manual check and its alert over it.
        if launch.check_now {
            #[cfg(not(feature = "disable-updates"))]
            updates::check_manually(&shell);
        }
        gtk::glib::ExitCode::SUCCESS
    });

    // Activation with no command line: the notification's action through
    // D-Bus (`--gapplication-service`, the installed service file), and a
    // plain `Activate` from a launcher that uses D-Bus.
    let slot = Rc::clone(&window);
    let show_updates = gio::ActionEntry::builder(updates::SHOW_UPDATES_ACTION)
        .activate(move |application: &adw::Application, _, _| {
            present(application, &slot, Some(SettingsPane::General));
        })
        .build();
    application.add_action_entries([show_updates]);
    let slot = Rc::clone(&window);
    application.connect_activate(move |application| {
        present(application, &slot, None);
    });
    application.run()
}

/// The window on `pane` (or wherever the user left it): the first call
/// builds it, a later one (the menu row pressed again, another `--pane`)
/// re-activates it — the Windows single-instance mutex's contract, native
/// here.
fn present(
    application: &adw::Application,
    slot: &WindowSlot,
    pane: Option<SettingsPane>,
) -> Rc<SettingsWindow> {
    let existing = slot.borrow().clone();
    let shell = match existing {
        Some(shell) => shell,
        None => {
            // The window's own icon: the hicolor `taigikeyboard`, not one
            // named after the application id (which is not installed).
            gtk::Window::set_default_icon_name("taigikeyboard");
            let writer = writer::SettingsWriter::at_launch();
            // The stores follow the settings: no user directory, no
            // learning data either (the banner says so).
            let stores = if writer.is_read_only() {
                Err("HOME / XDG_DATA_HOME".to_owned())
            } else {
                user_data::open_at_launch()
            };
            let shell = SettingsWindow::build(application, writer, stores);
            *slot.borrow_mut() = Some(Rc::clone(&shell));
            shell
        }
    };
    let pane = pane.unwrap_or_else(|| {
        shell
            .writer()
            .borrow()
            .document()
            .choice(&keys::SELECTED_SETTINGS_PANE)
    });
    shell.show(pane);
    shell
}

/// The panes the sidebar lists on Linux, in order: the Mac's roster minus
/// 字型管理 — the framework's panel draws the candidates in its own font,
/// set in Fcitx5 / IBus, and the bundled typefaces install as system fonts
/// (roadmap L4, L7).
pub const SIDEBAR: [SettingsPane; 5] = [
    SettingsPane::General,
    SettingsPane::Appearance,
    SettingsPane::Shortcuts,
    SettingsPane::DictionarySources,
    SettingsPane::CustomDictionary,
];
