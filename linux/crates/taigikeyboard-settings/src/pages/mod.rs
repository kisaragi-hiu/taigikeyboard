//! The pages, one module per pane, and the row shapes they share. A page
//! is an `adw::PreferencesPage` plus the closures that put the document's
//! values back into its rows (`refresh`); a row's own handler writes
//! through the window, and stays quiet while a refresh is setting it.

pub mod about;
pub mod appearance;
pub mod general;

use crate::window::{SettingsWindow, Shell};
use adw::prelude::*;
use std::cell::Cell;
use std::rc::Rc;
use taigi_desktop_core::settings::{SettingChoice, SettingsDocument, SettingsKey, SettingsPane};
use taigi_desktop_core::strings::{StringKey, StringResolver};

/// The panes this crate draws today, listed or not; PR7 / PR8 add theirs.
pub const BUILT: [SettingsPane; 3] = [
    SettingsPane::General,
    SettingsPane::Appearance,
    SettingsPane::About,
];

/// A refresher: one row following the document.
type Refresher = Box<dyn Fn(&SettingsDocument)>;

pub struct Page {
    pub pane: SettingsPane,
    pub widget: adw::PreferencesPage,
    refreshers: Vec<Refresher>,
    /// Set while `refresh` runs, so a row's notify handler does not write
    /// the value it was just given.
    suppress: Rc<Cell<bool>>,
}

impl Page {
    pub fn refresh(&self, document: &SettingsDocument) {
        self.suppress.set(true);
        for refresher in &self.refreshers {
            refresher(document);
        }
        self.suppress.set(false);
    }
}

/// What a page is built from.
pub struct PageContext<'a> {
    pub shell: Shell,
    pub strings: &'a StringResolver,
    pub document: &'a SettingsDocument,
    suppress: Rc<Cell<bool>>,
    refreshers: Vec<Refresher>,
}

impl<'a> PageContext<'a> {
    fn new(
        window: &Rc<SettingsWindow>,
        strings: &'a StringResolver,
        document: &'a SettingsDocument,
    ) -> Self {
        Self {
            shell: Shell(Rc::downgrade(window)),
            strings,
            document,
            suppress: Rc::new(Cell::new(false)),
            refreshers: Vec::new(),
        }
    }

    fn finish(self, pane: SettingsPane, widget: adw::PreferencesPage) -> Page {
        Page {
            pane,
            widget,
            refreshers: self.refreshers,
            suppress: self.suppress,
        }
    }

    /// A switch row bound to a boolean key.
    pub fn switch_row(
        &mut self,
        group: &adw::PreferencesGroup,
        title: StringKey,
        key: SettingsKey<bool>,
    ) {
        let row = adw::SwitchRow::builder()
            .title(self.strings.resolve(title))
            .active(self.document.bool(&key))
            .build();
        let shell = self.shell.clone();
        let suppress = Rc::clone(&self.suppress);
        row.connect_active_notify(move |row| {
            if suppress.get() {
                return;
            }
            let is_on = row.is_active();
            shell.update(|document| document.set_bool(&key, is_on));
        });
        group.add(&row);
        self.refreshers.push(Box::new(move |document| {
            let value = document.bool(&key);
            if row.is_active() != value {
                row.set_active(value);
            }
        }));
    }

    /// A combo row over a `SettingChoice` roster, bound to its key.
    pub fn choice_row<T: SettingChoice>(
        &mut self,
        group: &adw::PreferencesGroup,
        title: StringKey,
        roster: &'static [T],
        key: SettingsKey<T>,
        label: impl Fn(T) -> StringKey,
    ) -> adw::ComboRow {
        let labels: Vec<String> = roster
            .iter()
            .map(|choice| self.strings.resolve(label(*choice)).to_owned())
            .collect();
        let current = self.document.choice(&key);
        self.picker_row(
            group,
            self.strings.resolve(title),
            labels,
            roster,
            current,
            move |choice, document| document.set_choice(&key, choice),
            move |document| document.choice(&key),
        )
    }

    /// A combo row over any roster: `write` stores a pick, `read` answers
    /// the stored value for a refresh.
    #[allow(clippy::too_many_arguments)]
    pub fn picker_row<T: Copy + PartialEq + 'static>(
        &mut self,
        group: &adw::PreferencesGroup,
        title: &str,
        labels: Vec<String>,
        roster: &'static [T],
        current: T,
        write: impl Fn(T, &mut SettingsDocument) + 'static,
        read: impl Fn(&SettingsDocument) -> T + 'static,
    ) -> adw::ComboRow {
        let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        let row = adw::ComboRow::builder()
            .title(title)
            .model(&gtk::StringList::new(&label_refs))
            .selected(roster.iter().position(|c| *c == current).unwrap_or(0) as u32)
            .build();
        let shell = self.shell.clone();
        let suppress = Rc::clone(&self.suppress);
        row.connect_selected_notify(move |row| {
            if suppress.get() {
                return;
            }
            let Some(choice) = roster.get(row.selected() as usize).copied() else {
                return;
            };
            shell.update(|document| write(choice, document));
        });
        group.add(&row);
        let refreshed = row.clone();
        self.refreshers.push(Box::new(move |document| {
            let value = read(document);
            if let Some(index) = roster.iter().position(|c| *c == value) {
                if refreshed.selected() != index as u32 {
                    refreshed.set_selected(index as u32);
                }
            }
        }));
        row
    }

    /// A page's 恢復預設設定 row, its own group at the end, acting on every
    /// row above it. One shape for every pane that has one.
    pub fn reset_row(&mut self, page: &adw::PreferencesPage, reset: fn(&mut SettingsDocument)) {
        let group = adw::PreferencesGroup::new();
        let button = gtk::Button::builder()
            .label(self.strings.resolve(StringKey::SettingsReset))
            .valign(gtk::Align::Center)
            .build();
        let row = adw::ActionRow::builder()
            .title(self.strings.resolve(StringKey::ThemeEditorResetAll))
            .build();
        row.add_suffix(&button);
        row.set_activatable_widget(Some(&button));
        let shell = self.shell.clone();
        button.connect_clicked(move |_| shell.update(reset));
        group.add(&row);
        page.add(&group);
    }

    /// A row that opens a link, marked with the external-link arrow.
    pub fn link_row(&mut self, group: &adw::PreferencesGroup, title: &str, url: &'static str) {
        let row = adw::ActionRow::builder()
            .title(title)
            .activatable(true)
            .build();
        row.add_suffix(&gtk::Image::from_icon_name("adw-external-link-symbolic"));
        let shell = self.shell.clone();
        row.connect_activated(move |_| shell.open_url(url));
        group.add(&row);
    }

    /// Whether a refresh is setting the rows right now (for a handler that
    /// is not one of the shapes above).
    pub fn is_refreshing(&self) -> Rc<Cell<bool>> {
        Rc::clone(&self.suppress)
    }

    /// A refresher for a row the shapes above do not cover.
    pub fn on_refresh(&mut self, refresher: impl Fn(&SettingsDocument) + 'static) {
        self.refreshers.push(Box::new(refresher));
    }
}

/// Builds the page for `pane` against the window.
pub fn build(
    pane: SettingsPane,
    window: &Rc<SettingsWindow>,
    strings: &StringResolver,
    document: &SettingsDocument,
) -> Page {
    let context = PageContext::new(window, strings, document);
    let widget = adw::PreferencesPage::new();
    let context = match pane {
        SettingsPane::Appearance => appearance::build(context, &widget),
        SettingsPane::About => about::build(context, &widget),
        _ => general::build(context, &widget),
    };
    context.finish(pane, widget)
}
