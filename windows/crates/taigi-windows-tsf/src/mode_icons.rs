//! The DLL's ICON resource for each mode indicator icon
//! (`taigi_desktop_core::mode_indicator::ModeIndicator::icon_name`, files
//! under `windows/resources/mode/`). One table for both ends: `build.rs`
//! embeds each file under its id, `lang_bar::owned_icon` loads it back by
//! name. Id 1 is the app icon and stays first — the profile's icon index 0
//! (`registration.rs`) is the lowest id.

pub const MODE_ICON_RESOURCES: [(u16, &str); 6] = [
    (2, "taigikeyboard-tl-hanji"),
    (3, "taigikeyboard-tl-romanization"),
    (4, "taigikeyboard-poj-hanji"),
    (5, "taigikeyboard-poj-romanization"),
    (6, "taigikeyboard-tps"),
    (7, "taigikeyboard-english"),
];
