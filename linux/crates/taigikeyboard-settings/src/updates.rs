//! The settings binary's side of updates (roadmap L10): the manual check —
//! the 一般 pane's 檢查更新 button and `--check-now` from the panel menu —
//! answered in an alert, and the automatic daily check — `--check-updates`,
//! spawned by the engine — answered with a desktop notification once per
//! version. Both run off the UI thread and record in `settings.json` the way
//! the other desktops record it. The decisions are `taigi-desktop-update`'s;
//! Linux only ever offers the download page (a package needs root, and
//! three formats share one release), so there is no install stage.

use crate::jobs;
use crate::presentation::strings_for;
use crate::window::SettingsWindow;
use crate::writer::settings_store;
use adw::prelude::*;
use std::rc::Rc;
use taigi_desktop_core::composing::{Clock, SystemClock};
use taigi_desktop_core::settings::{update_schedule, SettingsDocument};
use taigi_desktop_core::strings::StringKey;
use taigi_desktop_update::{
    checker, run_scheduled_check, HttpTransport, ManualOutcome, Outcome, UpdateManifest,
};

/// Where the Linux manifest is published (the site's `appcast/linux.json`,
/// rendered from the .deb's release data). Compiled into every shipped
/// build; old installs request it forever, so it stays on a domain the
/// project controls.
pub const PUBLISHED_URL: &str = "https://taigikeyboard.tw/appcast/linux.json";

/// The application action a desktop notification opens: the window on 一般,
/// where the known update's row is (`notify`). Reached through D-Bus
/// activation when the process that posted it has exited.
pub(crate) const SHOW_UPDATES_ACTION: &str = "show-updates";

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

/// `--check-updates`: the daily check with no window. The application is
/// held until the check has answered (and a notification, if any, is handed
/// to the desktop), then released — the process ends unless a window is
/// open in it.
pub fn check_in_background(application: &adw::Application) {
    let Ok(store) = settings_store() else {
        return;
    };
    let hold = application.hold();
    let application = application.clone();
    jobs::spawn(
        move || {
            let transport = HttpTransport {
                manifest_url: PUBLISHED_URL,
            };
            run_scheduled_check(
                |mutate| store.update(|document| mutate(document)).ok(),
                &transport,
                INSTALLED_VERSION,
                SystemClock.now_ms(),
            )
        },
        move |ran| {
            if let Some(Some(ran)) = ran {
                if let Some(manifest) = &ran.announce {
                    notify(&application, &ran.document, manifest);
                }
            }
            drop(hold);
        },
    );
}

/// The one notice per version (`UpdateAnnouncement.post`): 有新版本 and the
/// version; clicking it opens the window on 一般, where the known update's
/// row offers 去下載.
fn notify(application: &adw::Application, document: &SettingsDocument, manifest: &UpdateManifest) {
    let strings = strings_for(document);
    let notification =
        gio::Notification::new(strings.resolve(StringKey::DesktopUpdateAvailableTitle));
    notification.set_body(Some(&strings.format(
        StringKey::DesktopUpdateAvailableMessage,
        &[&manifest.version],
    )));
    notification.set_default_action(&format!("app.{SHOW_UPDATES_ACTION}"));
    application.send_notification(Some("update-available"), &notification);
}
