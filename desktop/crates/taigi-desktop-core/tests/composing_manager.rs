//! The composing orchestration against the REAL engine and dictionaries, with
//! in-memory ports and a recording executor — the port of macOS's
//! `ComposingManagerTests` / `ComposingManagerCandidateTests` /
//! `ComposingManagerLearningTests` / `ComposingSessionCoordinatorTests`.
//!
//! Same singleton discipline as `engine_roundtrip.rs`: one lock, one fresh
//! generation block per test.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use taigi_desktop_core::composing::{
    CandidateCommitOutcome, CandidateFetchOutcome, CandidateScript, Clock, ComposingEffectExecutor,
    ComposingManager, ComposingSessionCoordinator, ContextToken, NextWordPort,
};
use taigi_desktop_core::dictionary_artifacts::DictionaryArtifacts;
use taigi_desktop_core::engine::{self, ContinuousCandidate, Effect};
use taigi_desktop_core::keys::CaretDirection;
use taigi_desktop_core::platform::DesktopPlatform;
use taigi_desktop_core::settings::{
    keys, CandidateDisplayMode, EngineSettings, SettingsDocument, SettingsProvider,
};
use taigi_desktop_core::symbols::SymbolTable;

// MARK: - Fixtures

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

/// Blocks of 100 so a manager's own `start_new_session` bumps never reach
/// the next test's block.
fn fresh_generation() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(100_000);
    NEXT.fetch_add(100, Ordering::Relaxed)
}

/// Settings a test can change mid-way; every read sees the latest document.
#[derive(Clone, Default)]
struct MutableSettings(Arc<Mutex<SettingsDocument>>);

impl MutableSettings {
    fn edit(&self, edit: impl FnOnce(&mut SettingsDocument)) {
        edit(&mut self.0.lock().unwrap());
    }
}

impl SettingsProvider for MutableSettings {
    fn current(&self) -> Arc<SettingsDocument> {
        Arc::new(self.0.lock().unwrap().clone())
    }
}

/// One next-word handshake the manager reported. What the engine learns from
/// it — the window, noise, sentence ends, compounds — is the engine's
/// (`engine/nextword/src/decide.rs`); when and what the manager reports is
/// the desktop's.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Handshake {
    Selected {
        text: String,
        roman: String,
        now_ms: i64,
    },
    Nailed {
        text: String,
        roman: String,
    },
    Forgot,
}

/// What the manager handed on: each next-word handshake, and the clock. The
/// picks are the engine's to count (R5) — pinned by the engine's
/// `dispatch/tests/user_data_writes.rs` `engine_resolved_picks_are_counted_by_the_engine`.
#[derive(Default)]
struct Memory {
    handshakes: Mutex<Vec<Handshake>>,
    now_ms: Mutex<i64>,
}

impl Memory {
    /// The reported texts in order, `"∅"` for a forgotten context.
    fn reported(&self) -> Vec<String> {
        self.handshakes
            .lock()
            .unwrap()
            .iter()
            .map(|handshake| match handshake {
                Handshake::Selected { text, .. } | Handshake::Nailed { text, .. } => text.clone(),
                Handshake::Forgot => "∅".to_owned(),
            })
            .collect()
    }
}

#[derive(Clone)]
struct Handle(Arc<Memory>);

impl NextWordPort for Handle {
    fn word_selected(
        &self,
        text: &str,
        roman: &str,
        _preceding: &[protos::engine::CommittedWord],
        now_ms: i64,
        _settings: &EngineSettings,
        _generation: u64,
    ) {
        self.0.handshakes.lock().unwrap().push(Handshake::Selected {
            text: text.to_owned(),
            roman: roman.to_owned(),
            now_ms,
        });
    }

    fn segment_nailed(
        &self,
        text: &str,
        roman: &str,
        _now_ms: i64,
        _settings: &EngineSettings,
        _generation: u64,
    ) {
        self.0.handshakes.lock().unwrap().push(Handshake::Nailed {
            text: text.to_owned(),
            roman: roman.to_owned(),
        });
    }

    fn forget_context(&self, _now_ms: i64, _settings: &EngineSettings, _generation: u64) {
        self.0.handshakes.lock().unwrap().push(Handshake::Forgot);
    }
}

impl Clock for Handle {
    fn now_ms(&self) -> i64 {
        *self.0.now_ms.lock().unwrap()
    }
}

#[derive(Default)]
struct Recorder {
    effects: Vec<Effect>,
}

