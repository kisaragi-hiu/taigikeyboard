// The DLL's icons (the input-method profile's; one per tray mode,
// `src/mode_icons.rs`) and its VERSIONINFO. Logic shared with the settings exe: `../../build-support/resource.rs`.

#[path = "src/mode_icons.rs"]
mod mode_icons;
#[path = "../../build-support/resource.rs"]
mod resource;

fn main() {
    // The 32-bit build ships beside the 64-bit one under its own name
    // (installer `ServiceDll32`, docs/architecture/windows-release.md).
    let original_filename = match std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("x86") => "TaigiKeyboard32.dll",
        _ => "TaigiKeyboard.dll",
    };
    resource::embed(resource::Resources {
        file_description: "TaigiKeyboard text service",
        original_filename,
        file_type: resource::FileType::Library,
        with_icon: true,
        mode_icons: &mode_icons::MODE_ICON_RESOURCES,
    });
}
