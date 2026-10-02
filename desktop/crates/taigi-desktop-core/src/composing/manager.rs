//! Turns a user intent into an engine round-trip, mirrors what came back, and
//! hands the engine's effects to the executor for the client that asked.
//! Port of `ComposingManager.swift`.
//!
//! The engine owns the composition (phase, raw buffer, candidate index); this
//! owns only a mirror of the last answer, which the controller reads to
//! decide whether a key belongs to the composition or to the host.

use std::sync::Arc;

use super::cell_content::CandidateScript;
use super::clock::Clock;
use super::next_word::NextWordPort;
use super::outcomes::{CandidateCommitOutcome, CandidateFetchOutcome};
use super::presentation::{leads_with_literal_roman, presentation, PresentedCandidate};
use crate::engine::{self, CommitContinuousArgs, ComposingTransition, ContinuousCandidate, Effect};
use crate::keys::CaretDirection;
use crate::platform::DesktopPlatform;
use crate::settings::{EngineSettings, SettingsProvider};

/// Writes the engine's document effects into the client that is currently
/// focused. Learning handshakes never reach it — the manager reports them to
/// the next-word port, because they write to a database rather than a document.
pub trait ComposingEffectExecutor {
    fn execute(&mut self, effect: &Effect);
}

pub struct ComposingManager {
    is_composing: bool,
    /// What the user typed, with numeric tones. Once a candidate has been
    /// nailed this is only the pending tail (`transition.rs:585`).
    raw_input: String,
    /// The composition as the preedit renders it — the whole thing, nailed
    /// prefix included. Mirrored because only the engine knows how segments
    /// join, and because the candidate window anchors to what is on screen.
    display_text: String,
    settings: Arc<dyn SettingsProvider>,
    /// Where the next-word handshakes go, stamped with `clock`.
    next_word: Box<dyn NextWordPort>,
    clock: Box<dyn Clock>,
    /// The desktop every engine request names (`AppConfig.platform_id`).
    platform: DesktopPlatform,
    /// Unique across everything that talks to the engine in this process:
    /// the engine keeps one composition per process and drops it whenever the
    /// generation it is handed changes.
    current_generation: u64,
}

impl ComposingManager {
    /// `starting_generation` defaults to 1 in production because 0 is the
    /// generation an unset proto field carries.
    pub fn new(
        settings: Arc<dyn SettingsProvider>,
        next_word: Box<dyn NextWordPort>,
        clock: Box<dyn Clock>,
        platform: DesktopPlatform,
        starting_generation: u64,
    ) -> Self {
        Self {
            is_composing: false,
            raw_input: String::new(),
            display_text: String::new(),
            settings,
            next_word,
            clock,
            platform,
            current_generation: starting_generation,
        }
    }

    pub fn is_composing(&self) -> bool {
        self.is_composing
    }

    pub fn raw_input(&self) -> &str {
        &self.raw_input
    }

    pub fn display_text(&self) -> &str {
        &self.display_text
    }

    pub fn generation(&self) -> u64 {
        self.current_generation
    }

    fn current_settings(&self) -> EngineSettings {
        self.settings.current().engine_settings()
    }

    // MARK: - Session lifecycle

    /// Abandons any composition without touching a document, so the next
    /// session starts from an idle engine. Sends no `Reset`: the engine drops
    /// its state as soon as it sees the new generation (`handle.rs:61-66`).
    /// Does NOT clear the preedit on screen — that is the outgoing session's
    /// job while it still has its context.
    pub fn start_new_session(&mut self) {
        self.current_generation = self.current_generation.wrapping_add(1);
        self.clear_mirror();
        // The next-word context goes with the composition: a session change is
        // usually a change of application, and carrying the context across
        // would learn the last word typed in a chat window as the predecessor
        // of the first word typed in a terminal.
        self.next_word.forget_context(
            self.clock.now_ms(),
            &self.current_settings(),
            self.current_generation,
        );
    }

    /// A character reached the host without going through a composition.
    /// Forwarded so the engine can end the context on sentence-end
    /// punctuation; letters (they start compositions) and whitespace (never
    /// punctuation, and a round-trip per space bar) are excluded.
    pub fn note_character_typed_outside_composition(&self, character: &str) {
        if character.is_empty()
            || character
                .chars()
                .any(|c| c.is_alphabetic() || c.is_whitespace())
        {
            return;
        }
        self.next_word.word_selected(
            character,
            "",
            &[],
            self.clock.now_ms(),
            &self.current_settings(),
            self.current_generation,
        );
    }

