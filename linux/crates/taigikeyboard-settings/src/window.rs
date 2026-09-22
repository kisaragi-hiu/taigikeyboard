//! The settings window: an `adw::NavigationSplitView` with the pane list
//! in the sidebar and the selected pane's page in the content, the
//! write-failure banner over it, a 1 s live-reload tick (roadmap L3 / L9).
//! Port of the Windows `winui/window.rs`'s job with GTK's own state model:
//! widgets hold their state, the writer holds the document, and a page is
//! rebuilt only when the display language changes.
//!
//! Named divergences from the Mac, all deliberate: the window frame is
//! not persisted; the 外觀 mode is a combo row; no sidebar icons.

use crate::pages::{self, Page};
use crate::presentation::pane_title;
use crate::writer::{SettingsWriter, REFRESH_INTERVAL};
use crate::SIDEBAR;
use adw::prelude::*;
use gtk::{gio, glib};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use taigi_desktop_core::settings::{
    keys, AppearanceMode, SettingChoice, SettingsDocument, SettingsPane,
};
use taigi_desktop_core::strings::{DisplayLanguage, StringKey};

/// `SettingsPaneLayout` in `SettingsSplitView.swift`: sidebar 215 + detail 545.
const SIDEBAR_WIDTH: f64 = 215.0;
const INITIAL_WIDTH: i32 = 760;
const INITIAL_HEIGHT: i32 = 560;

pub struct SettingsWindow {
    window: adw::ApplicationWindow,
    writer: RefCell<SettingsWriter>,
    sidebar: gtk::ListBox,
    stack: gtk::Stack,
    banner: adw::Banner,
    pages: RefCell<Vec<Page>>,
    /// The language the pages and the sidebar were built for; a document
    /// that names another rebuilds them.
    built_language: Cell<DisplayLanguage>,
    /// `true` while the code selects a sidebar row, so the handler does not
    /// write the selection back.
    is_selecting: Cell<bool>,
    current: Cell<SettingsPane>,
}

impl SettingsWindow {
    pub fn build(application: &adw::Application, writer: SettingsWriter) -> Rc<Self> {
        let sidebar = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .css_classes(["navigation-sidebar"])
            .build();
        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .hexpand(true)
            .vexpand(true)
            .build();
        let banner = adw::Banner::builder().revealed(false).build();
        let content_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content_box.append(&banner);
        content_box.append(&stack);

        let sidebar_view = adw::ToolbarView::new();
        sidebar_view.add_top_bar(&adw::HeaderBar::new());
        sidebar_view.set_content(Some(
            &gtk::ScrolledWindow::builder()
                .child(&sidebar)
                .hscrollbar_policy(gtk::PolicyType::Never)
                .build(),
        ));
        let content_view = adw::ToolbarView::new();
        content_view.add_top_bar(&adw::HeaderBar::new());
        content_view.set_content(Some(&content_box));

        let split = adw::NavigationSplitView::builder()
            .sidebar(&adw::NavigationPage::new(&sidebar_view, ""))
            .content(&adw::NavigationPage::new(&content_view, ""))
            .min_sidebar_width(SIDEBAR_WIDTH)
            .max_sidebar_width(SIDEBAR_WIDTH)
            .build();
        let window = adw::ApplicationWindow::builder()
            .application(application)
            .default_width(INITIAL_WIDTH)
            .default_height(INITIAL_HEIGHT)
            .content(&split)
            .build();

        let language = crate::presentation::display_language_of(writer.document());
        let shell = Rc::new(Self {
            window,
            writer: RefCell::new(writer),
            sidebar,
            stack,
            banner,
            pages: RefCell::new(Vec::new()),
            built_language: Cell::new(language),
            is_selecting: Cell::new(false),
            current: Cell::new(SettingsPane::General),
        });
        shell.build_pages();
        shell.apply_chrome();

        let weak = Rc::downgrade(&shell);
        shell.sidebar.connect_row_selected(move |_, row| {
            let Some(shell) = weak.upgrade() else { return };
            if shell.is_selecting.get() {
                return;
            }
            let Some(row) = row else { return };
            let Some(pane) = SIDEBAR.get(row.index() as usize).copied() else {
                return;
            };
            shell.show_pane(pane);
            shell.update(|document| document.set_choice(&keys::SELECTED_SETTINGS_PANE, pane));
        });
        // The live-reload beat, for as long as the window lives.
        let weak = Rc::downgrade(&shell);
        glib::timeout_add_local(REFRESH_INTERVAL, move || match weak.upgrade() {
            Some(shell) => {
                shell.tick();
                glib::ControlFlow::Continue
            }
            None => glib::ControlFlow::Break,
        });
        shell
    }

    pub fn writer(&self) -> &RefCell<SettingsWriter> {
        &self.writer
    }

    pub fn window(&self) -> &adw::ApplicationWindow {
        &self.window
    }

    /// Opens (or re-activates) the window on `pane`.
    pub fn show(self: &Rc<Self>, pane: SettingsPane) {
        self.show_pane(pane);
        self.window.present();
    }

