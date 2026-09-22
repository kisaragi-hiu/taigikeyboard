//! One `org.freedesktop.IBus.Engine` object per client context the daemon
//! asks the factory for (roadmap L1 / L13). The method roster is the
//! introspection XML in ibus `src/ibusengine.c`; the base
//! `org.freedesktop.IBus.Service` (`Destroy`) is served beside it at the
//! same path (`src/ibusservice.c`).
//!
//! Every method body runs under `catch_unwind` and answers `false` / `()`
//! on a panic: the daemon respawns a dead engine and the user loses the
//! composition, which is worse than one key dropped. Signals are emitted
//! AFTER the key's work, never under the engine lock (`session`).

use crate::executor::{Emit, LookupTableContent};
use crate::runtime::Runtime;
use crate::session::{self, EngineState};
use crate::wire::{self, LookupTable, Orientation};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use taigi_desktop_core::composing::ContextToken;
use taigi_desktop_core::keys::CandidateNavigation;
use taigi_linux_platform::{snapshot, RawKeyEvent};
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{OwnedObjectPath, Value};
use zbus::{interface, Connection};

/// `IBUS_ENGINE_PREEDIT_COMMIT` (ibus `src/ibustypes.h:139`): a focus loss
/// commits what is typed, as the Mac's `commitComposition` does.
const PREEDIT_MODE_COMMIT: u32 = 1;

pub struct Engine {
    runtime: Arc<Runtime>,
    token: ContextToken,
    path: OwnedObjectPath,
    state: EngineState,
}

impl Engine {
    pub fn new(runtime: Arc<Runtime>, token: ContextToken, path: OwnedObjectPath) -> Self {
        Self {
            runtime,
            token,
            path,
            state: EngineState::default(),
        }
    }

    /// Runs `work` against this engine's state under a panic boundary;
    /// answers `fallback` on a panic.
    fn guarded<T>(&mut self, what: &str, fallback: T, work: impl FnOnce(&mut Self) -> T) -> T {
        match catch_unwind(AssertUnwindSafe(|| work(self))) {
            Ok(answer) => answer,
            Err(_) => {
                log::error!(
                    "engine.panic method={what} token={:?} path={}",
                    self.token,
                    self.path
                );
                fallback
            }
        }
    }

    async fn replay(&self, emitter: &SignalEmitter<'_>, emits: Vec<Emit>) {
        for emit in emits {
            let sent = match emit {
                Emit::Preedit { text, caret } => {
                    Self::update_preedit_text(emitter, wire::preedit_text(&text), caret, true).await
                }
                Emit::ClearPreedit => {
                    Self::update_preedit_text(emitter, wire::plain_text(""), 0, false).await
                }
                Emit::Commit(text) => Self::commit_text(emitter, wire::plain_text(&text)).await,
                Emit::DeleteSurrounding { offset, count } => {
                    Self::delete_surrounding_text(emitter, offset, count).await
                }
                Emit::LookupTable(content) => {
                    Self::update_lookup_table(emitter, table_value(&content), true).await
                }
                Emit::HideLookupTable => {
                    Self::update_lookup_table(emitter, empty_table_value(), false).await
                }
            };
            if let Err(error) = sent {
                log::error!("engine.signal_failed token={:?} error={error}", self.token);
            }
        }
    }

    async fn end_session(&mut self, what: &str, emitter: &SignalEmitter<'_>) {
        let emits = self.guarded(what, Vec::new(), |engine| {
            session::end_session(&engine.runtime, engine.token, &mut engine.state)
        });
        self.replay(emitter, emits).await;
    }

    async fn navigate(
        &mut self,
        what: &str,
        emitter: &SignalEmitter<'_>,
        direction: CandidateNavigation,
    ) {
        let emits = self.guarded(what, Vec::new(), |engine| {
            session::navigate_from_panel(
                &engine.runtime,
                engine.token,
                &mut engine.state,
                direction,
            )
        });
        self.replay(emitter, emits).await;
    }
}