    /// Appends one typed character. The engine starts a composition when idle,
    /// so there is no separate "begin" call — one engine request per key.
    pub fn append(&mut self, character: &str, executor: &mut dyn ComposingEffectExecutor) {
        log::debug!("append");
        let settings = self.current_settings();
        let transition =
            engine::append(character, &settings, self.platform, self.current_generation);
        self.apply(transition, executor);
    }

    /// Applies one Telex key — a tone letter, `z` or `f` — to the pending
    /// syllable. Shaped like `append` because it is the same step with the
    /// engine deciding what the key writes (`engine/composing/src/telex.rs`):
    /// an idle `z` starts a composition the way a letter does.
    pub fn telex_key(&mut self, key: &str, executor: &mut dyn ComposingEffectExecutor) {
        log::debug!("telexKey");
        let settings = self.current_settings();
        let transition = engine::telex_key(key, &settings, self.platform, self.current_generation);
        self.apply(transition, executor);
    }

    /// Drops the last character of the raw buffer. Ends the composition when
    /// that empties it.
    pub fn delete_backward(&mut self, executor: &mut dyn ComposingEffectExecutor) {
        log::debug!("deleteBackward");
        let transition = engine::delete_backward(
            &self.current_settings(),
            self.platform,
            self.current_generation,
        );
        self.apply(transition, executor);
    }

    /// Steps the caret inside the pending tail. Not a buffer change: the
    /// engine asks for no fetch — the candidates on
    /// screen still describe the same text (`ComposingManager.swift`
    /// `moveCaret`).
    pub fn move_caret(
        &mut self,
        direction: CaretDirection,
        executor: &mut dyn ComposingEffectExecutor,
    ) {
        log::debug!("moveCaret");
        let transition = engine::move_caret(
            direction,
            &self.current_settings(),
            self.platform,
            self.current_generation,
        );
        self.apply(transition, executor);
    }

    /// Commits the composition as rendered (`CommitRaw`). Answers the text
    /// the commit wrote, or `None` for a commit that never reached the engine
    /// or wrote nothing — what auto-space earns its trailing space from.
    pub fn commit_composition(
        &mut self,
        executor: &mut dyn ComposingEffectExecutor,
    ) -> Option<String> {
        log::debug!("commitComposition");
        let transition = engine::commit_raw(
            &self.current_settings(),
            self.platform,
            self.current_generation,
        );
        let committed = Self::committed_text(transition.as_ref());
        self.apply(transition, executor);
        committed
    }

    /// Commits the composition and appends `text` after it in the same engine
    /// step, so one keystroke reaches the host as one document mutation.
    pub fn commit_composition_then_insert(
        &mut self,
        text: &str,
        executor: &mut dyn ComposingEffectExecutor,
    ) -> Option<String> {
        log::debug!("commitCompositionThenInsert");
        let settings = self.current_settings();
        let transition = engine::commit_preedit_then_insert_external(
            text,
            &settings,
            self.platform,
            self.current_generation,
        );
        let committed = Self::committed_text(transition.as_ref());
        self.apply(transition, executor);
        // The one commit path the engine does not describe to next word: it
        // emits `NextWordClearForNewComposing` and no `NextWordWordSelected`
        // (`transition.rs:769-780`). If the context were left alone, the NEXT
        // commit would pair itself with whatever was committed BEFORE this one.
        // Dropping the context under-learns one pair rather than learning a
        // wrong one.
        self.next_word
            .forget_context(self.clock.now_ms(), &settings, self.current_generation);
        committed
    }

    /// Abandons the composition. Nothing reaches the document.
    pub fn cancel_composition(&mut self, executor: &mut dyn ComposingEffectExecutor) {
        log::debug!("cancelComposition");
        let transition = engine::reset(self.current_generation);
        self.apply(transition, executor);
    }

    // MARK: - Candidates

    /// Reads the candidates for the composition as it stands, ranked against
    /// what the user has committed before. One fetch: the engine reads the
    /// user's own data — frequency, custom dictionary, learned phrases —
    /// and ranks with it itself (user-data-engine-roadmap P3b / P5).
    pub fn fetch_candidates(&mut self) -> CandidateFetchOutcome {
        let settings = self.current_settings();
        let Some(fetched) = engine::fetch_at_pos(
            &settings,
            self.platform,
            self.current_generation,
            self.clock.now_ms(),
        ) else {
            return CandidateFetchOutcome::Unavailable;
        };
        self.mirror(&fetched.transition);
        match fetched.candidates {
            Some(candidates) => CandidateFetchOutcome::Found(candidates),
            None => CandidateFetchOutcome::NotComposing,
        }
    }

