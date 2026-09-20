//! The tray button and its menu — the macOS input-source menu, exactly:
//! the two global shortcuts a click can stand in for / separator / 設定 /
//! separator / 檢查更新 (`TaigiInputController.menu()`; roadmap W6). The button sits in the standard input-mode slot
//! (`GUID_LBI_INPUTMODE`, rakukan `language_bar.rs:21-23`).
//!
//! The menu is DRAWN HERE, from `OnClick`, rather than declared through
//! TSF's `TF_LBI_STYLE_BTN_MENU` / `InitMenu`: the Windows 8+ taskbar input
//! indicator that hosts `GUID_LBI_INPUTMODE` routes a click to
//! `ITfLangBarItemButton::OnClick` and never drives the TSF menu, so a
//! menu-style button there answers a click with nothing at all (observed
//! 2026-08-31 on Windows 11). Both mainstream TIPs do it this way — mozc
//! registers its tray item as a non-menu button and builds a Win32 popup in
//! `OnClick` (`tip_lang_bar.cc:196-240`, `tip_lang_bar_menu.cc:243-330`),
//! and khiin-rs does the same (`lang_bar_indicator.rs:53-58,194-215`).
//! `InitMenu` is the legacy desktop language bar's path, off by default on
//! Windows 11.

use crate::guids::CLSID_TEXT_SERVICE;
use crate::module::instance;
use crate::product_name;
use crate::ui::window;
use crate::wide::{fill_fixed, to_wide_nul};
use taigi_windows_core::keys::{LanguageMode, ShortcutAction};
use taigi_windows_core::settings::SettingsDocument;
use taigi_windows_core::strings::{StringKey, StringResolver};
use windows::core::{Result, PCWSTR};
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetActiveWindow, GetFocus};
use windows::Win32::UI::TextServices::{
    GUID_LBI_INPUTMODE, TF_LANGBARITEMINFO, TF_LBI_STYLE_BTN_BUTTON, TF_LBI_STYLE_SHOWNINTRAY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CopyIcon, CreatePopupMenu, DestroyMenu, LoadIconW, LoadImageW, TrackPopupMenuEx,
    HICON, HMENU, IDI_APPLICATION, IMAGE_ICON, LR_DEFAULTSIZE, MF_SEPARATOR, MF_STRING,
    TPM_NONOTIFY, TPM_RETURNCMD,
};

/// Menu command ids `show_popup` answers with. `TPM_RETURNCMD` spells a
/// dismissed menu as 0, so an id of 0 would read as "the user chose
/// nothing" — the one rule a new row has to keep.
pub const MENU_OPEN_SETTINGS: u32 = 1;
pub const MENU_CHECK_FOR_UPDATES: u32 = 2;
/// The 關於 page's one doorway: it has no sidebar row (USER 2026-09-20).
pub const MENU_ABOUT: u32 = 5;
/// The global shortcuts the menu stands in for, in row order, each with its
/// id (USER 2026-09-19: the menu is where a user looks up the chords they
/// last recorded). One table drives both the drawing and the id → action
/// lookup, so no row can print one action and fire another. Not the 漢羅對調
/// swap — its default is the bare backtick, which the Mac's menu can never
/// print (`TaigiInputController.menu()`) — not the symbol picker, which
/// needs the caret a click has no hold of (`needs_key_context`) — and not the
/// Telex guide (USER 2026-09-20: 「極少人使用」).
pub const MENU_SHORTCUT_ROWS: [(u32, ShortcutAction); 2] = [
    (3, ShortcutAction::ToggleRomanization),
    (4, ShortcutAction::CycleCandidateDisplayMode),
];
const _: () = assert!(MENU_OPEN_SETTINGS != 0 && MENU_CHECK_FOR_UPDATES != 0 && MENU_ABOUT != 0);
const _: () =
    assert!(MENU_ABOUT != MENU_SHORTCUT_ROWS[0].0 && MENU_ABOUT != MENU_SHORTCUT_ROWS[1].0);
const _: () = assert!(MENU_SHORTCUT_ROWS[0].0 != 0 && MENU_SHORTCUT_ROWS[1].0 != 0);

/// The global shortcut a menu id stands for, `None` for the other rows.
pub fn shortcut_for_menu_id(id: u32) -> Option<ShortcutAction> {
    MENU_SHORTCUT_ROWS
        .iter()
        .find(|(row_id, _)| *row_id == id)
        .map(|(_, action)| *action)
}
/// The one cookie `ITfSource::AdviseSink` hands out for the lang-bar sink.
pub const LANG_BAR_SINK_COOKIE: u32 = 0x5461_6967;
/// The DLL icon resource the installer build adds (PR10); index 1.
const ICON_RESOURCE_ID: u16 = 1;
/// What the tray shows when it draws text instead of the icon: the script
/// being typed, in one character, as 微軟注音 spells its own 中/英 state.
/// Not an i18n string — it names the script, so it reads the same in every UI
/// language.
pub fn tray_text(mode: LanguageMode) -> &'static str {
    match mode {
        LanguageMode::Taigi => "台",
        LanguageMode::English => "英",
    }
}