    /// One settings write from a row, then every row follows the file.
    pub fn update(self: &Rc<Self>, mutate: impl FnOnce(&mut SettingsDocument)) {
        self.writer.borrow_mut().update(mutate);
        self.refresh_pages();
    }

    /// Opens `url` in the browser; a launcher that refused is logged and
    /// said in the banner (`ExternalLinkButton.swift:740-744`).
    pub fn open_url(self: &Rc<Self>, url: &str) {
        let weak = Rc::downgrade(self);
        let url = url.to_owned();
        gtk::UriLauncher::new(&url).launch(
            Some(&self.window),
            gio::Cancellable::NONE,
            move |result| {
                if let Err(error) = result {
                    log::error!("url.open_failed url={url} error={error}");
                    if let Some(shell) = weak.upgrade() {
                        let strings = shell.writer.borrow().strings();
                        shell.banner.set_title(&format!(
                            "{} — {url}",
                            strings.resolve(StringKey::DesktopOpenURLFailed)
                        ));
                        shell.banner.set_revealed(true);
                    }
                }
            },
        );
    }

    fn show_pane(&self, pane: SettingsPane) {
        self.current.set(pane);
        self.stack.set_visible_child_name(pane.raw());
        self.is_selecting.set(true);
        match SIDEBAR.iter().position(|listed| *listed == pane) {
            Some(index) => self
                .sidebar
                .select_row(self.sidebar.row_at_index(index as i32).as_ref()),
            None => self.sidebar.select_row(None::<&gtk::ListBoxRow>),
        }
        self.is_selecting.set(false);
        let strings = self.writer.borrow().strings();
        self.window.set_title(Some(&pane_title(&strings, pane)));
    }

    /// The pages and the sidebar rows, in the built language.
    fn build_pages(self: &Rc<Self>) {
        while let Some(child) = self.stack.first_child() {
            self.stack.remove(&child);
        }
        while let Some(row) = self.sidebar.row_at_index(0) {
            self.sidebar.remove(&row);
        }
        let (strings, document) = {
            let writer = self.writer.borrow();
            (writer.strings(), writer.document().clone())
        };
        for pane in SIDEBAR {
            let row = adw::ActionRow::builder()
                .title(pane_title(&strings, pane))
                .build();
            self.sidebar.append(&row);
        }
        let mut pages = Vec::new();
        for pane in pages::BUILT {
            let page = pages::build(pane, self, &strings, &document);
            self.stack.add_named(&page.widget, Some(pane.raw()));
            pages.push(page);
        }
        *self.pages.borrow_mut() = pages;
        self.show_pane(self.current.get());
    }

    /// Every row follows the document as it is now.
    fn refresh_pages(self: &Rc<Self>) {
        let document = self.writer.borrow().document().clone();
        for page in self.pages.borrow().iter() {
            page.refresh(&document);
        }
        self.apply_chrome();
    }

    /// The banner and the colour scheme, from the document.
    fn apply_chrome(&self) {
        let writer = self.writer.borrow();
        let strings = writer.strings();
        match writer.write_failure() {
            // One banner for both: no user directory reads as the write
            // that would fail, with the missing variable as the detail.
            Some(detail) => {
                self.banner.set_title(&format!(
                    "{} — {detail}",
                    strings.resolve(StringKey::DesktopSettingsWriteFailed)
                ));
                self.banner.set_revealed(true);
            }
            None => self.banner.set_revealed(false),
        }
        // Adwaita follows the system light / dark; the 外觀 row maps onto it
        // (roadmap L3 — identical semantics for the window).
        let scheme = match writer.document().choice(&keys::APPEARANCE_MODE) {
            AppearanceMode::Light => adw::ColorScheme::ForceLight,
            AppearanceMode::Dark => adw::ColorScheme::ForceDark,
            AppearanceMode::Auto => adw::ColorScheme::Default,
        };
        adw::StyleManager::default().set_color_scheme(scheme);
    }

    /// The 1 s beat: the file re-read; a display language that changed
    /// rebuilds every page, any other change refreshes the rows.
    fn tick(self: &Rc<Self>) {
        let changed = self.writer.borrow_mut().refresh();
        if !changed {
            return;
        }
        let language = crate::presentation::display_language_of(self.writer.borrow().document());
        if language != self.built_language.get() {
            self.built_language.set(language);
            self.build_pages();
        }
        self.refresh_pages();
    }
}

/// A row's link back to the window: weak, so a page never keeps the window
/// alive, and silent once the window is gone.
#[derive(Clone)]
pub struct Shell(pub Weak<SettingsWindow>);

impl Shell {
    pub fn update(&self, mutate: impl FnOnce(&mut SettingsDocument)) {
        if let Some(shell) = self.0.upgrade() {
            shell.update(mutate);
        }
    }

    pub fn open_url(&self, url: &str) {
        if let Some(shell) = self.0.upgrade() {
            shell.open_url(url);
        }
    }
}
