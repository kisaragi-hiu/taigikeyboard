//! One key, end to end: snapshot → classifier → engine, answered as the
//! signals the daemon should get (roadmap L4 / L13). Port of
//! `taigi-windows-tsf::session` (`key_down` + `run_key` + `perform_work`),
//! itself the port of `TaigiInputController.handle(_:client:)`, minus what
//! IBus does for us: the edit session (there is none — `CommitText` is
//! the one write and cannot be refused), the caret rectangle (the panel
//! anchors itself), the handover commit (the daemon's `FocusOut` reached
//! the previous engine first and committed its preedit).
//!
//! The engine's work runs under the coordinator lock (`run_key`) and emits
//! nothing; the chrome above it (`chrome`) takes the lock only where it
//! needs the engine. The caller replays [`KeyReply::emits`] after every
//! lock is dropped.

use crate::chrome;
use crate::executor::{Emit, KeySurface, LookupTableContent, Recorder};
use crate::runtime::Runtime;
use crate::selection::LookupSelection;
use taigi_desktop_core::composing::{
    pass_through_may_consume, perform_intent, CandidateSource, ComposingManager,
    ComposingSessionCoordinator, ContextToken,
};
use taigi_desktop_core::keys::{
    CandidateNavigation, CandidateSlotKeySet, ComposingKeyBindings, ComposingKeyIntent,
    KeyEventSnapshot, ShortcutAction, SymbolPickerIntent,
};
use taigi_desktop_core::settings::{keys, CandidateLayout, SettingsDocument};
use taigi_linux_platform::key_translation::state::RELEASE;
use taigi_linux_platform::{snapshot, RawKeyEvent, DESKTOP_PLATFORM};

/// `IBUS_CAP_SURROUNDING_TEXT` (ibus `src/ibustypes.h:124`).
pub const CAP_SURROUNDING_TEXT: u32 = 1 << 5;

/// The slot keys a page holds — nine on both sets (`BARE_KEY_ROW`, `1`–`9`).
pub(crate) const PAGE_SIZE: usize = CandidateSlotKeySet::BARE_KEY_ROW.len();

/// The symbol picker while it is up: the symbols in pick order (recents
/// first) and the highlight over them (roadmap L4: a lookup table, not a
/// window of its own).
#[derive(Debug)]
pub struct SymbolPicker {
    pub symbols: Vec<String>,
    pub selection: LookupSelection,
}

/// What one engine object remembers between keys.
#[derive(Debug)]
pub struct EngineState {
    /// The fetched list and its presentation (cells, scripts).
    pub candidates: CandidateSource,
    /// The highlight and page over `candidates`' cells.
    pub selection: LookupSelection,
    /// The auto-space swap promised to the next key.
    pub armed_auto_space: bool,
    /// The client's `SetCapabilities` mask.
    pub capabilities: u32,
    /// The focused field hides what is typed (IBus content purpose
    /// `PASSWORD` / `PIN`, Fcitx5 `CapabilityFlag::Password`): nothing
    /// composes and nothing is learned there — the Windows
    /// `is_password_field` gate.
    pub is_password_field: bool,
    /// Whether the daemon currently shows a lookup table of ours.
    pub is_table_shown: bool,
    /// A toggle chord held down: auto-repeat is invisible on the wire, so
    /// the chord is latched until any other key or a release arrives
    /// (Windows `release_toggle_chord_on_other_key`).
    pub latched_chord: Option<ShortcutAction>,
    /// The Telex guide is up (as a lookup table) — the first key takes it
    /// down (`chrome::perform_global`).
    pub telex_guide_shown: bool,
    /// The symbol picker is up; every key is its first.
    pub symbol_picker: Option<SymbolPicker>,
}

impl Default for EngineState {
    fn default() -> Self {
        Self {
            candidates: CandidateSource::default(),
            selection: LookupSelection::new(0, PAGE_SIZE),
            armed_auto_space: false,
            capabilities: 0,
            is_password_field: false,
            is_table_shown: false,
            latched_chord: None,
            telex_guide_shown: false,
            symbol_picker: None,
        }
    }
}

impl EngineState {
    pub fn can_delete_surrounding(&self) -> bool {
        self.capabilities & CAP_SURROUNDING_TEXT != 0
    }

