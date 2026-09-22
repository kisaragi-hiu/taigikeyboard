//! Every pane mounts — the regression net for the whole window (roadmap
//! L12, the Windows `pane_planning`). Each built page is created against a
//! real window over a temporary settings directory and refreshed once.
//! Needs a display: GTK cannot initialise without one, so the test skips
//! where there is none; CI runs it under `xvfb-run`. Its own `main`
//! (`harness = false`): GTK on macOS initialises only on the main thread.

use adw::prelude::*;
use std::process::ExitCode;
use taigi_desktop_storage::SettingsFileStore;
use taigikeyboard_settings::pages;
use taigikeyboard_settings::window::SettingsWindow;
use taigikeyboard_settings::writer::SettingsWriter;

fn main() -> ExitCode {
    if gtk::init().is_err() {
        eprintln!("panes: skipping, no display for GTK");
        return ExitCode::SUCCESS;
    }
    adw::init().expect("libadwaita initialises after GTK");
    let directory = tempfile::tempdir().expect("a temp directory");
    let application = adw::Application::builder()
        .application_id("tw.taigikeyboard.Settings.Test")
        .build();
    // A window may join an application only after its `startup`; registering
    // the (primary) instance emits it without running the main loop.
    application
        .register(gtk::gio::Cancellable::NONE)
        .expect("the test application registers");
    let writer = SettingsWriter::new(SettingsFileStore::new(directory.path()), false);
    let window = SettingsWindow::build(&application, writer);
    let document = window.writer().borrow().document().clone();
    let strings = window.writer().borrow().strings();
    for pane in pages::BUILT {
        let page = pages::build(pane, &window, &strings, &document);
        assert!(page.widget.first_child().is_some(), "{pane:?} has no rows");
        page.refresh(&document);
        eprintln!("panes: {pane:?} mounted");
    }
    // Read-only over the same directory: the banner tree must build too.
    let read_only = SettingsWindow::build(
        &application,
        SettingsWriter::new(SettingsFileStore::new(directory.path()), true),
    );
    assert!(read_only.writer().borrow().is_read_only());
    eprintln!("panes: ok");
    ExitCode::SUCCESS
}
