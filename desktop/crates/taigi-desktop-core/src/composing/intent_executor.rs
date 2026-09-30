//! What one classified key DOES to a composition — the Windows and Linux
//! input methods run it; it mirrors macOS
//! `TaigiInputController.handle(_:client:)`'s switch: the engine calls per
//! [`ComposingKeyIntent`], the candidate commit, the trailing auto space and
//! its swap (§23), the punctuation this input method writes itself. Each
//! shell keeps its document and its list window behind [`IntentSurface`].

use super::{
    CandidateCommitOutcome, CandidateListChange, CandidateSource, ComposingEffectExecutor,
};
use super::{ComposingManager, ResolvedCommit};
use crate::keys::{CandidateNavigation, ComposingKeyIntent, KeyEventSnapshot};
use crate::policies;
use crate::settings::{keys, SettingsDocument};

/// The document and the list window, as the executor reaches them. It is
/// also the executor the engine's effects are replayed on.
pub trait IntentSurface: ComposingEffectExecutor {
    /// Writes `text` outside any composition (a mapped punctuation, a
    /// picked symbol, the auto space).
    fn insert_external(&mut self, text: &str);

    /// Replaces the space the last commit left with `replacement` (the
    /// swap, `guá ` + `，` → `guá，`). `false` when no swap is armed for
    /// this key, or the document cannot take it.
    fn swap_preceding_space(&mut self, replacement: &str) -> bool;

    /// Arms the swap for the next key, at the caret the last write left.
    fn arm_swap(&mut self);

    /// Whether a write of this key has failed so far (a surface keeps its
    /// first failure). A surface that cannot fail answers `false`.
    fn has_write_failed(&self) -> bool;

    /// The list was refetched: show it, update it in place, or hide it.
    fn list_changed(&mut self, list: &mut CandidateSource);

    /// The list closed with the composition.
    fn list_closed(&mut self);

    /// The highlighted cell, if a list shows one.
    fn selected_index(&self) -> Option<usize>;

    /// The cell the `slot`-th selection key addresses on the page shown.
    fn index_for_key_slot(&self, slot: usize) -> Option<usize>;

    fn navigate(&mut self, direction: CandidateNavigation);
}