    pub(crate) fn clear_list(&mut self) {
        self.candidates.clear();
        self.selection = LookupSelection::new(0, PAGE_SIZE);
    }
}

/// What a key did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyReply {
    /// `true` = consumed; `false` = the client processes the key itself.
    pub handled: bool,
    pub emits: Vec<Emit>,
}

/// A key as the framework hands it — keysym, keycode, modifier mask with
/// the release bit. Releases and bare modifier presses are answered
/// unhandled; a release also unlatches a held toggle chord.
pub fn process_raw_key(
    runtime: &Runtime,
    token: ContextToken,
    state: &mut EngineState,
    raw: RawKeyEvent,
) -> KeyReply {
    let reply = answer_raw_key(runtime, token, state, raw);
    #[cfg(feature = "e2e-trace")]
    crate::trace::key(raw, &reply, state);
    reply
}

fn answer_raw_key(
    runtime: &Runtime,
    token: ContextToken,
    state: &mut EngineState,
    raw: RawKeyEvent,
) -> KeyReply {
    let unhandled = KeyReply {
        handled: false,
        emits: Vec::new(),
    };
    if raw.state & RELEASE != 0 {
        state.latched_chord = None;
        return unhandled;
    }
    match snapshot(raw) {
        Some(snapshot) => process_key(runtime, token, state, &snapshot),
        None => unhandled,
    }
}