    /// The cells the window shows for `candidates`, under one snapshot of the
    /// settings in force right now — Combined splits a candidate into two, so the
    /// window's indices are cell indices (`CandidateSource::resolve`) — plus
    /// whether the first cell is the §34 literal, which takes no slot key.
    ///
    /// ONE snapshot for BOTH answers, so the cells and the key row can never
    /// be resolved against two different instants. Port of macOS
    /// `ComposingManager.presentation(for:)`.
    pub fn presentation(
        &self,
        candidates: &[ContinuousCandidate],
    ) -> (Vec<PresentedCandidate>, bool) {
        let settings = self.current_settings();
        (
            presentation(candidates, &settings),
            leads_with_literal_roman(candidates, &settings),
        )
    }

    /// Commits `candidate`, which must come from the `fetch_candidates` call
    /// that produced the list the user is looking at. `script` picks WHICH of
    /// the candidate's two renderings the document gets; the engine resolves
    /// the text under the same settings snapshot, answers `Ignored` for a
    /// script the candidate does not have (Space on a one-script cell), and —
    /// with the user data open — counts the pick under its `(display text,
    /// canonical TL)` identity (Core Principle #7), never the rendering.
    pub fn commit_candidate(
        &mut self,
        candidate: &ContinuousCandidate,
        script: CandidateScript,
        executor: &mut dyn ComposingEffectExecutor,
    ) -> CandidateCommitOutcome {
        log::debug!(
            "commitCandidate consumedBytes={}",
            candidate.consumed_span_end
        );
        let Some(committed) = engine::commit_continuous(
            &CommitContinuousArgs {
                script: match script {
                    CandidateScript::Primary => engine::CommitScript::Lead,
                    CandidateScript::Alternate => engine::CommitScript::Other,
                },
                roman: &candidate.roman,
                canonical_text: &candidate.display_text,
                association_tl: &candidate.canonical_tl,
                hanji: candidate.hanji.as_deref(),
                consumed_bytes: candidate.consumed_span_end,
                syllable_count: candidate.syllable_count,
            },
            &self.current_settings(),
            self.platform,
            self.current_generation,
        ) else {
            return CandidateCommitOutcome::Unavailable;
        };
        let outcome = CandidateCommitOutcome::from_resolution(&committed.commit);
        self.apply(Some(committed.transition), executor);
        outcome
    }

    /// The text `transition` wrote to the document, read off the effects —
    /// the mirror carries the display rendering, which the output settings
    /// can make differ from the document string.
    fn committed_text(transition: Option<&ComposingTransition>) -> Option<String> {
        transition?
            .effects
            .iter()
            .filter_map(|effect| match effect {
                Effect::CommitTextReplacingPreedit(text) => Some(text.clone()),
                _ => None,
            })
            .next_back()
    }

    /// Mirror first, then run the effects in the order the engine listed
    /// them. `None` is a round-trip that never reached the engine: nothing to
    /// mirror and nothing to perform.
    fn apply(
        &mut self,
        transition: Option<ComposingTransition>,
        executor: &mut dyn ComposingEffectExecutor,
    ) {
        let Some(transition) = transition else { return };
        self.mirror(&transition);
        let settings = self.current_settings();
        for effect in &transition.effects {
            match effect {
                // Learning handshakes are not document effects; routed here so
                // the executor keeps its one job and the generation stays out
                // of the effect path.
                Effect::NextWordWordSelected {
                    text,
                    roman,
                    preceding,
                    ..
                } => {
                    self.next_word.word_selected(
                        text,
                        roman,
                        preceding,
                        self.clock.now_ms(),
                        &settings,
                        self.current_generation,
                    );
                }
                Effect::NextWordUpdateLastSelectedWord { text, roman } => {
                    self.next_word.segment_nailed(
                        text,
                        roman,
                        self.clock.now_ms(),
                        &settings,
                        self.current_generation,
                    );
                }
                // Hides predictions while keeping the context. The desktop
                // shows no predictions, so there is nothing to hide and the
                // context is exactly what must survive.
                Effect::NextWordClearForNewComposing => {}
                Effect::UpdatePreedit { .. }
                | Effect::ClearPreeditWithoutCommit
                | Effect::CommitTextReplacingPreedit(_)
                | Effect::ClearCandidates
                | Effect::RefreshCandidates
                | Effect::ResetCandidateContext => executor.execute(effect),
            }
        }
    }

    /// Updates the mirror from an engine answer, without performing anything.
    /// The read paths use this directly: a fetch response still carries the
    /// authoritative composition state.
    fn mirror(&mut self, transition: &ComposingTransition) {
        self.is_composing = transition.is_composing;
        self.raw_input = transition.raw_input.clone();
        self.display_text = transition.display_text.clone();
    }

    fn clear_mirror(&mut self) {
        self.is_composing = false;
        self.raw_input.clear();
        self.display_text.clear();
    }
}
