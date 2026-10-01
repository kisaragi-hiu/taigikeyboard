//! The shared key-intent executor against the REAL engine and dictionaries,
//! through a surface that records every call — what each intent asks of
//! the document and the list, independent of either shell. The Linux
//! session tests (`taigi-linux-core/tests/session_characterisation.rs`) pin
//! the same paths end to end through that shell's adapter.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use taigi_desktop_core::composing::{
    insert_symbol, pass_through_may_consume, perform_intent, represent_list, CandidateSource,
    ComposingEffectExecutor, ComposingManager, EngineNextWord, IntentSurface, SystemClock,
};
use taigi_desktop_core::dictionary_artifacts::DictionaryArtifacts;
use taigi_desktop_core::engine::{self, Effect};
use taigi_desktop_core::keys::{
    CandidateNavigation, ComposingKeyIntent, KeyEventSnapshot, KeyModifiers,
};
use taigi_desktop_core::settings::{keys, SettingsDocument, StaticSettingsProvider};

/// One lock, one lexicon install: the engine is one per process.
fn engine_lock() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    static INSTALLED: OnceLock<()> = OnceLock::new();
    let guard = LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    INSTALLED.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../assets/dictionaries");
        let artifacts = DictionaryArtifacts::locate(&dir).expect("repo dictionaries present");
        engine::lexicon_install(&artifacts, 1).expect("lexicon installs");
    });
    guard
}

/// Blocks of 100 so one manager's own session bumps never reach the next.
fn fresh_generation() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(900_000);
    NEXT.fetch_add(100, Ordering::Relaxed)
}

/// Every call the executor makes, in order.
#[derive(Debug, Default)]
struct Surface {
    calls: Vec<String>,
    /// Whether this key may swap (the shell's arm AND its document).
    can_swap: bool,
    has_failed: bool,
    selected: Option<usize>,
}

impl ComposingEffectExecutor for Surface {
    fn execute(&mut self, effect: &Effect) {
        if let Effect::CommitTextReplacingPreedit(text) = effect {
            self.calls.push(format!("commit {text}"));
        }
    }
}

impl IntentSurface for Surface {
    fn insert_external(&mut self, text: &str) {
        self.calls.push(format!("insert {text:?}"));
    }

    fn swap_preceding_space(&mut self, replacement: &str) -> bool {
        self.calls.push(format!("swap {replacement:?}"));
        self.can_swap
    }

    fn arm_swap(&mut self) {
        self.calls.push("arm".to_owned());
    }

    fn has_write_failed(&self) -> bool {
        self.has_failed
    }

    fn list_changed(&mut self, list: &mut CandidateSource) {
        self.calls.push(format!("list {}", list.len()));
    }

    fn list_closed(&mut self) {
        self.calls.push("list closed".to_owned());
    }

    fn selected_index(&self) -> Option<usize> {
        self.selected
    }

    fn index_for_key_slot(&self, slot: usize) -> Option<usize> {
        // The literal leads the list and takes no key here: slot 0 is cell 1.
        Some(slot + 1)
    }

    fn navigate(&mut self, direction: CandidateNavigation) {
        self.calls.push(format!("navigate {direction:?}"));
    }
}

struct Rig {
    _engine: MutexGuard<'static, ()>,
    settings: SettingsDocument,
    manager: ComposingManager,
    list: CandidateSource,
    surface: Surface,
}

fn new_rig(is_auto_space_enabled: bool) -> Rig {
    let engine = engine_lock();
    let mut settings = SettingsDocument::default();
    settings.set_bool(&keys::IS_AUTO_SPACE_ENABLED, is_auto_space_enabled);
    let manager = ComposingManager::new(
        Arc::new(StaticSettingsProvider::new(settings.clone())),
        Box::new(EngineNextWord),
        Box::new(SystemClock),
        fresh_generation(),
    );
    Rig {
        _engine: engine,
        settings,
        manager,
        list: CandidateSource::default(),
        surface: Surface::default(),
    }
}

impl Rig {
    fn run(&mut self, intent: ComposingKeyIntent, key: &KeyEventSnapshot) -> bool {
        perform_intent(
            &intent,
            key,
            &self.settings,
            &mut self.manager,
            &mut self.list,
            &mut self.surface,
        )
    }

    fn type_word(&mut self, word: &str) {
        for letter in word.chars() {
            let text = letter.to_string();
            let key = KeyEventSnapshot::text(&text, KeyModifiers::NONE);
            assert!(self.run(ComposingKeyIntent::Input(text), &key));
        }
        self.surface.calls.clear();
    }

    fn calls(&self) -> Vec<&str> {
        self.surface.calls.iter().map(String::as_str).collect()
    }
}

fn no_key() -> KeyEventSnapshot {
    KeyEventSnapshot::default()
}

fn comma() -> KeyEventSnapshot {
    KeyEventSnapshot::text(",", KeyModifiers::NONE)
}

