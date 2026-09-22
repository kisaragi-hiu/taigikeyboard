//! One key, end to end: snapshot → classifier → engine, answered as the
//! signals the daemon should get (roadmap L4 / L13). Port of
//! `taigi-windows-tsf::session` (`key_down` + `run_key` + `perform_work`),
//! itself the port of `TaigiInputController.handle(_:client:)`, minus what
//! IBus does for us: the edit session (there is none — `CommitText` is
//! the one write and cannot be refused), the caret rectangle (the panel
//! anchors itself), the handover commit (the daemon's `FocusOut` reached
//! the previous engine first and committed its preedit).
//!
//! Everything here runs under the coordinator lock and emits nothing; the
//! caller replays [`KeyReply::emits`] after the lock is dropped.

use crate::executor::{Emit, LookupTableContent, Recorder};
use crate::runtime::Runtime;
use crate::selection::LookupSelection;
use taigi_desktop_core::composing::{
    CandidateCommitOutcome, CandidateListChange, CandidateSource, ComposingManager,
    ComposingSessionCoordinator, ContextToken, ResolvedCommit,
};
use taigi_desktop_core::keys::{
    CandidateNavigation, CandidateSlotKeySet, ComposingKeyBindings, ComposingKeyIntent,
    KeyEventSnapshot,
};
use taigi_desktop_core::policies;
use taigi_desktop_core::settings::{keys, CandidateLayout, SettingsDocument};

/// `IBUS_CAP_SURROUNDING_TEXT` (ibus `src/ibustypes.h:124`).
pub const CAP_SURROUNDING_TEXT: u32 = 1 << 5;

/// The slot keys a page holds — nine on both sets (`BARE_KEY_ROW`, `1`–`9`).
const PAGE_SIZE: usize = CandidateSlotKeySet::BARE_KEY_ROW.len();

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
    /// Whether the daemon currently shows a lookup table of ours.
    pub is_table_shown: bool,
}

impl Default for EngineState {
    fn default() -> Self {
        Self {
            candidates: CandidateSource::default(),
            selection: LookupSelection::new(0, PAGE_SIZE),
            armed_auto_space: false,
            capabilities: 0,
            is_table_shown: false,
        }
    }
}

impl EngineState {
    pub fn can_delete_surrounding(&self) -> bool {
        self.capabilities & CAP_SURROUNDING_TEXT != 0
    }

