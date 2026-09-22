//! The 外觀 pane, the Linux row set (roadmap L4): the window's light /
//! dark mode, the candidate list's layout and what each cell shows. The
//! size and typeface rows of the other desktops are the panel's here and
//! are not drawn. Port of `AppearanceSettingsView.swift`.

use super::PageContext;
use adw::prelude::*;
use taigi_desktop_core::settings::{
    keys, AppearanceMode, CandidateDisplayMode, CandidateLayout, SettingChoice, SettingsDocument,
};
use taigi_desktop_core::strings::StringKey;

pub fn build<'a>(mut context: PageContext<'a>, page: &adw::PreferencesPage) -> PageContext<'a> {
    let group = adw::PreferencesGroup::new();
    context.choice_row(
        &group,
        StringKey::DesktopAppearanceTab,
        AppearanceMode::ALL,
        keys::APPEARANCE_MODE,
        AppearanceMode::label_key,
    );
    context.choice_row(
        &group,
        StringKey::DesktopCandidateWindowLayout,
        CandidateLayout::ALL,
        keys::CANDIDATE_LAYOUT,
        CandidateLayout::label_key,
    );
    context.choice_row(
        &group,
        StringKey::SettingsCandidateDisplayMode,
        CandidateDisplayMode::ALL,
        keys::CANDIDATE_DISPLAY_MODE,
        CandidateDisplayMode::label_key,
    );
    page.add(&group);
    context.reset_row(page, SettingsDocument::reset_appearance);
    context
}