pub fn item_info() -> TF_LANGBARITEMINFO {
    let mut info = TF_LANGBARITEMINFO {
        clsidService: CLSID_TEXT_SERVICE,
        guidItem: GUID_LBI_INPUTMODE,
        // A plain button, NOT `TF_LBI_STYLE_BTN_MENU`: the taskbar input
        // indicator gives a menu-style button's click to nobody (see the
        // module header). The rows come from `OnClick` → `show_popup`.
        dwStyle: TF_LBI_STYLE_BTN_BUTTON | TF_LBI_STYLE_SHOWNINTRAY,
        ulSort: 0,
        szDescription: [0; 32],
    };
    fill_fixed(&mut info.szDescription, &product_name::localized());
    info
}

/// The rows, in order, as (id, label) — `None` is a separator. Pure, so the
/// menu is testable without a live menu. Every shortcut row prints the chord
/// the user last recorded on it, tab-separated: a Win32 menu draws what
/// follows a tab in its accelerator column, which is what `show_popup`
/// builds. The shortcut rows carry the 快捷鍵 pane's own names; the 設定 row
/// keeps its one-word menu name. 檢查更新 and 關於 carry no chord by design.
pub fn menu_rows(
    strings: &StringResolver,
    settings: &SettingsDocument,
) -> Vec<Option<(u32, String)>> {
    let shortcut_label = |action: ShortcutAction, name: StringKey| match action.chord_in(settings) {
        Some(chord) => format!("{}\t{}", strings.resolve(name), chord.display()),
        None => strings.resolve(name).to_owned(),
    };
    let mut rows: Vec<Option<(u32, String)>> = MENU_SHORTCUT_ROWS
        .iter()
        .map(|(id, action)| Some((*id, shortcut_label(*action, action.label_key()))))
        .collect();
    rows.extend([
        None,
        Some((
            MENU_OPEN_SETTINGS,
            shortcut_label(
                ShortcutAction::OpenLastSettingsPane,
                StringKey::CommonSettings,
            ),
        )),
        None,
        Some((
            MENU_CHECK_FOR_UPDATES,
            strings.resolve(StringKey::DesktopUpdateCheckNow).to_owned(),
        )),
        Some((
            MENU_ABOUT,
            strings.resolve(StringKey::DesktopAboutTab).to_owned(),
        )),
    ]);
    rows
}

/// The window a popup is owned by: the focused window of the calling
/// thread, which inside a lang-bar callback is the host's own text window
/// (mozc passes `GetFocus()` the same way), and the thread's active window
/// when nothing holds focus. `TrackPopupMenuEx` refuses a null owner, so a
/// thread with neither gets no menu rather than a failed call.
fn popup_owner() -> Option<HWND> {
    // SAFETY: plain queries about the calling thread's own windows.
    let window = unsafe {
        let focused = GetFocus();
        if focused.is_invalid() {
            GetActiveWindow()
        } else {
            focused
        }
    };
    (!window.is_invalid()).then_some(window)
}

/// `point` with x pulled back inside the monitor's work area, so a menu
/// raised from the right-hand end of the taskbar is not drawn off-screen
/// (mozc `tip_lang_bar_menu.cc:311-322`).
fn clamped_to_work_area(point: POINT) -> POINT {
    let Some(area) = window::monitor_at(point) else {
        return point;
    };
    POINT {
        x: point.x.clamp(area.work_area.left, area.work_area.right),
        y: point.y,
    }
}

/// Draws `rows` as a Win32 popup at `point` and answers the id the user
/// chose, or `None` for a dismissed menu.
///
/// `TPM_RETURNCMD` hands the id back here instead of posting `WM_COMMAND`
/// to a window that has no handler for it, and `TPM_NONOTIFY` keeps the
/// owner from seeing the menu's own messages — a host that reacts to them
/// has been seen to change the menu's state underneath it (mozc's IE 10
/// note, `tip_lang_bar_menu.cc:308-310`). Alignment and button flags are
/// left at their defaults, which are all zero.
pub fn show_popup(rows: &[Option<(u32, String)>], point: POINT) -> Option<u32> {
    let owner = popup_owner()?;
    // SAFETY: a menu this call owns; `OwnedMenu` destroys it on every exit,
    // a panic through the COM guard included.
    let menu = OwnedMenu(unsafe { CreatePopupMenu() }.ok()?);
    for row in rows {
        // SAFETY: the menu is ours and still alive; each text buffer
        // outlives its call.
        let appended = unsafe {
            match row {
                Some((id, label)) => {
                    let text = to_wide_nul(label);
                    AppendMenuW(menu.0, MF_STRING, *id as usize, PCWSTR(text.as_ptr()))
                }
                None => AppendMenuW(menu.0, MF_SEPARATOR, 0, PCWSTR::null()),
            }
        };
        if let Err(error) = appended {
            log::warn!("lang_bar.menu_append_failed error={error}");
            return None;
        }
    }
    let point = clamped_to_work_area(point);
    // SAFETY: the menu is ours and filled; the owner is a live window of
    // the calling thread. This runs a nested modal message loop — nothing
    // of ours is borrowed across it, the rows are already owned values.
    let chosen = unsafe {
        TrackPopupMenuEx(
            menu.0,
            TPM_NONOTIFY.0 | TPM_RETURNCMD.0,
            point.x,
            point.y,
            owner,
            None,
        )
    };
    // `TPM_RETURNCMD` returns the chosen id, or 0 for a menu the user
    // dismissed (and for an error, which is the same nothing to do).
    (chosen.0 > 0).then_some(chosen.0 as u32)
}