    fn clear_list(&mut self) {
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

/// The classifier's answer plus everything the key does — the Windows
/// `key_down` for one context.
pub fn process_key(
    runtime: &Runtime,
    token: ContextToken,
    state: &mut EngineState,
    snapshot: &KeyEventSnapshot,
) -> KeyReply {
    let settings = runtime.settings.current();
    let mut emits = Vec::new();
    // A list the user switched off since the last key comes down HERE,
    // before the key is read (`TaigiInputController.handle`): a Return
    // classified against a list still up would pick a candidate the user
    // asked never to see.
    if !settings.bool(&keys::IS_CANDIDATE_WINDOW_ENABLED) && !state.candidates.is_empty() {
        state.clear_list();
        emits.push(Emit::HideLookupTable);
        state.is_table_shown = false;
    }
    let bindings = ComposingKeyBindings::from_document(&settings);
    let is_composing = runtime
        .coordinator_if_built()
        .and_then(|coordinator| {
            coordinator
                .try_lock()
                .ok()
                .and_then(|guard| guard.manager_ref(token).map(ComposingManager::is_composing))
        })
        .unwrap_or(false);
    let is_showing_candidates = !state.candidates.is_empty();
    let intent =
        ComposingKeyIntent::intent(snapshot, is_composing, is_showing_candidates, &bindings);
    log::debug!(
        "key.intent {intent:?} composing={is_composing} candidates={is_showing_candidates}"
    );
    let would_consume = match &intent {
        ComposingKeyIntent::PassThrough => pass_through_may_consume(snapshot, &settings, state),
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
        KeyWork::Compose(intent),
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
    emits
}

/// A panel navigation (`PageUp` … `CursorDown`): the same intents the keys
/// carry, run without a key.
pub fn navigate_from_panel(
    runtime: &Runtime,
    token: ContextToken,
    state: &mut EngineState,
    direction: CandidateNavigation,
) -> Vec<Emit> {
    if state.candidates.is_empty() {
        return Vec::new();
    }
    let settings = runtime.settings.current();
    let bindings = ComposingKeyBindings::from_document(&settings);
    let mut coordinator = runtime.lock_coordinator();
    run_key(
        &mut coordinator,
        token,
        state,
        &KeyEventSnapshot::default(),
        KeyWork::Compose(ComposingKeyIntent::Navigate(direction)),
        &settings,
        &bindings,
    )
    .emits
}

/// A click on the `position`-th cell of the current page: selects, never
/// commits (`CandidateItemView.swift:47-48`, identical semantics).
pub fn click_from_panel(runtime: &Runtime, state: &mut EngineState, position: usize) -> Vec<Emit> {
    let Some(index) = state.selection.candidate_index_on_page(position) else {
        return Vec::new();
    };
    state.selection.select(index);
    let settings = runtime.settings.current();
    let bindings = ComposingKeyBindings::from_document(&settings);
    vec![Emit::LookupTable(table_content(
        state,
        &settings,
        bindings.slot_key_set(),
    ))]
}

enum KeyWork {
    Compose(ComposingKeyIntent),
}

/// The Windows `run_key` + `perform_work`, against the recorder.
fn run_key(
    coordinator: &mut ComposingSessionCoordinator,
    token: ContextToken,
    state: &mut EngineState,
    snapshot: &KeyEventSnapshot,
    work: KeyWork,
    settings: &SettingsDocument,
    bindings: &ComposingKeyBindings,
) -> KeyReply {
    // Every key gets exactly one chance at the swap: the arm is consumed
    // here, and only the auto-space paths below re-arm it.
    let armed_swap = std::mem::take(&mut state.armed_auto_space);
    let mut recorder = Recorder::new(state.can_delete_surrounding());
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
    let KeyWork::Compose(intent) = work;
    let slot_key_set = bindings.slot_key_set();
    let handled = match &intent {
        ComposingKeyIntent::Input(text) => {
            manager.append(text, &mut recorder);
            refresh_candidates(settings, manager, state);
            true
        }
        ComposingKeyIntent::TelexKey(key) => {
            manager.telex_key(key, &mut recorder);
            refresh_candidates(settings, manager, state);
            true
        }
        ComposingKeyIntent::DeleteBackward => {
            manager.delete_backward(&mut recorder);
            refresh_candidates(settings, manager, state);
            true
        }
        ComposingKeyIntent::Commit => {
            let committed = manager
                .commit_composition(&mut recorder)
                .map(|text| ResolvedCommit {
                    text,
                    wrote_romanization: raw_preedit_wrote_romanization(settings),
                });
            state.clear_list();
            append_auto_space(committed.as_ref(), settings, &mut recorder);
            true
        }
        ComposingKeyIntent::Cancel => {
            manager.cancel_composition(&mut recorder);
            state.clear_list();
            true
        }
        ComposingKeyIntent::CommitThenInsert(text) => {
            let is_width_flip = ComposingKeyIntent::width_flip_character(snapshot).is_some();
            let document_text =
                document_punctuation(settings, text, is_width_flip).unwrap_or_else(|| text.clone());
            let gate = auto_space_gate(settings, raw_preedit_wrote_romanization(settings));
            let insert = policies::augment_insert(&document_text, manager.display_text(), gate);
            let committed = manager.commit_composition_then_insert(&insert.text, &mut recorder);
            state.clear_list();
            if insert.leaves_trailing_auto_space && committed.is_some() {
                recorder.arm_swap();
            }
            true
        }
        ComposingKeyIntent::CommitThenPassThrough => {
            manager.commit_composition(&mut recorder);
            state.clear_list();
            false
        }
        ComposingKeyIntent::PassThrough => {
            match ComposingKeyIntent::document_text(snapshot) {
                None => false,
                Some(typed) => {
                    let is_width_flip =
                        ComposingKeyIntent::width_flip_character(snapshot).is_some();
                    let punctuation = document_punctuation(settings, &typed, is_width_flip);
                    let swapping = if is_width_flip {
                        punctuation.as_deref().unwrap_or(&typed)
                    } else {
                        &typed
                    };
                    if swap_auto_space(swapping, armed_swap, settings, manager, &mut recorder) {
                        true
                    } else if let Some(punctuation) = punctuation {
                        // Punctuation this input method writes itself: the
                        // client cannot map a key it types.
                        recorder.insert_external(&punctuation);
                        manager.note_character_typed_outside_composition(&punctuation);
                        true
                    } else {
                        manager.note_character_typed_outside_composition(&typed);
                        false
                    }
                }
            }
        }
        ComposingKeyIntent::CommitHighlightedCandidate => {
            commit_candidate(
                state.selection.selected_index(),
                false,
                settings,
                manager,
                &mut recorder,
                state,
            );
            true
        }
        // Space: the highlighted cell's OTHER script.
        ComposingKeyIntent::CommitAlternateScript => {
            commit_candidate(
                state.selection.selected_index(),
                true,
                settings,
                manager,
                &mut recorder,
                state,
            );
            true
        }
        ComposingKeyIntent::SelectCandidateSlot { slot, flip } => {
            // A chord aimed at an empty slot is consumed all the same.
            commit_candidate(
                state.selection.candidate_index_for_key_slot(*slot),
                *flip,
                settings,
                manager,
                &mut recorder,
                state,
            );
            true
        }
        ComposingKeyIntent::Navigate(direction) => {
            state.selection.navigate(*direction);
            true
        }
        ComposingKeyIntent::MoveCaret(direction) => {
            // No refetch: the text did not change, so the candidates, the
            // highlight and the page still describe it.
            manager.move_caret(*direction, &mut recorder);
            true
        }
    };
    state.armed_auto_space = recorder.armed_swap;
    let mut emits = recorder.emits;
    present_list(state, settings, slot_key_set, &mut emits);
    KeyReply { handled, emits }
}

/// The lookup table as the list now stands — shown, updated in place, or
/// taken down. Appended after the composing signals so the preedit the
/// list describes is already on screen.
fn present_list(
    state: &mut EngineState,
    settings: &SettingsDocument,
    slot_key_set: CandidateSlotKeySet,
    emits: &mut Vec<Emit>,
) {
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
/// (合用) after a space; labels = the slot keys, one per page position.
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
    let vertical = settings.choice(&keys::CANDIDATE_LAYOUT) != CandidateLayout::Horizontal;
    LookupTableContent {
        candidates,
        labels,
        cursor: state.selection.selected_index().unwrap_or(0) as u32,
        cursor_visible: true,
        page_size: PAGE_SIZE as u32,
        vertical,
    }
}

/// The pass-through keys this input method consumes: attaching
/// punctuation right after an auto space (the swap), full-width
/// punctuation in hanji-first mode, and the width-flip chord in either
/// width. Mirrors the Windows key sink's `pass_through_may_consume`.
fn pass_through_may_consume(
    snapshot: &KeyEventSnapshot,
    settings: &SettingsDocument,
    state: &EngineState,
) -> bool {
    let Some(typed) = ComposingKeyIntent::document_text(snapshot) else {
        return false;
    };
    let is_width_flip = ComposingKeyIntent::width_flip_character(snapshot).is_some();
    if document_punctuation(settings, &typed, is_width_flip).is_some() {
        return true;
    }
    state.armed_auto_space
        && state.can_delete_surrounding()
        && policies::is_attaching_punctuation(&typed)
        && settings.bool(&keys::IS_AUTO_SPACE_ENABLED)
}

/// Re-reads the candidates for the composition as it now stands; with the
/// 候選窗 setting off nothing is fetched, not merely not shown.
fn refresh_candidates(
    settings: &SettingsDocument,
    manager: &mut ComposingManager,
    state: &mut EngineState,
) {
    if !settings.bool(&keys::IS_CANDIDATE_WINDOW_ENABLED) {
        state.clear_list();
        return;
    }
    match manager.fetch_candidates().list_change() {
        CandidateListChange::Replace(candidates) => {
            state.candidates.set(candidates, manager);
            let count = state.candidates.cells().len();
            state.selection = LookupSelection::new(count, PAGE_SIZE);
        }
        CandidateListChange::Clear => state.clear_list(),
    }
}

/// Commits the candidate behind cell `cell_index`, in its own script or
/// (`flip`, Space) the other one.
fn commit_candidate(
    cell_index: Option<usize>,
    flip: bool,
    settings: &SettingsDocument,
    manager: &mut ComposingManager,
    recorder: &mut Recorder,
    state: &mut EngineState,
) {
    let Some((candidate, script)) =
        cell_index.and_then(|index| state.candidates.resolve(index, flip))
    else {
        return;
    };
    let candidate = candidate.clone();
    let (outcome, committed) = manager.commit_candidate(&candidate, script, recorder);
    log::debug!("candidate.commit {outcome:?}");
    match outcome {
        CandidateCommitOutcome::Finalized => {
            state.clear_list();
            append_auto_space(committed.as_ref(), settings, recorder);
        }
        CandidateCommitOutcome::Nailed
        | CandidateCommitOutcome::Ignored
        | CandidateCommitOutcome::Unavailable => {
            refresh_candidates(settings, manager, state);
        }
    }
}

/// The §23 swap for `text` about to be written outside a composition.
fn swap_auto_space(
    text: &str,
    armed_swap: bool,
    settings: &SettingsDocument,
    manager: &mut ComposingManager,
    recorder: &mut Recorder,
) -> bool {
    if !armed_swap
        || !policies::is_attaching_punctuation(text)
        || !settings.bool(&keys::IS_AUTO_SPACE_ENABLED)
        || !recorder.swap_preceding_space(&format!("{text} "))
    {
        return false;
    }
    manager.note_character_typed_outside_composition(text);
    recorder.arm_swap();
    true
}

fn auto_space_gate(settings: &SettingsDocument, wrote_romanization: bool) -> bool {
    policies::is_gate_active(
        settings.bool(&keys::IS_AUTO_SPACE_ENABLED),
        wrote_romanization,
    )
}

fn raw_preedit_wrote_romanization(settings: &SettingsDocument) -> bool {
    policies::raw_preedit_writes_romanization(settings.choice(&keys::INPUT_MODE))
}

/// The trailing auto space after an explicit commit, and the swap armed on
/// it.
fn append_auto_space(
    committed: Option<&ResolvedCommit>,
    settings: &SettingsDocument,
    recorder: &mut Recorder,
) {
    let Some(committed) = committed else { return };
    if !auto_space_gate(settings, committed.wrote_romanization)
        || !policies::should_append_space(&committed.text)
    {
        return;
    }
    recorder.insert_external(" ");
    recorder.arm_swap();
}

fn document_punctuation(
    settings: &SettingsDocument,
    text: &str,
    is_width_flip: bool,
) -> Option<String> {
    policies::document_punctuation(
        text,
        settings.engine_settings().is_full_width_punctuation,
        is_width_flip,
    )
}

/// An executor that records nothing: for a composition the DAEMON already
/// ended — only the engine is reset.
struct NullRecorder;

impl taigi_desktop_core::composing::ComposingEffectExecutor for NullRecorder {
    fn execute(&mut self, _effect: &taigi_desktop_core::engine::Effect) {}
}
