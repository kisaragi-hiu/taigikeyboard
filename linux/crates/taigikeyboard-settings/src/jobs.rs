//! Store work off the UI thread: `gio::spawn_blocking` on GLib's thread
//! pool, the answer delivered back on the main context (the Windows
//! `spawn_background`). A job that panics answers `None`, never a hung
//! page.

use gtk::{gio, glib};

pub fn spawn<T: Send + 'static>(
    job: impl FnOnce() -> T + Send + 'static,
    done: impl FnOnce(Option<T>) + 'static,
) {
    glib::spawn_future_local(async move {
        let outcome = gio::spawn_blocking(job).await;
        match outcome {
            Ok(value) => done(Some(value)),
            Err(_) => {
                log::error!("job.panicked");
                done(None);
            }
        }
    });
}
