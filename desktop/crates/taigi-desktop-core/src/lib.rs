//! Platform-independent core shared by the desktop input methods (Windows
//! TSF, Linux IBus).
//!
//! Everything an OS handle is not needed for lives here so it can be unit
//! tested on any host: the settings model the input method and the settings
//! window share, the protobuf envelope to the shared engine, the composing
//! orchestration ported from macOS, the key classifier, the candidate window
//! geometry and the generated UI strings. The shells (`taigi-windows-tsf`,
//! `taigi-windows-settings`, `taigikeyboard-ibus`, `taigikeyboard-settings`)
//! are thin adapters over it.
//!
//! Behaviour oracle is the macOS input method
//! (`macos/Sources/TaigiInputMethodCore`); ported items cite the Swift they
//! mirror. Design records: `docs/architecture/windows-roadmap.md` (W1–W17,
//! where this crate was born) and `linux-roadmap.md` (L2, the move here).

pub mod candidates;
pub mod composing;
pub mod dictionary_artifacts;
pub mod engine;
pub mod keys;
pub mod policies;
pub mod settings;
pub mod strings;
pub mod symbols;