/// The classifier's answer plus everything the key does — the Windows
/// `key_down` for one context: the global chords and the two overlays
/// first, then the composing contract.
pub fn process_key(
    runtime: &Runtime,
    token: ContextToken,
    state: &mut EngineState,
    snapshot: &KeyEventSnapshot,
) -> KeyReply {
    let settings = runtime.settings.current();
    let mut emits = Vec::new();
    let bindings = ComposingKeyBindings::from_document(&settings, DESKTOP_PLATFORM);
    let global_action = ShortcutAction::matching(snapshot, &settings, DESKTOP_PLATFORM);
    // A held toggle chord repeats on the wire as presses with no release
    // between them; the repeats are consumed without firing again.
    if let Some(latched) = state.latched_chord {
        if global_action == Some(latched) {
            return KeyReply {
                handled: true,
                emits,
            };
        }
        state.latched_chord = None;
    }
    // The Telex guide goes down on the first key after it came up, before
    // that key is read: it is a card to glance at, not a mode
    // (`TaigiInputController.handle`). Not on the global chords, so the
    // toggle chord is not "any key". A plain Escape is swallowed — the user
    // mid-word who checked the table keeps the composition and its list.
    if global_action.is_none() && state.telex_guide_shown {
        state.telex_guide_shown = false;
        present_table(state, &settings, &bindings, &mut emits);
        if snapshot.is_bare_escape() {
            return KeyReply {
                handled: true,
                emits,
            };
        }
    }
    // The global chords, before the classifier — the Carbon hotkey's
    // position on the Mac, and like it independent of whether a
    // composition is running.
    if let Some(action) = global_action {
        if action.fires_once_per_press() {
            state.latched_chord = Some(action);
        }
        emits.append(&mut chrome::perform_global(runtime, token, state, action));
        return KeyReply {
            handled: true,
            emits,
        };
    }
    // A password field gets every key untouched — composing a secret would
    // show it in the preedit and teach it to the stores. The global chords
    // above still run, as on Windows (`run_key` answers ToHost there). A
    // picker left up from before the field changed goes down unpicked, and
    // the auto-space promise is spent: its swap would delete in this field.
    if state.is_password_field {
        state.armed_auto_space = false;
        if state.symbol_picker.take().is_some() {
            present_table(state, &settings, &bindings, &mut emits);
        }
        return KeyReply {
            handled: false,
            emits,
        };
    }
    // With the picker up, every key is the picker's first — read before the
    // composing contract so the slot keys and the arrows reach it rather
    // than a list that is not showing. A key the picker has no use for
    // takes it down and goes on below.
    if state.symbol_picker.is_some() {
        match SymbolPickerIntent::intent(snapshot, &bindings, DESKTOP_PLATFORM) {
            SymbolPickerIntent::Close => {
                state.symbol_picker = None;
                present_table(state, &settings, &bindings, &mut emits);
                return KeyReply {
                    handled: true,
                    emits,
                };
            }
            SymbolPickerIntent::Navigate(direction) => {
                if let Some(picker) = &mut state.symbol_picker {
                    picker
                        .selection
                        .navigate(direction, is_vertical_layout(&settings));
                }
                present_table(state, &settings, &bindings, &mut emits);
                return KeyReply {
                    handled: true,
                    emits,
                };
            }
            SymbolPickerIntent::PickSlot(slot) => {
                // An empty slot on a short last page is consumed all the
                // same: the key is the picker's while it is up.
                let index = state
                    .symbol_picker
                    .as_ref()
                    .and_then(|picker| picker.selection.candidate_index_for_key_slot(slot));
                emits.append(&mut chrome::pick_symbol(
                    runtime, token, state, &settings, &bindings, index,
                ));
                return KeyReply {
                    handled: true,
                    emits,
                };
            }
            SymbolPickerIntent::Confirm => {
                let index = state
                    .symbol_picker
                    .as_ref()
                    .and_then(|picker| picker.selection.selected_index());
                emits.append(&mut chrome::pick_symbol(
                    runtime, token, state, &settings, &bindings, index,
                ));
                return KeyReply {
                    handled: true,
                    emits,
                };
            }
            SymbolPickerIntent::CloseAndPassThrough => {
                state.symbol_picker = None;
                present_table(state, &settings, &bindings, &mut emits);
            }
        }
    }
    // A list the user switched off since the last key comes down HERE,
    // before the key is read (`TaigiInputController.handle`): a Return
    // classified against a list still up would pick a candidate the user
    // asked never to see.
    if !settings.bool(&keys::IS_CANDIDATE_WINDOW_ENABLED) && !state.candidates.is_empty() {
        state.clear_list();
        emits.push(Emit::HideLookupTable);
        state.is_table_shown = false;
    }
    let is_composing = self::is_composing(runtime, token);
    let is_showing_candidates = !state.candidates.is_empty();
    let intent = ComposingKeyIntent::intent(
        snapshot,
        is_composing,
        is_showing_candidates,
        &bindings,
        DESKTOP_PLATFORM,
    );
    log::debug!(
        "key.intent {intent:?} composing={is_composing} candidates={is_showing_candidates}"
    );
    let would_consume = match &intent {
        ComposingKeyIntent::PassThrough => pass_through_may_consume(
            snapshot,
            &settings,
            state.armed_auto_space && state.can_delete_surrounding(),
        ),
        ComposingKeyIntent::CommitThenPassThrough => is_composing,
        _ => true,
    };
    if !would_consume && !matches!(intent, ComposingKeyIntent::CommitThenPassThrough) {
        // Not ours, and nothing to finish — but a character reaching the
        // document outside a composition still ends the next-word context.
        if let Some(typed) = ComposingKeyIntent::document_text(snapshot) {
            if let Some(coordinator) = runtime.coordinator_if_built() {
                if let Ok(mut coordinator) = coordinator.try_lock() {
                    if let Some(manager) = coordinator.manager(token) {
                        manager.note_character_typed_outside_composition(&typed);
                    }
                }
            }
        }
        return KeyReply {
            handled: false,
            emits,
        };
    }

    // From here the key is ours: the runtime comes up now, never for a key
    // merely observed.
    let setup = runtime.prepare_for_first_key();
    if setup.lexicon.is_none() {
        log::warn!("key.no_lexicon");
    }
    let mut coordinator = runtime.lock_coordinator();
    let mut reply = run_key(
        &mut coordinator,
        token,
        state,
        snapshot,
        intent,
        &settings,
        &bindings,
    );
    drop(coordinator);
    emits.append(&mut reply.emits);
    KeyReply {
        handled: reply.handled,
        emits,
    }
}

