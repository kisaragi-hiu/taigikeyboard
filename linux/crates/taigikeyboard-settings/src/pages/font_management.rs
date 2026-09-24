//! 字型管理 on Linux (roadmap L4): the Mac / Windows pane's shape — a list of
//! typefaces with `+` / `−` under it — over a different job. The framework
//! draws the candidate window, so nothing here is SELECTED: the list is what
//! this install puts in fontconfig's reach (the bundled typefaces the package
//! installed, then the ones the user added), and a selected row is only what
//! `−` acts on — so the bundled rows are not selectable. No installed-families
//! section, search box or pager: the other desktops need them to choose
//! among a few hundred OS families; here there is nothing to choose.

use super::{chosen_path, icon_button, remove_rows, PageContext};
use crate::fonts::{self, ImportOutcome};
use crate::jobs;
use crate::presentation::PageMessage;
use crate::window::{JobSlot, Shell};
use adw::prelude::*;
use gtk::gio;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use taigi_desktop_core::strings::{StringKey, StringResolver};
use taigi_desktop_storage::{stored_file_names, ALLOWED_EXTENSIONS};
use taigi_linux_platform::{InstallLayout, UserDirectories};

pub fn build<'a>(mut context: PageContext<'a>, page: &adw::PreferencesPage) -> PageContext<'a> {
    let fonts = FontManagementPage::new(&context, page);
    // Read whenever the pane comes on screen: the user may have changed the
    // font directory elsewhere since (the Windows pane's `on_enter`).
    let weak = Rc::downgrade(&fonts);
    page.connect_map(move |_| {
        if let Some(fonts) = weak.upgrade() {
            fonts.reload();
        }
    });
    context.retain(fonts);
    context
}

pub struct FontManagementPage {
    shell: Shell,
    strings: StringResolver,
    /// The window's one work slot, shared with the user-data pages: it
    /// outlives a page rebuilt under a running job (a display language
    /// change), so a second import is refused rather than run beside it.
    job_slot: JobSlot,
    /// `None` with no user directory: nothing can be added or removed.
    directories: Option<UserDirectories>,
    layout: InstallLayout,
    list: gtk::ListBox,
    add: gtk::Button,
    remove: gtk::Button,
    /// One entry per row as last drawn: `None` for a bundled typeface,
    /// the stored file name for one the user added.
    rows: RefCell<Vec<Option<String>>>,
}

impl FontManagementPage {
    fn new(context: &PageContext<'_>, page: &adw::PreferencesPage) -> Rc<Self> {
        let strings = *context.strings;
        // No note over the list (USER 2026-09-24 「把說明文字都拿掉,keep page clean」).
        let group = adw::PreferencesGroup::new();
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .css_classes(["boxed-list"])
            .build();
        group.add(&list);
        let verbs = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(6)
            .margin_top(6)
            .build();
        let add = icon_button(
            "list-add-symbolic",
            strings.resolve(StringKey::DesktopCustomFontAdd),
        );
        let remove = icon_button(
            "list-remove-symbolic",
            strings.resolve(StringKey::CommonDelete),
        );
        verbs.append(&add);
        verbs.append(&remove);
        group.add(&verbs);
        page.add(&group);

        let this = Rc::new(Self {
            shell: context.shell.clone(),
            strings,
            job_slot: context.job_slot.clone(),
            directories: UserDirectories::resolve(),
            layout: InstallLayout::shipped(),
            list,
            add,
            remove,
            rows: RefCell::new(Vec::new()),
        });
        this.connect();
        this.reload();
        this
    }

    fn connect(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.list.connect_selected_rows_changed(move |_| {
            if let Some(page) = weak.upgrade() {
                page.render_verbs();
            }
        });
        let weak = Rc::downgrade(self);
        self.add.connect_clicked(move |_| {
            if let Some(page) = weak.upgrade() {
                page.choose_file();
            }
        });
        let weak = Rc::downgrade(self);
        self.remove.connect_clicked(move |_| {
            if let Some(page) = weak.upgrade() {
                page.remove_selected();
            }
        });
    }

    /// Re-reads both sources and redraws the list. Cheap: a directory
    /// listing and four `stat`s, so it runs on the UI thread.
    pub fn reload(&self) {
        remove_rows(&self.list);
        let mut rows = Vec::new();
        for choice in fonts::installed_bundled_fonts(&self.layout) {
            // Listed, never selectable: `−` takes only what the user added.
            self.list
                .append(&font_row(self.strings.resolve(choice.label_key()), false));
            rows.push(None);
        }
        let added = self
            .directories
            .as_ref()
            .map(|directories| stored_file_names(&directories.fonts))
            .unwrap_or_default();
        for file_name in added {
            self.list
                .append(&font_row(displayed_name(&file_name), true));
            rows.push(Some(file_name));
        }
        *self.rows.borrow_mut() = rows;
        self.render_verbs();
    }