/// Runs `intent` for the key `snapshot`. Answers whether the key was
/// consumed; `false` hands it to the client.
pub fn perform_intent(
    intent: &ComposingKeyIntent,
    snapshot: &KeyEventSnapshot,
    settings: &SettingsDocument,
    manager: &mut ComposingManager,
    list: &mut CandidateSource,
    surface: &mut impl IntentSurface,
) -> bool {
    match intent {
        ComposingKeyIntent::Input(text) => {
            manager.append(text, surface);
            refresh(settings, manager, list, surface);
            true
        }
        ComposingKeyIntent::TelexKey(key) => {
            manager.telex_key(key, surface);
            refresh(settings, manager, list, surface);
            true
        }
        ComposingKeyIntent::DeleteBackward => {
            manager.delete_backward(surface);
            refresh(settings, manager, list, surface);
            true
        }
        ComposingKeyIntent::Commit => {
            // The preedit AS TYPED: romanization on a platform shipping TL and
            // POJ only, whichever script the candidate list led with.
            let committed = manager
                .commit_composition(surface)
                .map(|text| ResolvedCommit {
                    text,
                    wrote_romanization: raw_preedit_wrote_romanization(settings),
                });
            close_list(list, surface);
            append_auto_space(committed.as_ref(), settings, surface);
            true
        }
        ComposingKeyIntent::Cancel => {
            manager.cancel_composition(surface);
            close_list(list, surface);
            true
        }
        ComposingKeyIntent::CommitThenInsert(text) => {
            // Mapped before the auto-space augmentation so the full-width
            // character rides the same single mutation as the commit. Both
            // rewrites CAN fire: this path commits the preedit as typed,
            // which is romanization under every mode, while the full-width
            // map still answers to the output MODE — so Hanji-first gets
            // `taigi？ ` (macOS pins the same pair).
            let is_width_flip = ComposingKeyIntent::width_flip_character(snapshot).is_some();
            let document_text =
                document_punctuation(settings, text, is_width_flip).unwrap_or_else(|| text.clone());
            let gate = auto_space_gate(settings, raw_preedit_wrote_romanization(settings));
            let insert = policies::augment_insert(&document_text, manager.display_text(), gate);
            let committed = manager.commit_composition_then_insert(&insert.text, surface);
            close_list(list, surface);
            if insert.leaves_trailing_auto_space && committed.is_some() {
                surface.arm_swap();
            }
            true
        }
        ComposingKeyIntent::CommitThenPassThrough => {
            manager.commit_composition(surface);
            close_list(list, surface);
            false
        }
        ComposingKeyIntent::PassThrough => {
            let Some(typed) = ComposingKeyIntent::document_text(snapshot) else {
                return false;
            };
            // The swap is read before the width for a bare key (the word in
            // front of the caret is romanization, which keeps Latin marks);
            // the width-flip chord named its width, so the swap attaches the
            // glyph the user asked for (`TaigiInputController.swift`).
            let is_width_flip = ComposingKeyIntent::width_flip_character(snapshot).is_some();
            let punctuation = document_punctuation(settings, &typed, is_width_flip);
            let swapping = if is_width_flip {
                punctuation.as_deref().unwrap_or(&typed)
            } else {
                &typed
            };
            if swap_auto_space(swapping, settings, manager, surface) {
                return true;
            }
            // Punctuation this input method writes itself: the client cannot
            // map a key it types.
            if let Some(punctuation) = punctuation {
                surface.insert_external(&punctuation);
                manager.note_character_typed_outside_composition(&punctuation);
                return true;
            }
            manager.note_character_typed_outside_composition(&typed);
            false
        }
        ComposingKeyIntent::CommitHighlightedCandidate => {
            let cell = surface.selected_index();
            commit_candidate(cell, false, settings, manager, list, surface);
            true
        }
        // Space: the highlighted cell's OTHER script.
        ComposingKeyIntent::CommitAlternateScript => {
            let cell = surface.selected_index();
            commit_candidate(cell, true, settings, manager, list, surface);
            true
        }
        ComposingKeyIntent::SelectCandidateSlot { slot, flip } => {
            // A chord aimed at an empty slot is consumed all the same. `flip`
            // is Shift on the key: the same cell in its other script.
            let cell = surface.index_for_key_slot(*slot);
            commit_candidate(cell, *flip, settings, manager, list, surface);
            true
        }
        ComposingKeyIntent::Navigate(direction) => {
            surface.navigate(*direction);
            true
        }
        ComposingKeyIntent::MoveCaret(direction) => {
            // No refetch: the text did not change, so the candidates, the
            // highlight and the page still describe it.
            manager.move_caret(*direction, surface);
            true
        }
    }
}

/// A symbol from the symbol picker, written at the caret as one string (so a
/// bracket pair lands as both halves) and not through the full-width map:
/// what the user picked is what they get. An attaching mark swaps with the
/// auto space a commit left, as a typed one would.
pub fn insert_symbol(
    symbol: &str,
    settings: &SettingsDocument,
    manager: &mut ComposingManager,
    surface: &mut impl IntentSurface,
) {
    if !swap_auto_space(symbol, settings, manager, surface) {
        surface.insert_external(symbol);
        manager.note_character_typed_outside_composition(symbol);
    }
}

/// Whether a key the classifier passes through is one this input method
/// consumes all the same: punctuation it writes itself (full width, or the
/// width-flip chord in either width), or an attaching mark right after an
/// auto space (`is_swap_armed`: the shell's best answer before the key
/// runs — at least the arm; a shell that knows its document cannot take the
/// swap folds that in too).
pub fn pass_through_may_consume(
    snapshot: &KeyEventSnapshot,
    settings: &SettingsDocument,
    is_swap_armed: bool,
) -> bool {
    let Some(typed) = ComposingKeyIntent::document_text(snapshot) else {
        return false;
    };
    let is_width_flip = ComposingKeyIntent::width_flip_character(snapshot).is_some();
    if document_punctuation(settings, &typed, is_width_flip).is_some() {
        return true;
    }
    is_swap_armed
        && policies::is_attaching_punctuation(&typed)
        && settings.bool(&keys::IS_AUTO_SPACE_ENABLED)
}