impl ComposingEffectExecutor for Recorder {
    fn execute(&mut self, effect: &Effect) {
        self.effects.push(effect.clone());
    }
}

impl Recorder {
    fn preedits(&self) -> Vec<&str> {
        self.effects
            .iter()
            .filter_map(|effect| match effect {
                Effect::UpdatePreedit { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }
    fn committed(&self) -> Vec<&str> {
        self.effects
            .iter()
            .filter_map(|effect| match effect {
                Effect::CommitTextReplacingPreedit(text) => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }
    fn cleared(&self) -> bool {
        self.effects.contains(&Effect::ClearPreeditWithoutCommit)
    }
}

struct Rig {
    manager: ComposingManager,
    settings: MutableSettings,
    memory: Arc<Memory>,
    recorder: Recorder,
}

fn rig() -> Rig {
    rig_on(DesktopPlatform::Windows)
}

/// A rig whose requests name `platform` — the ports of macOS's own suites
/// run on `MacOS`, the `platform_id` macOS sends.
fn rig_on(platform: DesktopPlatform) -> Rig {
    let memory = Arc::new(Memory::default());
    *memory.now_ms.lock().unwrap() = 1_000;
    let handle = Handle(Arc::clone(&memory));
    let settings = MutableSettings::default();
    let manager = ComposingManager::new(
        Arc::new(settings.clone()),
        Box::new(handle.clone()),
        Box::new(handle),
        platform,
        fresh_generation(),
    );
    Rig {
        manager,
        settings,
        memory,
        recorder: Recorder::default(),
    }
}

impl Rig {
    fn type_text(&mut self, text: &str) {
        for character in text.chars() {
            self.manager
                .append(&character.to_string(), &mut self.recorder);
        }
    }

    fn candidates(&mut self) -> Vec<ContinuousCandidate> {
        match self.manager.fetch_candidates() {
            CandidateFetchOutcome::Found(candidates) => candidates,
            other => panic!("expected candidates, got {other:?}"),
        }
    }

    fn candidate(&mut self, hanji: &str) -> ContinuousCandidate {
        let candidates = self.candidates();
        candidates
            .iter()
            .find(|candidate| candidate.hanji.as_deref() == Some(hanji))
            .cloned()
            .unwrap_or_else(|| {
                panic!(
                    "{hanji} among {:?}",
                    candidates
                        .iter()
                        .map(|c| &c.display_text)
                        .collect::<Vec<_>>()
                )
            })
    }

    /// The engine's outcome, and the text this commit wrote to the document.
    fn commit(
        &mut self,
        candidate: &ContinuousCandidate,
        script: CandidateScript,
    ) -> (CandidateCommitOutcome, Option<String>) {
        let before = self.recorder.effects.len();
        let outcome = self
            .manager
            .commit_candidate(candidate, script, &mut self.recorder);
        let committed =
            self.recorder.effects[before..]
                .iter()
                .rev()
                .find_map(|effect| match effect {
                    Effect::CommitTextReplacingPreedit(text) => Some(text.clone()),
                    _ => None,
                });
        (outcome, committed)
    }

    /// The engine's auto-space verdict on a commit that must finalize.
    fn commit_verdict(&mut self, candidate: &ContinuousCandidate, script: CandidateScript) -> bool {
        match self.commit(candidate, script).0 {
            CandidateCommitOutcome::Finalized { earns_auto_space } => earns_auto_space,
            other => panic!("expected a final commit, got {other:?}"),
        }
    }

    fn advance_clock(&self, ms: i64) {
        *self.memory.now_ms.lock().unwrap() += ms;
    }
}

// MARK: - ComposingManagerTests

#[test]
fn append_shows_the_preedit_and_mirrors_the_engine() {
    let _lock = engine_lock();
    let mut rig = rig();
    rig.type_text("tai5");
    assert!(rig.manager.is_composing());
    assert_eq!(rig.manager.raw_input(), "tai5");
    assert_eq!(rig.manager.display_text(), "tâi");
    assert_eq!(rig.recorder.preedits().last(), Some(&"tâi"));
}

/// USER's example (2026-09-09): `ka2`, Ctrl+← Ctrl+←, `h` → `kha2`, shown as
/// `khá` with the caret after the `h` (`ComposingManagerTests.swift`
/// `testMoveCaret_thenAppend_insertsWhereTheCaretIs`).
#[test]
fn move_caret_then_append_inserts_where_the_caret_is() {
    let _lock = engine_lock();
    let mut rig = rig();
    rig.type_text("ka2");
    rig.recorder.effects.clear();

    rig.manager
        .move_caret(CaretDirection::Left, &mut rig.recorder);
    assert_eq!(
        rig.recorder.effects,
        [Effect::UpdatePreedit {
            text: "ká".into(),
            caret_utf16: 2
        }],
        "a caret move re-marks the same text with the caret moved and asks for nothing else"
    );
    rig.manager
        .move_caret(CaretDirection::Left, &mut rig.recorder);
    assert_eq!(
        rig.recorder.effects.last(),
        Some(&Effect::UpdatePreedit {
            text: "ká".into(),
            caret_utf16: 1
        })
    );
    rig.recorder.effects.clear();

    rig.manager.append("h", &mut rig.recorder);

    assert_eq!(rig.manager.raw_input(), "kha2");
    assert_eq!(rig.manager.display_text(), "khá");
    assert_eq!(
        rig.recorder.effects.first(),
        Some(&Effect::UpdatePreedit {
            text: "khá".into(),
            caret_utf16: 2
        })
    );
}

// INVARIANT_EVERY_COMPOSING_OP_CARRIES_THE_RENDERING_CONFIG (behavioral-invariants.md §54)
/// Under hanji-first a nailed prefix takes no separator before the pending
/// tail (`台gi`), and every op after the nail — a caret move, a keystroke —
/// renders it the same way: the caret walks the tail (after `g` = 2,
/// before it = 1), stops at the nailed segment, and the `h` typed there
/// lands in the tail without a space appearing. Before this round the
/// mutators sent the base config and the first keystroke after a nail
/// showed `台 gi`.
#[test]
fn move_caret_and_typing_after_a_nail_keep_the_hanji_first_rendering() {
    let _lock = engine_lock();
    let mut rig = rig();
    rig.settings
        .edit(|doc| doc.set_bool(&keys::IS_HANJI_FIRST, true));
    rig.type_text("taigi");
    let tai = rig
        .candidates()
        .into_iter()
        .find(|c| c.hanji.as_deref() == Some("台") && c.consumed_span_end == 3)
        .expect("台 over `tai`");
    rig.manager
        .commit_candidate(&tai, CandidateScript::Primary, &mut rig.recorder);
    assert_eq!(rig.manager.display_text(), "台gi");
    assert_eq!(rig.manager.raw_input(), "gi");
    rig.recorder.effects.clear();

    for expected_caret in [2, 1] {
        rig.manager
            .move_caret(CaretDirection::Left, &mut rig.recorder);
        assert_eq!(
            rig.recorder.effects,
            [Effect::UpdatePreedit {
                text: "台gi".into(),
                caret_utf16: expected_caret,
            }]
        );
        rig.recorder.effects.clear();
    }

    rig.manager
        .move_caret(CaretDirection::Left, &mut rig.recorder);
    assert!(
        rig.recorder.effects.is_empty(),
        "the caret never enters the nailed segment"
    );

    rig.manager.append("h", &mut rig.recorder);
    assert_eq!(rig.manager.raw_input(), "hgi");
    assert_eq!(rig.manager.display_text(), "台hgi");
    assert_eq!(
        rig.recorder.effects.first(),
        Some(&Effect::UpdatePreedit {
            text: "台hgi".into(),
            caret_utf16: 2,
        })
    );
}

#[test]
fn move_caret_at_the_start_changes_nothing() {
    let _lock = engine_lock();
    let mut rig = rig();
    rig.type_text("k");
    rig.manager
        .move_caret(CaretDirection::Left, &mut rig.recorder);
    rig.recorder.effects.clear();

    rig.manager
        .move_caret(CaretDirection::Left, &mut rig.recorder);

    assert!(
        rig.recorder.effects.is_empty(),
        "nothing to step over, nothing to tell the host"
    );
    assert!(rig.manager.is_composing());
    assert_eq!(rig.manager.raw_input(), "k");
}

#[test]
fn commit_composition_writes_the_composition_and_ends_it() {
    let _lock = engine_lock();
    let mut rig = rig();
    rig.type_text("gua2");
    let committed = rig.manager.commit_composition(&mut rig.recorder);
    assert_eq!(committed.as_deref(), Some("guá"));
    assert!(!rig.manager.is_composing());
    assert_eq!(rig.manager.raw_input(), "");
    assert_eq!(rig.recorder.committed(), ["guá"]);
}

#[test]
fn commit_then_insert_reaches_the_host_as_one_write_and_drops_the_context() {
    let _lock = engine_lock();
    let mut rig = rig();
    rig.type_text("gua2");
    let committed = rig
        .manager
        .commit_composition_then_insert(" ", &mut rig.recorder);
    assert_eq!(committed.as_deref(), Some("guá "));
    assert_eq!(
        rig.recorder.committed(),
        ["guá "],
        "one mutation for preedit + space"
    );
    assert!(!rig.manager.is_composing());
}

#[test]
fn cancel_clears_without_writing_and_delete_to_empty_ends() {
    let _lock = engine_lock();
    let mut rig = rig();
    rig.type_text("tai");
    rig.manager.cancel_composition(&mut rig.recorder);
    assert!(rig.recorder.cleared());
    assert!(rig.recorder.committed().is_empty());
    assert!(!rig.manager.is_composing());

    rig.type_text("t");
    assert!(rig.manager.is_composing());
    rig.manager.delete_backward(&mut rig.recorder);
    assert!(!rig.manager.is_composing());
    assert_eq!(rig.manager.raw_input(), "");
}

#[test]
fn start_new_session_drops_the_composition_without_touching_the_client() {
    let _lock = engine_lock();
    let mut rig = rig();
    rig.type_text("tai");
    let before = rig.recorder.effects.len();
    rig.manager.start_new_session();
    assert!(!rig.manager.is_composing());
    assert_eq!(rig.manager.display_text(), "");
    assert_eq!(
        rig.recorder.effects.len(),
        before,
        "nothing is written into the old client"
    );
    // The engine dropped the old composition: a fresh append starts clean.
    rig.type_text("g");
    assert_eq!(rig.manager.raw_input(), "g");
}

// MARK: - ComposingManagerCandidateTests

#[test]
fn fetch_candidates_finds_candidates_while_composing_and_reports_idle_otherwise() {
    let _lock = engine_lock();
    let mut rig = rig();
    assert_eq!(
        rig.manager.fetch_candidates(),
        CandidateFetchOutcome::NotComposing
    );
    rig.type_text("taigi");
    let candidates = rig.candidates();
    assert!(candidates
        .iter()
        .any(|c| c.hanji.as_deref() == Some("台語")));
    assert!(
        rig.manager.is_composing(),
        "a fetch leaves the composition intact"
    );
    rig.type_text("k");
    assert_eq!(rig.manager.raw_input(), "taigik");
}

#[test]
fn commit_candidate_consuming_the_whole_buffer_writes_the_document_and_ends() {
    let _lock = engine_lock();
    let mut rig = rig();
    rig.type_text("taigi");
    let taigi = rig.candidate("台語");
    let (outcome, committed) = rig.commit(&taigi, CandidateScript::Primary);
    assert_eq!(
        outcome,
        CandidateCommitOutcome::Finalized {
            earns_auto_space: false
        }
    );
    assert_eq!(
        committed.as_deref(),
        Some("台語"),
        "hanji-first out of the box (USER 2026-09-18): a commit writes the hanji"
    );
    assert!(!rig.manager.is_composing());
    assert_eq!(rig.recorder.committed(), ["台語"]);
}

#[test]
fn commit_candidate_roman_output_writes_the_romanization_and_alternate_writes_the_other() {
    let _lock = engine_lock();
    let mut rig = rig();
    rig.settings
        .edit(|doc| doc.set_bool(&keys::IS_HANJI_FIRST, false));
    rig.type_text("taigi");
    let taigi = rig.candidate("台語");
    let (outcome, committed) = rig.commit(&taigi, CandidateScript::Primary);
    assert_eq!(
        outcome,
        CandidateCommitOutcome::Finalized {
            earns_auto_space: true
        }
    );
    assert_eq!(committed.as_deref(), Some("tâi-gí"));

    rig.type_text("taigi");
    let taigi = rig.candidate("台語");
    let (outcome, committed) = rig.commit(&taigi, CandidateScript::Alternate);
    assert_eq!(
        outcome,
        CandidateCommitOutcome::Finalized {
            earns_auto_space: false
        }
    );
    assert_eq!(
        committed.as_deref(),
        Some("台語"),
        "Space writes the other script"
    );
}

/// trace: engine `commit_text::lead` — the hanji-absent arm.
/// §34's literal is a one-script candidate, so it earns the auto space under
/// every mode; the old gate read `(script, swap)` and called it a hanji
/// commit in Hanji-first and Hanji with Romanization, the two modes that force the swap on.
#[test]
fn commit_candidate_with_no_hanji_wrote_romanization_under_every_mode() {
    let _lock = engine_lock();
    for (swapped, display_mode) in [
        (false, CandidateDisplayMode::SideBySide),
        (true, CandidateDisplayMode::SideBySide),
        (true, CandidateDisplayMode::Combined),
    ] {
        let mut rig = rig();
        rig.settings.edit(|doc| {
            doc.set_bool(&keys::IS_HANJI_FIRST, swapped);
            doc.set_choice(&keys::CANDIDATE_DISPLAY_MODE, display_mode);
            doc.set_bool(&keys::IS_LITERAL_ROMAN_CANDIDATE_ENABLED, true);
        });
        rig.type_text("taigi");
        let literal = rig
            .candidates()
            .into_iter()
            .find(|candidate| candidate.nonempty_hanji().is_none())
            .expect("§34 literal leads the desktop list");
        assert_eq!(literal.roman, "taigi");

        assert!(
            rig.commit_verdict(&literal, CandidateScript::Primary),
            "swapped={swapped} mode={display_mode:?}"
        );
    }
}

/// And the other direction is untouched: a Hanji commit earns nothing.
#[test]
fn commit_candidate_of_a_hanji_wrote_no_romanization_when_the_mode_leads_with_it() {
    let _lock = engine_lock();
    let mut rig = rig();
    rig.settings
        .edit(|doc| doc.set_bool(&keys::IS_HANJI_FIRST, true));
    rig.type_text("taigi");
    let taigi = rig.candidate("台語");

    assert!(!rig.commit_verdict(&taigi, CandidateScript::Primary));
}

#[test]
fn commit_candidate_consuming_part_of_the_buffer_nails_it_and_keeps_composing() {
    let _lock = engine_lock();
    let mut rig = rig();
    rig.type_text("taigi");
    let candidates = rig.candidates();
    let tai = candidates
        .iter()
        .find(|c| c.hanji.as_deref() == Some("台") && c.consumed_span_end == 3)
        .cloned()
        .expect("台 over `tai`");
    let (outcome, committed) = rig.commit(&tai, CandidateScript::Primary);
    assert_eq!(outcome, CandidateCommitOutcome::Nailed);
    assert_eq!(
        committed, None,
        "Model B writes nothing until the final commit"
    );
    assert!(rig.manager.is_composing());
    assert_eq!(
        rig.manager.raw_input(),
        "gi",
        "the mirror holds only the pending tail"
    );
    assert_eq!(
        rig.manager.display_text(),
        "台gi",
        "the nailed prefix shows in the committed script (hanji-first), the pending tail after it"
    );
}

#[test]
fn commit_candidate_after_the_composition_ended_is_ignored() {
    let _lock = engine_lock();
    let mut rig = rig();
    rig.type_text("taigi");
    let taigi = rig.candidate("台語");
    rig.manager.cancel_composition(&mut rig.recorder);
    let (outcome, committed) = rig.commit(&taigi, CandidateScript::Primary);
    assert_eq!(outcome, CandidateCommitOutcome::Ignored);
    assert_eq!(committed, None);
}

/// §34 with Show Typed Text First ON (off by default since 2026-10-02): a fresh bar has the
/// typed literal in slot 0 — one script — so the highlighted-candidate commit
/// that Enter routes to (`keys/intent.rs` `return_commits_the_candidate_and_shift_return_the_literal`;
/// the window opens on slot 0) writes exactly what was typed in either output
/// mode. The dictionary's first
/// candidate is one slot along. ⇧Enter's raw path is
/// `commit_composition_writes_the_composition_and_ends_it`.
#[test]
fn enter_on_a_fresh_bar_commits_the_typed_literal_in_either_mode() {
    let _lock = engine_lock();
    for swapped in [false, true] {
        let mut rig = rig();
        rig.settings.edit(|doc| {
            doc.set_bool(&keys::IS_HANJI_FIRST, swapped);
            doc.set_bool(&keys::IS_LITERAL_ROMAN_CANDIDATE_ENABLED, true);
        });
        rig.type_text("taigi");
        let candidates = rig.candidates();
        assert_eq!(candidates[0].display_text, "taigi", "swapped={swapped}");
        assert!(candidates[0].is_roman_only(), "swapped={swapped}");
        assert!(
            candidates[1].hanji.is_some(),
            "the dictionary's first candidate is next — swapped={swapped}"
        );
        let (outcome, committed) = rig.commit(&candidates[0], CandidateScript::Primary);
        assert_eq!(
            outcome,
            CandidateCommitOutcome::Finalized {
                earns_auto_space: true
            },
            "swapped={swapped}"
        );
        assert_eq!(committed.as_deref(), Some("taigi"), "swapped={swapped}");
        assert!(!rig.manager.is_composing(), "swapped={swapped}");
        assert_eq!(rig.recorder.committed(), ["taigi"], "swapped={swapped}");
    }
}

/// Show Typed Text First OFF, read through the real settings document: the forced §34
/// row is gone, so the bar opens on a two-script dictionary candidate and Enter
/// writes that word rather than the typed letters. Asserted against the cell the
/// bar actually offered, not against a fixed dictionary word — which reading
/// ranks first is the lattice's business, not this switch's.
#[test]
fn enter_on_a_fresh_bar_commits_the_dictionary_word_when_the_literal_row_is_off() {
    let _lock = engine_lock();
    let mut rig = rig();
    rig.settings
        .edit(|doc| doc.set_bool(&keys::IS_LITERAL_ROMAN_CANDIDATE_ENABLED, false));
    rig.type_text("taigi");
    let candidates = rig.candidates();
    assert!(
        candidates[0].hanji.is_some(),
        "nothing forces a one-script row to the front: {:?}",
        candidates[0]
    );
    let (outcome, committed) = rig.commit(&candidates[0], CandidateScript::Primary);
    assert_eq!(
        outcome,
        CandidateCommitOutcome::Finalized {
            earns_auto_space: false
        }
    );
    assert_eq!(committed.as_deref(), candidates[0].hanji.as_deref());
    assert_ne!(
        committed.as_deref(),
        Some("taigi"),
        "the dictionary word, not the typed literal"
    );
}

#[test]
fn alternate_on_a_single_script_candidate_commits_nothing() {
    let _lock = engine_lock();
    let mut rig = rig();
    rig.settings
        .edit(|doc| doc.set_bool(&keys::IS_LITERAL_ROMAN_CANDIDATE_ENABLED, true));
    rig.type_text("tai");
    let candidates = rig.candidates();
    let literal = candidates[0].clone();
    assert!(literal.is_roman_only());
    let (outcome, committed) = rig.commit(&literal, CandidateScript::Alternate);
    assert_eq!(outcome, CandidateCommitOutcome::Ignored);
    assert_eq!(committed, None);
    assert!(rig.manager.is_composing(), "the composition is untouched");
}

// MARK: - ComposingManagerLearningTests
//
// Counting a pick under its `(display text, canonical TL)` pair, whichever
// script it wrote, is the engine's since R5 — `continuous_commit_resolution.rs`
// `a_nail_then_a_final_pick_report_their_outcomes` (the usage triple) and
// `user_data_writes.rs` `engine_resolved_picks_are_counted_by_the_engine`.

#[test]
fn two_commits_report_their_readings_and_a_full_stop_is_reported_between() {
    let _lock = engine_lock();
    let mut rig = rig();
    rig.type_text("tai5");
    let tai = rig.candidate("台");
    rig.commit(&tai, CandidateScript::Primary);
    rig.advance_clock(500);
    rig.type_text("gi2");
    let gi = rig.candidate("語");
    rig.commit(&gi, CandidateScript::Primary);
    assert_eq!(
        *rig.memory.handshakes.lock().unwrap(),
        vec![
            Handshake::Selected {
                text: "台".to_owned(),
                roman: "tâi".to_owned(),
                now_ms: 1_000,
            },
            Handshake::Selected {
                text: "語".to_owned(),
                roman: "gí".to_owned(),
                now_ms: 1_500,
            },
        ]
    );
    rig.memory.handshakes.lock().unwrap().clear();

    // A full stop typed outside a composition is reported — the engine ends
    // the context on it (`decide.rs` sentence-end rule).
    rig.manager.note_character_typed_outside_composition("。");
    rig.advance_clock(500);
    rig.type_text("bun5");
    let bun = rig.candidate("文");
    rig.commit(&bun, CandidateScript::Primary);
    assert_eq!(rig.memory.reported(), vec!["。", "文"]);
}

// INVARIANT_NEXTWORD_PUNCTUATION_OUTSIDE_COMPOSITION_REACHES_ENGINE (§40)
#[test]
fn a_comma_is_reported_and_a_letter_outside_is_not() {
    let _lock = engine_lock();
    let mut rig = rig();
    rig.type_text("tai5");
    let tai = rig.candidate("台");
    rig.commit(&tai, CandidateScript::Primary);
    rig.manager.note_character_typed_outside_composition(",");
    rig.manager.note_character_typed_outside_composition("x");
    rig.advance_clock(500);
    rig.type_text("gi2");
    let gi = rig.candidate("語");
    rig.commit(&gi, CandidateScript::Primary);
    // The engine ends the context on the comma (a clause mark, `decide.rs`).
    assert_eq!(rig.memory.reported(), vec!["台", ",", "語"]);
}

#[test]
fn a_new_session_and_a_mid_composition_punctuation_both_forget_the_context() {
    let _lock = engine_lock();
    let mut rig = rig();
    rig.type_text("tai5");
    let tai = rig.candidate("台");
    rig.commit(&tai, CandidateScript::Primary);
    rig.manager.start_new_session();
    rig.type_text("gi2");
    let gi = rig.candidate("語");
    rig.commit(&gi, CandidateScript::Primary);
    assert_eq!(rig.memory.reported(), vec!["台", "∅", "語"]);
    rig.memory.handshakes.lock().unwrap().clear();

    // Punctuation committed mid-composition drops the context rather than
    // letting the next commit skip a word.
    rig.type_text("gua2");
    rig.manager
        .commit_composition_then_insert(".", &mut rig.recorder);
    rig.type_text("bun5");
    let bun = rig.candidate("文");
    rig.commit(&bun, CandidateScript::Primary);
    assert_eq!(rig.memory.reported(), vec!["∅", "文"]);
}

// MARK: - macOS ports (`DesktopPlatform::MacOS`)

/// trace: ComposingManagerLearningTests.swift:50-66 — a pair with one half
/// written in the other script is reported under the identity: each
/// handshake carries the candidate's display text and canonical TL, whichever
/// script reached the document.
#[test]
fn the_mac_reports_a_pair_written_in_two_scripts_under_its_identity() {
    let _lock = engine_lock();
    let mut rig = rig_on(DesktopPlatform::MacOS);
    rig.type_text("tai5");
    let tai = rig.candidate("台");
    rig.commit(&tai, CandidateScript::Primary);
    rig.type_text("gi2");
    let gi = rig.candidate("語");
    rig.commit(&gi, CandidateScript::Alternate);
    let reported: Vec<(String, String)> = rig
        .memory
        .handshakes
        .lock()
        .unwrap()
        .iter()
        .map(|handshake| match handshake {
            Handshake::Selected { text, roman, .. } => (text.clone(), roman.clone()),
            other => panic!("expected a selection, got {other:?}"),
        })
        .collect();
    assert_eq!(
        reported,
        vec![
            (tai.display_text, tai.canonical_tl),
            (gi.display_text, gi.canonical_tl),
        ]
    );
}

/// trace: ComposingManagerLearningTests.swift:183-201 — another manager's
/// generation resets the engine underneath this one; the next fetch answers
/// "not composing" and the mirror follows it.
#[test]
fn the_mac_mirror_follows_a_fetch_after_the_engine_was_reset_underneath_it() {
    let _lock = engine_lock();
    let mut rig = rig_on(DesktopPlatform::MacOS);
    rig.type_text("taigi");
    assert!(rig.manager.is_composing());
    let mut other = rig_on(DesktopPlatform::MacOS);
    other.type_text("t");
    assert_eq!(
        rig.manager.fetch_candidates(),
        CandidateFetchOutcome::NotComposing
    );
    assert!(!rig.manager.is_composing());
}

/// trace: RustEngineBridgeComposingTests.swift:115-151 — after a nail,
/// committing the composition writes what is shown once, nailed prefix
/// included, never the prefix twice.
#[test]
fn the_mac_commits_a_nailed_composition_once() {
    let _lock = engine_lock();
    let mut rig = rig_on(DesktopPlatform::MacOS);
    rig.type_text("taigi");
    let tai = rig
        .candidates()
        .into_iter()
        .find(|c| c.hanji.as_deref() == Some("台") && c.consumed_span_end == 3)
        .expect("台 over `tai`");
    assert_eq!(
        rig.commit(&tai, CandidateScript::Primary).0,
        CandidateCommitOutcome::Nailed
    );
    let shown = rig.manager.display_text().to_owned();
    rig.manager.commit_composition(&mut rig.recorder);
    assert_eq!(rig.recorder.committed(), [shown.as_str()]);
}

/// Pins the CURRENT core rule — roadmap E5, a real difference. Swift asks
/// `isLetter` / `isWhitespace` of each grapheme's first scalar
/// (`ComposingManager.swift:116-118`); the core asks every scalar, so a
/// non-letter base plus an Other_Alphabetic mark is forwarded only on the Mac.
#[test]
fn e5_a_mark_that_is_alphabetic_keeps_the_character_from_next_word() {
    // No engine call: the manager only hands the character to the port.
    let rig = rig_on(DesktopPlatform::MacOS);
    for character in [
        "。\u{345}", // core: skipped; Swift: forwarded
        ",\u{93E}",  // core: skipped; Swift: forwarded
        "。\u{301}", // U+0301 is not Alphabetic: forwarded on both
        " \u{301}",  // whitespace-led: skipped on both
        "\u{3000}",  // skipped on both
        "x",         // skipped on both
    ] {
        rig.manager
            .note_character_typed_outside_composition(character);
    }
    assert_eq!(rig.memory.reported(), vec!["。\u{301}"]);
}

/// E5 on picked symbols: every symbol of the shipped table reaches the
/// next-word context under the core's predicate, as every one does under
/// the Mac's (`ComposingBackendParityTests.swift`) — the difference never
/// reaches the picker.
#[test]
fn e5_every_bundled_symbol_reaches_next_word() {
    let rig = rig_on(DesktopPlatform::MacOS);
    let table = SymbolTable::bundled().expect("the bundled table parses");
    let symbols: Vec<&str> = table.symbols().collect();
    for symbol in &symbols {
        rig.manager.note_character_typed_outside_composition(symbol);
    }
    assert_eq!(rig.memory.reported(), symbols);
}

/// Roadmap E4, settled P11d: a format character is document text, so the
/// pass-through path hands it to the next-word gate. An isolated one and an
/// emoji ZWJ sequence carry no letter or whitespace and are forwarded;
/// `x‍y` stops on its letter. The Swift manager's side is
/// `ComposingBackendParityTests.testE4_anIdleFormatCharacterReachesTheNextWordGate`.
#[test]
fn e4_a_format_character_reaches_the_next_word_gate() {
    // No engine call: the manager only hands the character to the port.
    // trace: U+200B / U+00AD / U+FEFF / U+200D are neither Alphabetic nor
    // White_Space; neither are 👩 / 💻; `x` is Alphabetic.
    let rig = rig_on(DesktopPlatform::MacOS);
    for character in [
        "\u{200B}",
        "\u{AD}",
        "\u{FEFF}",
        "x\u{200D}y",
        "👩\u{200D}💻",
    ] {
        rig.manager
            .note_character_typed_outside_composition(character);
    }
    assert_eq!(
        rig.memory.reported(),
        vec!["\u{200B}", "\u{AD}", "\u{FEFF}", "👩\u{200D}💻"]
    );
}

// MARK: - ComposingSessionCoordinatorTests

#[test]
fn only_the_claiming_context_can_drive_the_engine_and_handover_starts_idle() {
    let _lock = engine_lock();
    let rig = rig();
    let mut coordinator = ComposingSessionCoordinator::new(rig.manager);
    // The shell allocates tokens (`text_service.rs::token_for`); `0` never
    // reaches the coordinator, so the pair here starts at 1.
    let a = ContextToken(1);
    let b = ContextToken(2);
    let mut recorder = Recorder::default();

    coordinator.claim(a).append("t", &mut recorder);
    assert!(
        coordinator.manager(b).is_none(),
        "b does not own the engine"
    );
    assert!(coordinator.manager(a).is_some());

    // Re-claiming an owned context leaves the composition running.
    assert!(coordinator.claim(a).is_composing());

    // Handover: b takes over from an idle engine.
    let manager = coordinator.claim(b);
    assert!(!manager.is_composing());
    assert_eq!(manager.raw_input(), "");
    assert!(coordinator.manager(a).is_none(), "a was superseded");

    // Releasing the superseded context leaves the live one alone.
    coordinator.claim(b).append("g", &mut recorder);
    coordinator.release(a);
    assert!(coordinator.manager(b).expect("b still owns").is_composing());

    // Releasing the live one frees the engine.
    coordinator.release(b);
    assert!(coordinator.manager(b).is_none());
    assert_eq!(coordinator.current_owner(), None);
}