    /// The added typeface the selected row is, if any.
    fn selected_file_name(&self) -> Option<String> {
        let index = usize::try_from(self.list.selected_row()?.index()).ok()?;
        self.rows.borrow().get(index)?.clone()
    }

    /// The verbs stay sensitive while a job runs — an insensitive button
    /// would lose the keyboard focus — and the job slot refuses a second one.
    fn render_verbs(&self) {
        let is_writable = self.directories.is_some();
        self.add.set_sensitive(is_writable);
        self.remove
            .set_sensitive(is_writable && self.selected_file_name().is_some());
    }

    fn choose_file(self: &Rc<Self>) {
        if self.job_slot.is_taken() {
            return;
        }
        let filter = gtk::FileFilter::new();
        for extension in ALLOWED_EXTENSIONS {
            filter.add_suffix(extension);
        }
        // Named after all three patterns: unnamed, the chooser labels the
        // filter with the first one alone (`*.ttf`).
        let patterns: Vec<String> = ALLOWED_EXTENSIONS
            .iter()
            .map(|extension| format!("*.{extension}"))
            .collect();
        filter.set_name(Some(&patterns.join(", ")));
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        let dialog = gtk::FileDialog::builder()
            .title(self.strings.resolve(StringKey::DesktopCustomFontAdd))
            .filters(&filters)
            .default_filter(&filter)
            .build();
        let weak = Rc::downgrade(self);
        dialog.open(
            self.shell.window().as_ref(),
            gio::Cancellable::NONE,
            move |result| {
                let Some(page) = weak.upgrade() else { return };
                match chosen_path(result) {
                    Ok(Some(path)) => page.import(path),
                    Ok(None) => {}
                    Err(error) => page.shell.report(
                        &PageMessage::failure(StringKey::CommonImportFailed, error),
                        &page.strings,
                    ),
                }
            },
        );
    }

    fn import(self: &Rc<Self>, source: PathBuf) {
        let Some(directories) = self.directories.clone() else {
            return;
        };
        self.run(
            StringKey::CommonImportFailed,
            move || fonts::import(&source, &directories),
            |outcome| match outcome {
                Ok(ImportOutcome::Added(_)) => None,
                Ok(ImportOutcome::AlreadyInstalled) => Some(PageMessage::Done(
                    StringKey::DesktopCustomFontAlreadyInstalled,
                )),
                Err(error) => Some(PageMessage::failure(StringKey::CommonImportFailed, error)),
            },
        );
    }

    fn remove_selected(self: &Rc<Self>) {
        let (Some(directories), Some(file_name)) =
            (self.directories.clone(), self.selected_file_name())
        else {
            return;
        };
        self.run(
            StringKey::DesktopCustomFontRemoveFailed,
            move || fonts::remove(&file_name, &directories),
            |outcome| {
                outcome.err().map(|error| {
                    PageMessage::failure(StringKey::DesktopCustomFontRemoveFailed, error)
                })
            },
        );
    }

    /// Runs `job` off the UI thread in the window's work slot, or does
    /// nothing because something holds it. The outcome reaches the user as a
    /// toast even when the page was rebuilt meanwhile; `failure` titles a job
    /// that stopped without an answer.
    fn run<T: Send + 'static>(
        self: &Rc<Self>,
        failure: StringKey,
        job: impl FnOnce() -> T + Send + 'static,
        message: impl FnOnce(T) -> Option<PageMessage> + 'static,
    ) {
        let Some(generation) = self.job_slot.take() else {
            return;
        };
        let weak = Rc::downgrade(self);
        let slot = self.job_slot.clone();
        let shell = self.shell.clone();
        let strings = self.strings;
        jobs::spawn(job, move |answer| {
            slot.release(generation);
            let report = match answer {
                Some(value) => message(value),
                None => Some(PageMessage::failure(
                    failure,
                    "the operation did not finish",
                )),
            };
            if let Some(report) = report {
                shell.report(&report, &strings);
            }
            if let Some(page) = weak.upgrade() {
                page.reload();
            }
        });
    }
}

/// One row: plain text (a file name is the user's, never markup).
fn font_row(title: &str, is_selectable: bool) -> adw::ActionRow {
    adw::ActionRow::builder()
        .title(title)
        .use_markup(false)
        .selectable(is_selectable)
        .build()
}

/// A stored file's name without its extension — what the list shows for a
/// typeface the user added (the Windows pane's `displayed_name`).
fn displayed_name(file_name: &str) -> &str {
    file_name
        .rsplit_once('.')
        .map_or(file_name, |(stem, _)| stem)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_added_row_shows_its_file_name_without_the_extension() {
        assert_eq!(displayed_name("jf-openhuninn-2.1.ttf"), "jf-openhuninn-2.1");
        assert_eq!(displayed_name("源樣明體.otf"), "源樣明體");
        assert_eq!(displayed_name("noextension"), "noextension");
    }
}