#[test]
fn typing_refetches_the_list_and_hands_it_to_the_surface() {
    let mut rig = new_rig(false);
    let key = KeyEventSnapshot::text("h", KeyModifiers::NONE);
    assert!(rig.run(ComposingKeyIntent::Input("h".to_owned()), &key));
    assert!(!rig.list.is_empty());
    assert_eq!(rig.calls(), [format!("list {}", rig.list.len())]);
}

#[test]
fn commit_writes_the_literal_then_the_auto_space_and_arms_it() {
    let mut rig = new_rig(true);
    rig.type_word("ho");
    assert!(rig.run(ComposingKeyIntent::Commit, &no_key()));
    assert_eq!(
        rig.calls(),
        ["commit ho", "list closed", "insert \" \"", "arm"]
    );
    assert!(rig.list.is_empty());
}

#[test]
fn no_auto_space_when_it_is_off_and_no_arm_after_a_failed_write() {
    let mut rig = new_rig(false);
    rig.type_word("ho");
    rig.run(ComposingKeyIntent::Commit, &no_key());
    assert_eq!(rig.calls(), ["commit ho", "list closed"]);

    drop(rig);
    let mut rig = new_rig(true);
    rig.type_word("ho");
    rig.surface.has_failed = true;
    rig.run(ComposingKeyIntent::Commit, &no_key());
    assert_eq!(
        rig.calls(),
        ["commit ho", "list closed", "insert \" \""],
        "the space is still written; the swap is not armed on it"
    );
}

#[test]
fn cancel_closes_the_list_and_writes_nothing() {
    let mut rig = new_rig(true);
    rig.type_word("ho");
    assert!(rig.run(ComposingKeyIntent::Cancel, &no_key()));
    assert_eq!(rig.calls(), ["list closed"]);
}

#[test]
fn a_slot_key_commits_its_hanji_cell_without_a_space() {
    let mut rig = new_rig(true);
    rig.type_word("ho");
    // The surface's slot 0 is cell 1 because the literal leads the list.
    assert!(rig.list.leads_with_literal_roman());
    let intent = ComposingKeyIntent::SelectCandidateSlot {
        slot: 0,
        flip: false,
    };
    assert!(rig.run(intent, &no_key()));
    assert_eq!(rig.calls(), ["commit 好", "list closed"]);
}

#[test]
fn a_slot_past_the_list_or_no_highlight_is_consumed_and_commits_nothing() {
    let mut rig = new_rig(true);
    rig.type_word("ho");
    let intent = ComposingKeyIntent::SelectCandidateSlot {
        slot: 10_000,
        flip: false,
    };
    assert!(rig.run(intent, &no_key()));
    assert!(rig.calls().is_empty());
    assert!(rig.run(ComposingKeyIntent::CommitHighlightedCandidate, &no_key()));
    assert!(rig.calls().is_empty());
}

#[test]
fn navigate_moves_the_surfaces_highlight_only() {
    let mut rig = new_rig(true);
    rig.type_word("ho");
    assert!(rig.run(
        ComposingKeyIntent::Navigate(CandidateNavigation::Down),
        &no_key()
    ));
    assert_eq!(rig.calls(), ["navigate Down"]);
}

#[test]
fn a_typed_comma_swaps_an_armed_space_or_is_written_full_width() {
    let mut rig = new_rig(true);
    rig.surface.can_swap = true;
    assert!(rig.run(ComposingKeyIntent::PassThrough, &comma()));
    assert_eq!(rig.calls(), ["swap \", \"", "arm"]);

    drop(rig);
    let mut rig = new_rig(true);
    rig.surface.can_swap = false;
    assert!(rig.run(ComposingKeyIntent::PassThrough, &comma()));
    assert_eq!(rig.calls(), ["swap \", \"", "insert \"，\""]);
}

#[test]
fn half_width_punctuation_outside_a_composition_is_the_clients() {
    let mut rig = new_rig(true);
    rig.settings.set_bool(&keys::IS_HANJI_FIRST, false);
    rig.surface.can_swap = false;
    assert!(!rig.run(ComposingKeyIntent::PassThrough, &comma()));
    assert_eq!(rig.calls(), ["swap \", \""]);
}

#[test]
fn a_picked_symbol_swaps_only_when_it_attaches() {
    let mut rig = new_rig(true);
    rig.surface.can_swap = true;
    insert_symbol("·", &rig.settings, &mut rig.manager, &mut rig.surface);
    assert_eq!(
        rig.calls(),
        ["insert \"·\""],
        "not attaching: no swap asked"
    );
    rig.surface.calls.clear();
    insert_symbol("，", &rig.settings, &mut rig.manager, &mut rig.surface);
    assert_eq!(rig.calls(), ["swap \"， \"", "arm"]);
}