fn table_value(content: &LookupTableContent) -> Value<'static> {
    LookupTable {
        page_size: content.page_size,
        cursor_pos: content.cursor,
        cursor_visible: content.cursor_visible,
        round: false,
        orientation: if content.vertical {
            Orientation::Vertical
        } else {
            Orientation::Horizontal
        },
        candidates: &content.candidates,
        labels: &content.labels,
    }
    .to_value()
}

fn empty_table_value() -> Value<'static> {
    LookupTable {
        page_size: 9,
        cursor_pos: 0,
        cursor_visible: false,
        round: false,
        orientation: Orientation::Horizontal,
        candidates: &[],
        labels: &[],
    }
    .to_value()
}

/// `spawn = false`: keys are handled in the order they arrive — a
/// composition is a sequence.
#[interface(name = "org.freedesktop.IBus.Engine", spawn = false)]
impl Engine {
    async fn process_key_event(
        &mut self,
        keyval: u32,
        keycode: u32,
        state: u32,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> bool {
        let Some(snapshot) = snapshot(RawKeyEvent {
            keyval,
            keycode,
            state,
        }) else {
            return false;
        };
        let reply = self.guarded("ProcessKeyEvent", None, |engine| {
            Some(session::process_key(
                &engine.runtime,
                engine.token,
                &mut engine.state,
                &snapshot,
            ))
        });
        let Some(reply) = reply else {
            return false;
        };
        self.replay(&emitter, reply.emits).await;
        reply.handled
    }

    async fn set_cursor_location(&self, _x: i32, _y: i32, _w: i32, _h: i32) {}

    async fn process_hand_writing_event(&self, _coordinates: Vec<f64>) {}

    async fn cancel_hand_writing(&self, _n_strokes: u32) {}

    async fn set_capabilities(&mut self, caps: u32) {
        log::debug!("engine.capabilities token={:?} caps={caps:#x}", self.token);
        self.state.capabilities = caps;
    }

    async fn property_activate(&self, name: String, state: u32) {
        log::debug!("engine.property_activate name={name} state={state}");
    }

    async fn property_show(&self, _name: String) {}

    async fn property_hide(&self, _name: String) {}

    async fn candidate_clicked(
        &mut self,
        index: u32,
        _button: u32,
        _state: u32,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) {
        let emits = self.guarded("CandidateClicked", Vec::new(), |engine| {
            session::click_from_panel(&engine.runtime, &mut engine.state, index as usize)
        });
        self.replay(&emitter, emits).await;
    }

    async fn focus_in(&self) {
        log::debug!("engine.focus_in token={:?}", self.token);
    }

    async fn focus_in_id(&self, _object_path: String, _client: String) {}

    async fn focus_out(&mut self, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) {
        log::debug!("engine.focus_out token={:?}", self.token);
        self.end_session("FocusOut", &emitter).await;
    }

    async fn focus_out_id(
        &mut self,
        _object_path: String,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) {
        self.end_session("FocusOutId", &emitter).await;
    }

    async fn reset(&mut self, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) {
        self.end_session("Reset", &emitter).await;
    }

    async fn enable(&self) {
        log::debug!("engine.enable token={:?}", self.token);
    }

    async fn disable(&mut self, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) {
        self.end_session("Disable", &emitter).await;
    }

    async fn page_up(&mut self, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) {
        self.navigate("PageUp", &emitter, CandidateNavigation::PageUp)
            .await;
    }

    async fn page_down(&mut self, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) {
        self.navigate("PageDown", &emitter, CandidateNavigation::PageDown)
            .await;
    }

    async fn cursor_up(&mut self, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) {
        self.navigate("CursorUp", &emitter, CandidateNavigation::PreviousCandidate)
            .await;
    }

    async fn cursor_down(&mut self, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) {
        self.navigate("CursorDown", &emitter, CandidateNavigation::NextCandidate)
            .await;
    }

    async fn set_surrounding_text(&self, _text: Value<'_>, _cursor_pos: u32, _anchor_pos: u32) {}

    async fn panel_extension_received(&self, _event: Value<'_>) {}

    async fn panel_extension_register_keys(&self, _data: Value<'_>) {}

    /// `(uu)` — the client's input purpose and hints; unused.
    #[zbus(property)]
    async fn set_content_type(&self, _content_type: (u32, u32)) {}

    /// `false`: the daemon uses the plain `FocusIn` / `FocusOut` pair.
    #[zbus(property)]
    async fn focus_id(&self) -> (bool,) {
        (false,)
    }

    /// `false`: nothing in the key table reads the document.
    #[zbus(property)]
    async fn active_surrounding_text(&self) -> (bool,) {
        (false,)
    }

    #[zbus(signal)]
    async fn commit_text(emitter: &SignalEmitter<'_>, text: Value<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn update_auxiliary_text(
        emitter: &SignalEmitter<'_>,
        text: Value<'_>,
        visible: bool,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn update_lookup_table(
        emitter: &SignalEmitter<'_>,
        table: Value<'_>,
        visible: bool,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn register_properties(emitter: &SignalEmitter<'_>, props: Value<'_>)
        -> zbus::Result<()>;

    #[zbus(signal)]
    async fn update_property(emitter: &SignalEmitter<'_>, prop: Value<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn forward_key_event(
        emitter: &SignalEmitter<'_>,
        keyval: u32,
        keycode: u32,
        state: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn delete_surrounding_text(
        emitter: &SignalEmitter<'_>,
        offset: i32,
        n_chars: u32,
    ) -> zbus::Result<()>;
}

/// `UpdatePreeditText(v text, u cursor_pos, b visible, u mode)`
/// (`src/ibusengine.c`): emitted by hand so the mode is always
/// `PREEDIT_MODE_COMMIT` — the one place the preedit's focus-loss
/// behaviour is decided (roadmap L4).
impl Engine {
    async fn update_preedit_text(
        emitter: &SignalEmitter<'_>,
        text: Value<'_>,
        cursor_pos: u32,
        visible: bool,
    ) -> zbus::Result<()> {
        emitter
            .emit(
                "org.freedesktop.IBus.Engine",
                "UpdatePreeditText",
                &(text, cursor_pos, visible, PREEDIT_MODE_COMMIT),
            )
            .await
    }
}

/// `org.freedesktop.IBus.Service`, served at the same path: `Destroy`
/// takes the engine object down.
pub struct Service {
    runtime: Arc<Runtime>,
    token: ContextToken,
    path: OwnedObjectPath,
}

impl Service {
    pub fn new(runtime: Arc<Runtime>, token: ContextToken, path: OwnedObjectPath) -> Self {
        Self {
            runtime,
            token,
            path,
        }
    }
}

#[interface(name = "org.freedesktop.IBus.Service", spawn = false)]
impl Service {
    /// Releases the engine's ownership and unregisters both interfaces.
    /// The unregistration runs on its own thread: an object cannot remove
    /// itself from inside one of its own method calls (the object server
    /// holds it for the call), and `Destroy` wants no reply body anyway.
    async fn destroy(&self, #[zbus(connection)] connection: &Connection) {
        log::info!("engine.destroy token={:?} path={}", self.token, self.path);
        if let Some(coordinator) = self.runtime.coordinator_if_built() {
            let mut coordinator = coordinator
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            coordinator.release(self.token);
        }
        let connection = connection.clone();
        let path = self.path.clone();
        std::thread::Builder::new()
            .name("taigi-engine-destroy".into())
            .spawn(move || {
                zbus::block_on(async move {
                    let server = connection.object_server();
                    if let Err(error) = server.remove::<Engine, _>(&path).await {
                        log::warn!("engine.remove_failed path={path} error={error}");
                    }
                    if let Err(error) = server.remove::<Service, _>(&path).await {
                        log::warn!("service.remove_failed path={path} error={error}");
                    }
                })
            })
            .ok();
    }
}
