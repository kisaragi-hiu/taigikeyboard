//! Generates the Swift bridge for `src/lib.rs` and the prost types for
//! `proto/desktop_shell.proto`. The Swift bridge is staged into `macos/RustEngine/`
//! by `engine/scripts/build-macos-xcframework.sh`, beside the engine's.

use std::io::Result;

/// The engine's protos: `desktop_shell.proto` imports `envelope.proto` for
/// `taigi.engine.ErrorCode`.
const ENGINE_PROTO_DIR: &str = "../../../engine/protos/proto";

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=proto");
    // prost-build emits no rerun directives; the imported engine protos are
    // inputs too.
    println!("cargo:rerun-if-changed={ENGINE_PROTO_DIR}");
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR set by cargo");
    // `RustTaigiDesktop`, not `RustTaigi`: swift-bridge writes each bridge to
    // `<OUT_DIR>/<name>/<name>.{h,swift}`, and the engine's already owns the
    // `RustTaigi` pair. Both land in the one `RustTaigi` Clang module.
    swift_bridge_build::parse_bridges(vec!["src/lib.rs"])
        .write_all_concatenated(&out_dir, "RustTaigiDesktop");

    prost_build::Config::new()
        // The engine types are the `protos` crate's; generate only this file's.
        .extern_path(".taigi.engine", "::protos::engine")
        .compile_protos(&["proto/desktop_shell.proto"], &["proto", ENGINE_PROTO_DIR])
}