#[test]
fn a_pass_through_key_is_consumed_for_punctuation_or_an_armed_swap() {
    let mut settings = SettingsDocument::default();
    settings.set_bool(&keys::IS_AUTO_SPACE_ENABLED, true);
    // Hanji-first: full-width punctuation is written by the input method.
    assert!(pass_through_may_consume(&comma(), &settings, false));
    settings.set_bool(&keys::IS_HANJI_FIRST, false);
    // Roman-first: half width is the client's, unless a swap is armed.
    assert!(!pass_through_may_consume(&comma(), &settings, false));
    assert!(pass_through_may_consume(&comma(), &settings, true));
    settings.set_bool(&keys::IS_AUTO_SPACE_ENABLED, false);
    assert!(!pass_through_may_consume(&comma(), &settings, true));
    // A letter is never punctuation.
    let letter = KeyEventSnapshot::text("a", KeyModifiers::NONE);
    assert!(!pass_through_may_consume(&letter, &settings, true));
    assert!(!pass_through_may_consume(&no_key(), &settings, true));
}

#[test]
fn enter_commits_the_highlighted_cell_and_space_its_other_script() {
    // trace (read by running): cell 1 is 好 hó; Enter writes 好 (Hanji takes
    // no space), Space writes hó, which earns the space and its arm.
    let mut rig = new_rig(true);
    rig.type_word("ho");
    rig.surface.selected = Some(1);
    assert!(rig.run(ComposingKeyIntent::CommitHighlightedCandidate, &no_key()));
    assert_eq!(rig.calls(), ["commit 好", "list closed"]);

    drop(rig);
    let mut rig = new_rig(true);
    rig.type_word("ho");
    rig.surface.selected = Some(1);
    assert!(rig.run(ComposingKeyIntent::CommitAlternateScript, &no_key()));
    assert_eq!(
        rig.calls(),
        ["commit hó", "list closed", "insert \" \"", "arm"]
    );
}

#[test]
fn enter_on_the_literal_writes_it_spaced_and_space_on_it_writes_nothing() {
    // trace: cell 0 is the §34 literal `ho` (no Hanji). Enter = LEAD → the
    // romanization, which earns the space; Space = OTHER → the engine has no
    // other script to write → IGNORED: nothing written, the list refetched.
    let mut rig = new_rig(true);
    rig.type_word("ho");
    rig.surface.selected = Some(0);
    assert!(rig.run(ComposingKeyIntent::CommitHighlightedCandidate, &no_key()));
    assert_eq!(
        rig.calls(),
        ["commit ho", "list closed", "insert \" \"", "arm"]
    );

    drop(rig);
    let mut rig = new_rig(true);
    rig.type_word("ho");
    rig.surface.selected = Some(0);
    let open = rig.list.len();
    assert!(rig.run(ComposingKeyIntent::CommitAlternateScript, &no_key()));
    assert_eq!(rig.calls(), [format!("list {open}")]);
    assert!(rig.manager.is_composing(), "the composition is untouched");
    assert_eq!(rig.manager.raw_input(), "ho");
}

#[test]
fn punctuation_mid_composition_commits_both_in_one_write_and_arms_the_space() {
    // trace (read by running): Hanji-first maps `?` to `？`; the preedit as
    // typed is romanization, so the one write carries the auto space too.
    let mut rig = new_rig(true);
    rig.type_word("ho");
    let key = KeyEventSnapshot::text("?", KeyModifiers::NONE);
    assert!(rig.run(ComposingKeyIntent::CommitThenInsert("?".to_owned()), &key));
    assert_eq!(rig.calls(), ["commit ho？ ", "list closed", "arm"]);
}

#[test]
fn a_key_that_finishes_the_composition_then_belongs_to_the_client() {
    let mut rig = new_rig(true);
    rig.type_word("ho");
    assert!(!rig.run(ComposingKeyIntent::CommitThenPassThrough, &no_key()));
    assert_eq!(rig.calls(), ["commit ho", "list closed"]);
}

#[test]
fn a_switch_re_presents_an_open_list_and_never_opens_one() {
    let mut rig = new_rig(false);
    // No list open: neither kind of switch opens one.
    assert!(!represent_list(
        &rig.settings,
        &mut rig.manager,
        &mut rig.list,
        true
    ));
    assert!(rig.list.is_empty());

    rig.type_word("ho");
    let open = rig.list.len();
    assert!(open > 0);
    assert!(represent_list(
        &rig.settings,
        &mut rig.manager,
        &mut rig.list,
        false
    ));
    assert_eq!(rig.list.len(), open, "re-rendered in place");
    assert!(represent_list(
        &rig.settings,
        &mut rig.manager,
        &mut rig.list,
        true
    ));
    assert_eq!(rig.list.len(), open, "same composition, same candidates");

    // Show Candidate Window switched off since: a refetch fetches nothing.
    rig.settings
        .set_bool(&keys::IS_CANDIDATE_WINDOW_ENABLED, false);
    assert!(!represent_list(
        &rig.settings,
        &mut rig.manager,
        &mut rig.list,
        true
    ));
    assert!(rig.list.is_empty());
}
