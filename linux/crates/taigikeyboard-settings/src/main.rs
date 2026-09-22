//! `taigikeyboard-settings`: the settings window the engine spawns
//! (`taigi-linux-platform::launcher`) and `ibus-setup` opens through the
//! component XML's `<setup>`. A GTK 4 + libadwaita port of the macOS
//! settings window — same panes, same controls, same `settings.json` the
//! engine live-reads (roadmap L3 / L8 / L9).

fn main() -> gtk::glib::ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    taigikeyboard_settings::run()
}