/// The lifecycle end of a composition: the daemon has just committed (or
/// dropped) the preedit itself — `FocusOut` under `PREEDIT_COMMIT`, `Reset`,
/// `Disable`, `Destroy` — so only the engine is reset and the list taken
/// down. The token's ownership is released so the next context claims a
/// fresh session.
pub fn end_session(runtime: &Runtime, token: ContextToken, state: &mut EngineState) -> Vec<Emit> {
    #[cfg(feature = "e2e-trace")]
    crate::trace::session_end();
    let mut emits = Vec::new();
    if let Some(coordinator) = runtime.coordinator_if_built() {
        let mut coordinator = coordinator
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(manager) = coordinator.manager(token) {
            if manager.is_composing() {
                manager.cancel_composition(&mut NullRecorder);
            }
        }
        coordinator.release(token);
    }
    if state.is_table_shown {
        emits.push(Emit::HideLookupTable);
        state.is_table_shown = false;
    }
    state.clear_list();
    state.armed_auto_space = false;
    state.telex_guide_shown = false;
    state.symbol_picker = None;
    state.latched_chord = None;
    emits
}

/// Whether `token`'s engine is mid-composition, read without bringing the
/// runtime up and without waiting on a held lock.
pub(crate) fn is_composing(runtime: &Runtime, token: ContextToken) -> bool {
    runtime
        .coordinator_if_built()
        .and_then(|coordinator| {
            coordinator
                .try_lock()
                .ok()
                .and_then(|guard| guard.manager_ref(token).map(ComposingManager::is_composing))
        })
        .unwrap_or(false)
}

/// The commit the picker chord runs before it opens: what is highlighted
/// when a list shows, the composition as typed otherwise.
pub(crate) fn commit_for_picker(
    runtime: &Runtime,
    token: ContextToken,
    state: &mut EngineState,
    settings: &SettingsDocument,
    bindings: &ComposingKeyBindings,
) -> KeyReply {
    let intent = if state.candidates.is_empty() {
        ComposingKeyIntent::Commit
    } else {
        ComposingKeyIntent::CommitHighlightedCandidate
    };
    runtime.prepare_for_first_key();
    let mut coordinator = runtime.lock_coordinator();
    run_key(
        &mut coordinator,
        token,
        state,
        &KeyEventSnapshot::default(),
        intent,
        settings,
        bindings,
    )
}

/// A panel navigation (`PageUp` … `CursorDown`): the same intents the keys
/// carry, run without a key.
pub fn navigate_from_panel(
    runtime: &Runtime,
    token: ContextToken,
    state: &mut EngineState,
    direction: CandidateNavigation,
) -> Vec<Emit> {
    let settings = runtime.settings.current();
    let bindings = ComposingKeyBindings::from_document(&settings, DESKTOP_PLATFORM);
    // The guide is one page with no highlight; a panel scroll over it must
    // not move the list it covers.
    if state.telex_guide_shown {
        return Vec::new();
    }
    if let Some(picker) = &mut state.symbol_picker {
        picker
            .selection
            .navigate(direction, is_vertical_layout(&settings));
        let mut emits = Vec::new();
        present_table(state, &settings, &bindings, &mut emits);
        return emits;
    }
    if state.candidates.is_empty() {
        return Vec::new();
    }
    let mut coordinator = runtime.lock_coordinator();
    run_key(
        &mut coordinator,
        token,
        state,
        &KeyEventSnapshot::default(),
        ComposingKeyIntent::Navigate(direction),
        &settings,
        &bindings,
    )
    .emits
}

/// A click on the `position`-th cell of the current page: selects, never
/// commits (`CandidateItemView.swift:47-48`, identical semantics).
pub fn click_from_panel(runtime: &Runtime, state: &mut EngineState, position: usize) -> Vec<Emit> {
    let settings = runtime.settings.current();
    let bindings = ComposingKeyBindings::from_document(&settings, DESKTOP_PLATFORM);
    if state.telex_guide_shown {
        return Vec::new();
    }
    if let Some(picker) = &mut state.symbol_picker {
        let Some(index) = picker.selection.candidate_index_on_page(position) else {
            return Vec::new();
        };
        picker.selection.select(index);
    } else {
        let Some(index) = state.selection.candidate_index_on_page(position) else {
            return Vec::new();
        };
        state.selection.select(index);
    }
    let mut emits = Vec::new();
    present_table(state, &settings, &bindings, &mut emits);
    emits
}

