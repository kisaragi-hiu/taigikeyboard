//! The whole window mounted over a temporary settings directory — the
//! regression net for the pages (roadmap L12, the Windows `pane_planning`):
//! every built pane is in the stack; a row's switch writes its key; an
//! outside write shows on the next tick without bumping the revision; a
//! display language picked in the window rebuilds the sidebar; an unbuilt
//! `--pane` lands on 一般; the read-only window writes nothing; a reset
//! keeps the display language.
//!
//! Needs a display: GTK cannot initialise without one, so the test skips
//! where there is none — unless `TAIGI_REQUIRE_DISPLAY` is set (CI under
//! xvfb), where a missing display is a failure. Its own `main`
//! (`harness = false`): GTK on macOS initialises only on the main thread.

use adw::prelude::*;
use std::process::ExitCode;
use std::rc::Rc;
use taigi_desktop_core::keys::{KeyModifiers, ShortcutAction};
use taigi_desktop_core::settings::{keys, SettingChoice, SettingsPane};
use taigi_desktop_core::strings::{DisplayLanguage, StringKey, StringResolver};
use taigi_desktop_storage::SettingsFileStore;
use taigikeyboard_settings::pages::BUILT;
use taigikeyboard_settings::recorder::RecorderTarget;
use taigikeyboard_settings::window::SettingsWindow;
use taigikeyboard_settings::writer::SettingsWriter;
use taigikeyboard_settings::SIDEBAR;

fn main() -> ExitCode {
    if gtk::init().is_err() {
        if std::env::var_os("TAIGI_REQUIRE_DISPLAY").is_some() {
            eprintln!("panes: no display for GTK and TAIGI_REQUIRE_DISPLAY is set");
            return ExitCode::FAILURE;
        }
        eprintln!("panes: skipping, no display for GTK");
        return ExitCode::SUCCESS;
    }
    adw::init().expect("libadwaita initialises after GTK");
    let application = adw::Application::builder()
        .application_id("tw.taigikeyboard.Settings.Test")
        .build();
    // A window may join an application only after its `startup`; registering
    // the (primary) instance emits it without running the main loop.
    application
        .register(gtk::gio::Cancellable::NONE)
        .expect("the test application registers");

    let directory = tempfile::tempdir().expect("a temp directory");
    let store = SettingsFileStore::new(directory.path());
    let window = SettingsWindow::build(&application, SettingsWriter::new(store.clone()));

    every_built_pane_is_in_the_stack(&window);
    a_switch_row_writes_its_key(&window);
    an_outside_write_shows_on_the_next_tick(&window, &store);
    a_language_picked_here_rebuilds_the_sidebar(&window);
    an_unbuilt_pane_lands_on_general(&window);
    a_reset_keeps_the_display_language(&window);
    a_recorded_press_binds_the_row(&window);
    the_kautian_expander_switch_writes_its_key(&window);
    the_read_only_window_writes_nothing(&application);
    eprintln!("panes: ok");
    ExitCode::SUCCESS
}

fn every_built_pane_is_in_the_stack(window: &Rc<SettingsWindow>) {
    for pane in BUILT {
        let page = window
            .page_widget(pane)
            .unwrap_or_else(|| panic!("{pane:?} is not in the stack"));
        assert!(
            find_first::<adw::PreferencesGroup>(&page).is_some(),
            "{pane:?} has no group"
        );
        assert_eq!(window.show_pane(pane), pane);
    }
    assert_eq!(window.sidebar_titles().len(), SIDEBAR.len());
    eprintln!("panes: {} panes in the stack", BUILT.len());
}

/// trace: 一般's first switch row is 候選窗 (`IS_CANDIDATE_WINDOW_ENABLED`,
/// default true). Flipping the switch writes `false`; flipping back `true`.
fn a_switch_row_writes_its_key(window: &Rc<SettingsWindow>) {
    let page = window.page_widget(SettingsPane::General).expect("general");
    let row = find_first::<adw::SwitchRow>(&page).expect("a switch row");
    assert!(row.is_active());
    row.set_active(false);
    assert!(!window
        .writer()
        .borrow()
        .document()
        .bool(&keys::IS_CANDIDATE_WINDOW_ENABLED));
    row.set_active(true);
    assert!(window
        .writer()
        .borrow()
        .document()
        .bool(&keys::IS_CANDIDATE_WINDOW_ENABLED));
    eprintln!("panes: switch row round-trips");
}

/// trace: the engine writes the file (revision N+1); the window's tick
/// adopts it, the row follows, and the refresh writes nothing (revision
/// stays N+1).
fn an_outside_write_shows_on_the_next_tick(window: &Rc<SettingsWindow>, store: &SettingsFileStore) {
    store
        .update(|document| document.set_bool(&keys::IS_AUTO_SPACE_ENABLED, true))
        .expect("the outside write");
    let revision = store.load().expect("load").revision;
    window.tick();
    let page = window.page_widget(SettingsPane::General).expect("general");
    let auto_space = find_all::<adw::SwitchRow>(&page)
        .into_iter()
        .find(|row| row.is_active() && row.title() != "")
        .expect("a row turned on");
    assert!(auto_space.is_active());
    assert_eq!(window.writer().borrow().document().revision, revision);
    assert_eq!(store.load().expect("load").revision, revision);
    eprintln!("panes: outside write adopted at revision {revision}");
}