/// A popup menu for as long as the call that built it.
struct OwnedMenu(HMENU);

impl Drop for OwnedMenu {
    fn drop(&mut self) {
        // SAFETY: destroying a menu this type owns, exactly once.
        unsafe { DestroyMenu(self.0).ok() };
    }
}

/// A CALLER-OWNED icon — `ITfLangBarItemButton::GetIcon`'s contract is
/// that TSF destroys what it is handed, so nothing shared may be returned:
/// the DLL's own resource loaded without `LR_SHARED`, or — in a build whose
/// resource is missing — a `CopyIcon` of the stock application icon.
pub fn owned_icon() -> Result<HICON> {
    // SAFETY: LoadImageW with a resource id from this DLL's own instance,
    // no LR_SHARED, so the handle is the caller's to destroy.
    let own = unsafe {
        LoadImageW(
            Some(instance()),
            PCWSTR(ICON_RESOURCE_ID as usize as *const u16),
            IMAGE_ICON,
            0,
            0,
            LR_DEFAULTSIZE,
        )
    };
    match own {
        Ok(handle) => Ok(HICON(handle.0)),
        // SAFETY: a stock system icon is shared; the copy is ours to hand over.
        Err(_) => unsafe { CopyIcon(LoadIconW(None, IDI_APPLICATION)?) },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use taigi_windows_core::strings::DisplayLanguage;

    #[test]
    fn the_menu_mirrors_the_macos_input_source_menu() {
        // trace: TaigiInputController.menu() → [shortcuts(2)], [settings], [checkForUpdates, about];
        // the literal oracle is the authored Hanji, as in TaigiInputControllerMenuTests.
        let strings = StringResolver::new(DisplayLanguage::Hanji);
        let rows = menu_rows(&strings, &SettingsDocument::default());
        let labels: Vec<Option<&str>> = rows
            .iter()
            .map(|row| row.as_ref().map(|(_, label)| label.as_str()))
            .collect();
        assert_eq!(
            labels,
            [
                Some("切換台羅/白話字\tCtrl+Alt+C"),
                Some("切換候選詞顯示\tCtrl+Alt+H"),
                None,
                Some("設定\tCtrl+Alt+S"),
                None,
                Some("檢查更新"),
                Some("關於台語齒盤"),
            ]
        );
        let ids: Vec<Option<u32>> = rows
            .iter()
            .map(|row| row.as_ref().map(|(id, _)| *id))
            .collect();
        assert_eq!(
            ids,
            [Some(3), Some(4), None, Some(1), None, Some(2), Some(5)]
        );
        for (id, action) in MENU_SHORTCUT_ROWS {
            assert_eq!(shortcut_for_menu_id(id), Some(action));
        }
        assert_eq!(shortcut_for_menu_id(MENU_OPEN_SETTINGS), None);
        assert_eq!(shortcut_for_menu_id(MENU_CHECK_FOR_UPDATES), None);
        assert_eq!(shortcut_for_menu_id(MENU_ABOUT), None);
        let mut cleared = SettingsDocument::default();
        ShortcutAction::OpenLastSettingsPane.store_in(&mut cleared, None);
        ShortcutAction::ToggleRomanization.store_in(&mut cleared, None);
        let rows = menu_rows(&strings, &cleared);
        assert_eq!(rows[0].as_ref().unwrap().1, "切換台羅/白話字");
        assert_eq!(rows[3].as_ref().unwrap().1, "設定");
        // A plain tray button, whose click reaches `OnClick`. A
        // `TF_LBI_STYLE_BTN_MENU` here shows no menu at all in the
        // Windows 8+ taskbar input indicator (module header).
        assert_eq!(
            item_info().dwStyle,
            TF_LBI_STYLE_BTN_BUTTON | TF_LBI_STYLE_SHOWNINTRAY
        );
    }
}