/// The Windows `run_key` + `perform_work`, against the recorder.
fn run_key(
    coordinator: &mut ComposingSessionCoordinator,
    token: ContextToken,
    state: &mut EngineState,
    snapshot: &KeyEventSnapshot,
    intent: ComposingKeyIntent,
    settings: &SettingsDocument,
    bindings: &ComposingKeyBindings,
) -> KeyReply {
    // Every key gets exactly one chance at the swap: the arm is consumed
    // here, and only the auto-space paths re-arm it.
    let is_swap_armed = std::mem::take(&mut state.armed_auto_space);
    // Ownership: a key from this context while another owns the engine
    // takes it over. The other context's composition was already finished
    // by the daemon's `FocusOut` (`end_session`), so `claim` finds nothing
    // to drop in the ordinary case; if it does (a client that skipped
    // `FocusOut`), the outgoing composition is dropped — the text the
    // daemon shows for it stays as it is.
    if coordinator.current_owner() != Some(token) {
        log::info!(
            "key.claim token={token:?} previous={:?}",
            coordinator.current_owner()
        );
    }
    let manager = coordinator.claim(token);
    let mut surface = KeySurface {
        recorder: Recorder::new(state.can_delete_surrounding()),
        is_swap_armed,
        selection: &mut state.selection,
        is_vertical: is_vertical_layout(settings),
    };
    let handled = perform_intent(
        &intent,
        snapshot,
        settings,
        manager,
        &mut state.candidates,
        &mut surface,
    );
    let recorder = surface.recorder;
    state.armed_auto_space = recorder.armed_swap;
    let mut emits = recorder.emits;
    present_table(state, settings, bindings, &mut emits);
    KeyReply { handled, emits }
}

/// The lookup table as things now stand — the symbol picker while it is
/// up, else the candidate list: shown, updated in place, or taken down.
/// Appended after the composing signals so the preedit the list describes
/// is already on screen. The Telex guide is emitted by its toggle alone.
pub(crate) fn present_table(
    state: &mut EngineState,
    settings: &SettingsDocument,
    bindings: &ComposingKeyBindings,
    emits: &mut Vec<Emit>,
) {
    if let Some(picker) = &state.symbol_picker {
        emits.push(Emit::LookupTable(chrome::symbol_picker_table(
            picker, settings, bindings,
        )));
        state.is_table_shown = true;
        return;
    }
    let slot_key_set = bindings.slot_key_set();
    if state.candidates.is_empty() {
        if state.is_table_shown {
            emits.push(Emit::HideLookupTable);
            state.is_table_shown = false;
        }
        return;
    }
    emits.push(Emit::LookupTable(table_content(
        state,
        settings,
        slot_key_set,
    )));
    state.is_table_shown = true;
}

/// The cells as the panel draws them: the leading script, the other script
/// (Combined) after a space; labels = the slot keys, one per page position.
///
/// NAMED DIVERGENCE: the §34 literal cell, which takes no key on macOS and
/// Windows (`lead_cell_is_unkeyed`), takes the first slot key here — the
/// panel labels every position of every page the same way, and a page
/// with a keyless first cell cannot be expressed to it.
fn table_content(
    state: &EngineState,
    settings: &SettingsDocument,
    slot_key_set: CandidateSlotKeySet,
) -> LookupTableContent {
    let candidates = state
        .candidates
        .cells()
        .into_iter()
        .map(|cell| match cell.annotation {
            Some(annotation) => format!("{} {annotation}", cell.text),
            None => cell.text,
        })
        .collect();
    let labels = (0..PAGE_SIZE)
        .map(|slot| slot_key_set.label_for_slot(slot))
        .collect();
    LookupTableContent {
        candidates,
        labels,
        cursor: state.selection.selected_index().unwrap_or(0) as u32,
        cursor_visible: true,
        page_size: PAGE_SIZE as u32,
        vertical: is_vertical_layout(settings),
    }
}

/// Whether the panel draws the list as a column — what it is told to draw
/// and how the arrows read, from the one setting.
pub(crate) fn is_vertical_layout(settings: &SettingsDocument) -> bool {
    settings.choice(&keys::CANDIDATE_LAYOUT) != CandidateLayout::Horizontal
}

/// An executor that records nothing: for a composition the DAEMON already
/// ended — only the engine is reset.
struct NullRecorder;

impl taigi_desktop_core::composing::ComposingEffectExecutor for NullRecorder {
    fn execute(&mut self, _effect: &taigi_desktop_core::engine::Effect) {}
}