fn a_language_picked_here_rebuilds_the_sidebar(window: &Rc<SettingsWindow>) {
    window.update(|document| {
        document.set_string(&keys::DISPLAY_LANGUAGE, DisplayLanguage::English.tag())
    });
    let english = StringResolver::new(DisplayLanguage::English);
    assert_eq!(
        window.sidebar_titles()[0],
        english.resolve(StringKey::DesktopGeneralTab)
    );
    window.update(|document| {
        document.set_string(&keys::DISPLAY_LANGUAGE, DisplayLanguage::Hanji.tag())
    });
    let hanji = StringResolver::new(DisplayLanguage::Hanji);
    assert_eq!(
        window.sidebar_titles()[0],
        hanji.resolve(StringKey::DesktopGeneralTab)
    );
    eprintln!("panes: language rebuild");
}

fn an_unbuilt_pane_lands_on_general(window: &Rc<SettingsWindow>) {
    assert_eq!(
        window.show_pane(SettingsPane::FontManagement),
        SettingsPane::General
    );
    assert_eq!(window.current_pane(), SettingsPane::General);
    assert_eq!(window.show_pane(SettingsPane::About), SettingsPane::About);
    eprintln!("panes: unbuilt pane routed");
}

/// trace: reset_general puts auto-space back to false and keeps the
/// language (#118).
fn a_reset_keeps_the_display_language(window: &Rc<SettingsWindow>) {
    window.update(|document| {
        document.set_string(&keys::DISPLAY_LANGUAGE, DisplayLanguage::English.tag());
        document.set_bool(&keys::IS_AUTO_SPACE_ENABLED, true);
    });
    window.update(taigi_desktop_core::settings::SettingsDocument::reset_general);
    let writer = window.writer();
    let document = writer.borrow();
    assert!(!document.document().bool(&keys::IS_AUTO_SPACE_ENABLED));
    assert_eq!(
        document.document().string(&keys::DISPLAY_LANGUAGE),
        DisplayLanguage::English.tag()
    );
    drop(document);
    window.update(|document| {
        document.set_string(&keys::DISPLAY_LANGUAGE, DisplayLanguage::System.tag())
    });
    eprintln!("panes: reset keeps the language");
}

/// trace: recording on 顯示 Telex 表 (default Ctrl+Alt+/); a bare `a`
/// (keysym 0x61, no modifiers) is refused and recording continues; then
/// Ctrl+Alt+K (0x6b, CONTROL 1<<2 | MOD1 1<<3) is recorded and stored as
/// the row's chord; the × then clears it to "".
fn a_recorded_press_binds_the_row(window: &Rc<SettingsWindow>) {
    let target = RecorderTarget::Global(ShortcutAction::ShowTelexGuide);
    window.start_recording(target);
    assert_eq!(window.recording().0, Some(target));
    window.record_press(0x61, 38, 0);
    assert_eq!(
        window.recording().0,
        Some(target),
        "a bare letter is refused"
    );
    assert!(window.recording().1.is_some());
    window.record_press(0x6b, 45, (1 << 2) | (1 << 3));
    assert_eq!(window.recording().0, None);
    let chord = ShortcutAction::ShowTelexGuide
        .chord_in(window.writer().borrow().document())
        .expect("the row is bound");
    assert_eq!(chord.key, "k");
    assert_eq!(
        chord.modifiers,
        KeyModifiers::CONTROL.with(KeyModifiers::ALT)
    );
    window.clear_shortcut(target);
    assert!(ShortcutAction::ShowTelexGuide
        .chord_in(window.writer().borrow().document())
        .is_none());
    window.update(taigi_desktop_core::settings::SettingsDocument::reset_global_shortcuts);
    eprintln!("panes: recorder binds and clears");
}

/// trace: the 教典 row is the one `adw::ExpanderRow` on 詞庫來源; its
/// enable switch is `IS_KAUTIAN_ENABLED` (default true).
fn the_kautian_expander_switch_writes_its_key(window: &Rc<SettingsWindow>) {
    let page = window
        .page_widget(SettingsPane::DictionarySources)
        .expect("dictionary sources");
    let kautian = find_first::<adw::ExpanderRow>(&page).expect("the 教典 expander");
    assert!(kautian.enables_expansion());
    kautian.set_enable_expansion(false);
    assert!(!window
        .writer()
        .borrow()
        .document()
        .bool(&keys::IS_KAUTIAN_ENABLED));
    kautian.set_enable_expansion(true);
    assert!(window
        .writer()
        .borrow()
        .document()
        .bool(&keys::IS_KAUTIAN_ENABLED));
    eprintln!("panes: kautian expander round-trips");
}

fn the_read_only_window_writes_nothing(application: &adw::Application) {
    let window = SettingsWindow::build(application, SettingsWriter::read_only("test".to_owned()));
    assert!(window.writer().borrow().is_read_only());
    window.update(|document| document.set_bool(&keys::IS_AUTO_SPACE_ENABLED, true));
    assert!(!window
        .writer()
        .borrow()
        .document()
        .bool(&keys::IS_AUTO_SPACE_ENABLED));
    assert_eq!(window.writer().borrow().write_failure(), Some("test"));
    assert_eq!(
        window
            .writer()
            .borrow()
            .document()
            .choice(&keys::SELECTED_SETTINGS_PANE)
            .raw(),
        SettingsPane::General.raw()
    );
    eprintln!("panes: read-only window writes nothing");
}

/// The first descendant of type `T`, depth first.
fn find_first<T: IsA<gtk::Widget>>(root: &gtk::Widget) -> Option<T> {
    find_all::<T>(root).into_iter().next()
}

fn find_all<T: IsA<gtk::Widget>>(root: &gtk::Widget) -> Vec<T> {
    let mut found = Vec::new();
    let mut child = root.first_child();
    while let Some(widget) = child {
        if let Ok(typed) = widget.clone().downcast::<T>() {
            found.push(typed);
        }
        found.extend(find_all::<T>(&widget));
        child = widget.next_sibling();
    }
    found
}