/// Re-reads the candidates for the composition as it now stands. With the
/// Show Candidate Window setting off nothing is fetched, not merely not
/// shown: a list kept behind no window would hand Space and the slot keys to
/// candidates the user cannot see.
pub fn refresh_list(
    settings: &SettingsDocument,
    manager: &mut ComposingManager,
    list: &mut CandidateSource,
) {
    if !settings.bool(&keys::IS_CANDIDATE_WINDOW_ENABLED) {
        list.clear();
        return;
    }
    match manager.fetch_candidates().list_change() {
        CandidateListChange::Replace(candidates) => list.set(candidates, manager),
        CandidateListChange::Clear => list.clear(),
    }
}

fn refresh(
    settings: &SettingsDocument,
    manager: &mut ComposingManager,
    list: &mut CandidateSource,
    surface: &mut impl IntentSurface,
) {
    refresh_list(settings, manager, list);
    surface.list_changed(list);
}

fn close_list(list: &mut CandidateSource, surface: &mut impl IntentSurface) {
    list.clear();
    surface.list_closed();
}

/// Commits the candidate behind cell `cell`, in the cell's own script or
/// (`flip`) the other one. The script is resolved BEFORE the commit and the
/// same one decides the auto space. No cell (no list, an index past it)
/// commits nothing.
fn commit_candidate(
    cell: Option<usize>,
    flip: bool,
    settings: &SettingsDocument,
    manager: &mut ComposingManager,
    list: &mut CandidateSource,
    surface: &mut impl IntentSurface,
) {
    let Some((candidate, script)) = cell.and_then(|index| list.resolve(index, flip)) else {
        return;
    };
    let (outcome, committed) = manager.commit_candidate(candidate, script, surface);
    log::debug!("candidate.commit {outcome:?}");
    match outcome {
        CandidateCommitOutcome::Finalized => {
            close_list(list, surface);
            append_auto_space(committed.as_ref(), settings, surface);
        }
        CandidateCommitOutcome::Nailed
        | CandidateCommitOutcome::Ignored
        | CandidateCommitOutcome::Unavailable => refresh(settings, manager, list, surface),
    }
}

/// The swap for `text` about to be written outside a composition. The arm's
/// EXISTENCE is the verdict — it is only ever set after a commit that wrote
/// romanization earned its space — so only Auto-Space itself is re-read
/// live here. On success the swap is re-armed (`?!` chains) and the engine
/// hears about the character as the end of a context.
fn swap_auto_space(
    text: &str,
    settings: &SettingsDocument,
    manager: &mut ComposingManager,
    surface: &mut impl IntentSurface,
) -> bool {
    if !policies::is_attaching_punctuation(text)
        || !settings.bool(&keys::IS_AUTO_SPACE_ENABLED)
        || !surface.swap_preceding_space(&format!("{text} "))
    {
        return false;
    }
    manager.note_character_typed_outside_composition(text);
    surface.arm_swap();
    true
}

/// The trailing auto space after an explicit commit, and the swap armed on
/// it — not once a write of this key has failed. Lifecycle commits never
/// come here.
fn append_auto_space(
    committed: Option<&ResolvedCommit>,
    settings: &SettingsDocument,
    surface: &mut impl IntentSurface,
) {
    let Some(committed) = committed else { return };
    if !auto_space_gate(settings, committed.wrote_romanization)
        || !policies::should_append_space(&committed.text)
    {
        return;
    }
    surface.insert_external(" ");
    if !surface.has_write_failed() {
        surface.arm_swap();
    }
}

/// The gate every auto-space site reads: Auto-Space live, and whether the
/// commit wrote romanization (from whatever resolved the string).
fn auto_space_gate(settings: &SettingsDocument, wrote_romanization: bool) -> bool {
    policies::is_gate_active(
        settings.bool(&keys::IS_AUTO_SPACE_ENABLED),
        wrote_romanization,
    )
}

/// Whether committing the preedit AS TYPED writes romanization. One key
/// read, not a whole `engine_settings()` snapshot: this runs per keystroke.
fn raw_preedit_wrote_romanization(settings: &SettingsDocument) -> bool {
    policies::raw_preedit_writes_romanization(settings.choice(&keys::INPUT_MODE))
}

/// `policies::document_punctuation` under the DERIVED width, so roman-only
/// stays half-width and combined follows the stored swap.
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
