//! The 詞庫來源 pane: which dictionaries the engine draws from, in three
//! groups — 教育部, the others, the supplements — with 教典's eleven
//! subcollections under it in an `adw::ExpanderRow` whose own switch is
//! 教典's (roadmap PR7; port of `DictionaryTogglesView.swift` and the
//! Windows `dictionary_sources.rs`). Every toggle is read live by the
//! engine bridge on the next fetch.

use super::PageContext;
use adw::prelude::*;
use taigi_desktop_core::settings::{keys, SettingsDocument, SettingsKey};
use taigi_desktop_core::strings::StringKey;

/// The 教典 subcollections, in `DictionaryTogglesView`'s order.
const KAUTIAN_SUBCOLLECTIONS: [(SettingsKey<bool>, StringKey); 11] = [
    (
        keys::IS_KAUTIAN_ACCENT_LUKANG_ENABLED,
        StringKey::DictionaryKautianAccentLukang,
    ),
    (
        keys::IS_KAUTIAN_ACCENT_SANSIA_ENABLED,
        StringKey::DictionaryKautianAccentSansia,
    ),
    (
        keys::IS_KAUTIAN_ACCENT_TAIPAK_ENABLED,
        StringKey::DictionaryKautianAccentTaipak,
    ),
    (
        keys::IS_KAUTIAN_ACCENT_GILAN_ENABLED,
        StringKey::DictionaryKautianAccentGilan,
    ),
    (
        keys::IS_KAUTIAN_ACCENT_TAINAN_ENABLED,
        StringKey::DictionaryKautianAccentTainan,
    ),
    (
        keys::IS_KAUTIAN_ACCENT_KAOHSIUNG_ENABLED,
        StringKey::DictionaryKautianAccentKaohsiung,
    ),
    (
        keys::IS_KAUTIAN_ACCENT_KINMEN_ENABLED,
        StringKey::DictionaryKautianAccentKinmen,
    ),
    (
        keys::IS_KAUTIAN_ACCENT_MAKUNG_ENABLED,
        StringKey::DictionaryKautianAccentMakung,
    ),
    (
        keys::IS_KAUTIAN_ACCENT_SINTIK_ENABLED,
        StringKey::DictionaryKautianAccentSintik,
    ),
    (
        keys::IS_KAUTIAN_ACCENT_TAICHUNG_ENABLED,
        StringKey::DictionaryKautianAccentTaichung,
    ),
    (
        keys::IS_KAUTIAN_NAME_APPENDIX_ENABLED,
        StringKey::DictionaryKautianNameAppendix,
    ),
];

const MOE_OTHERS: [(SettingsKey<bool>, StringKey); 3] = [
    (keys::IS_TAIGITV_ENABLED, StringKey::CommonNewwordDict),
    (keys::IS_KUNGGE_ENABLED, StringKey::CommonKunggeDict),
    (keys::IS_STTI_ENABLED, StringKey::CommonSttiDict),
];

const OTHERS: [(SettingsKey<bool>, StringKey); 4] = [
    (keys::IS_ITAIGI_ENABLED, StringKey::CommonITaigiDict),
    (keys::IS_TAIJIT_ENABLED, StringKey::CommonTaiwanJapanDict),
    (keys::IS_TAIHOA_ENABLED, StringKey::CommonTaiHuaDict),
    (keys::IS_SITBUT_ENABLED, StringKey::CommonTaiwanPlantDict),
];

const SUPPLEMENTS: [(SettingsKey<bool>, StringKey); 5] = [
    (
        keys::IS_VARIANT_ENABLED,
        StringKey::DictionaryVariantDictionary,
    ),
    (keys::IS_KHIIN_ENABLED, StringKey::DictionaryKhiin),
    (keys::IS_KHPOO_ENABLED, StringKey::CommonAccentDict),
    (keys::IS_LKK_ENABLED, StringKey::DictionaryLkkDict),
    (keys::IS_DEV_ENABLED, StringKey::DictionaryDevSupplementDict),
];

pub fn build<'a>(mut context: PageContext<'a>, page: &adw::PreferencesPage) -> PageContext<'a> {
    let moe = adw::PreferencesGroup::builder()
        .title(
            context
                .strings
                .resolve(StringKey::DictionaryMoeSectionTitle),
        )
        .build();
    // 教典 and its eleven subcollections in one expander: the expander's own
    // switch is the source's toggle, the rows inside are the 腔口. Off
    // collapses them without clearing them — the choices come back with it
    // (`DictionaryTogglesView.swift` indents the same eleven under the same
    // master toggle).
    let kautian = adw::ExpanderRow::builder()
        .title(context.strings.resolve(StringKey::CommonMoeDict))
        .show_enable_switch(true)
        .enable_expansion(context.document.bool(&keys::IS_KAUTIAN_ENABLED))
        .build();
    let shell = context.shell.clone();
    let suppress = context.refreshing_flag();
    kautian.connect_enable_expansion_notify(move |row| {
        if suppress.get() {
            return;
        }
        let is_on = row.enables_expansion();
        shell.update(|document| document.set_bool(&keys::IS_KAUTIAN_ENABLED, is_on));
    });
    for (key, label) in KAUTIAN_SUBCOLLECTIONS {
        context.switch_row_in(|row| kautian.add_row(row), label, key);
    }
    moe.add(&kautian);
    let refreshed = kautian.clone();
    context.on_refresh(move |document: &SettingsDocument| {
        let is_on = document.bool(&keys::IS_KAUTIAN_ENABLED);
        if refreshed.enables_expansion() != is_on {
            refreshed.set_enable_expansion(is_on);
        }
    });
    for (key, label) in MOE_OTHERS {
        context.switch_row(&moe, label, key);
    }
    page.add(&moe);

    let others = adw::PreferencesGroup::builder()
        .title(
            context
                .strings
                .resolve(StringKey::DictionaryOtherSectionTitle),
        )
        .build();
    for (key, label) in OTHERS {
        context.switch_row(&others, label, key);
    }
    page.add(&others);

    let supplements = adw::PreferencesGroup::builder()
        .title(
            context
                .strings
                .resolve(StringKey::DictionarySupplementSectionTitle),
        )
        .build();
    for (key, label) in SUPPLEMENTS {
        context.switch_row(&supplements, label, key);
    }
    page.add(&supplements);

    context.reset_row(page, SettingsDocument::reset_dictionary_sources);
    context
}
