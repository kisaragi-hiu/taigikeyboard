//! The window's side of updates (roadmap L10): the manual check — the 一般
//! pane's 檢查更新 button and `--check-now` from the panel menu — run off
//! the UI thread, recorded in `settings.json` the way the other desktops
//! record it, answered in an alert. The decisions are `taigi-desktop-update`'s;
//! Linux only ever offers the download page (a package needs root, and
//! three formats share one release), so there is no install stage.

use crate::jobs;
use crate::window::SettingsWindow;
use adw::prelude::*;
use std::rc::Rc;
use taigi_desktop_core::composing::{Clock, SystemClock};
use taigi_desktop_core::settings::update_schedule;
use taigi_desktop_core::strings::StringKey;
use taigi_desktop_update::{checker, HttpTransport, ManualOutcome, Outcome};

/// Where the Linux manifest is published (the site's `appcast/linux.json`,
/// rendered from the .deb's release data). Compiled into every shipped
/// build; old installs request it forever, so it stays on a domain the
/// project controls.
pub const PUBLISHED_URL: &str = "https://taigikeyboard.tw/appcast/linux.json";

/// The running build's version (`AppVersion.installed`).
pub const INSTALLED_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The 一般 pane's update-row button: 去下載 when an update is known,
/// else 檢查更新 — the row's own two states (`pages::general::update_row`).
pub fn press(shell: &Rc<SettingsWindow>) {
    let pending = checker::pending_update(shell.writer().borrow().document(), INSTALLED_VERSION);
    match pending {
        Some(manifest) => shell.open_url(&manifest.download_page_url),
        None => check_manually(shell),
    }
}

/// The 檢查更新 press (`checkManually`). A press while a check is in
/// flight is dropped: the check already running answers with an alert.
pub fn check_manually(shell: &Rc<SettingsWindow>) {
    if shell.is_checking_updates() {
        return;
    }
    shell.set_checking_updates(true);
    // Stamped before the fetch, as every check is: the daily schedule
    // restarts from a manual answer too.
    let now = SystemClock.now_ms();
    shell.update(|document| update_schedule::stamp_next_check(document, now));
    let weak = Rc::downgrade(shell);
    jobs::spawn(
        || {
            let transport = HttpTransport {
                manifest_url: PUBLISHED_URL,
            };
            checker::check(&transport, INSTALLED_VERSION)
        },
        move |outcome| {
            let Some(shell) = weak.upgrade() else { return };
            shell.set_checking_updates(false);
            deliver(&shell, outcome.unwrap_or(Outcome::Failed));
        },
    );
}

/// What the outcome leaves behind (`deliver`), then the alert.
fn deliver(shell: &Rc<SettingsWindow>, outcome: Outcome) {
    shell.update(|document| checker::record(document, &outcome));
    present(
        shell,
        ManualOutcome {
            outcome,
            installs_in_app: false,
        },
    );
}

const PROCEED_RESPONSE: &str = "proceed";
const CLOSE_RESPONSE: &str = "close";

/// The alert (`UpdateAlertPresenter`): an update offers 去下載 beside
/// 暫時毋免; up to date and a failed check answer with OK alone.
fn present(shell: &Rc<SettingsWindow>, manual: ManualOutcome) {
    let strings = shell.writer().borrow().strings();
    let (title, detail) = manual.alert_text(&strings);
    let dialog = adw::AlertDialog::new(Some(&title), detail.as_deref());
    match manual.proceed_key() {
        Some(proceed) => {
            dialog.add_response(
                CLOSE_RESPONSE,
                strings.resolve(StringKey::DesktopUpdateLaterAction),
            );
            dialog.add_response(PROCEED_RESPONSE, strings.resolve(proceed));
            dialog.set_response_appearance(PROCEED_RESPONSE, adw::ResponseAppearance::Suggested);
            dialog.set_default_response(Some(PROCEED_RESPONSE));
        }
        None => dialog.add_response(CLOSE_RESPONSE, strings.resolve(StringKey::CommonOk)),
    }
    dialog.set_close_response(CLOSE_RESPONSE);
    let weak = Rc::downgrade(shell);
    dialog.connect_response(None, move |_, response| {
        if response != PROCEED_RESPONSE {
            return;
        }
        let (Some(shell), Outcome::UpdateAvailable(manifest)) = (weak.upgrade(), &manual.outcome)
        else {
            return;
        };
        shell.open_url(&manifest.download_page_url);
    });
    dialog.present(Some(shell.window()));
}
