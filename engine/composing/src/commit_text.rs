//! What an R5 `CommitContinuous` writes into the composition — the document
//! text of one pick and whether it carries romanization — resolved by the
//! engine from the pick's scripts and the request's output settings.
//!
//! One port of the four platform resolvers (behavioral-invariants §23 / §42):
//! iOS `ActionHandler+Suggestions.swift` `formatOutputText` /
//! `markedCellCommit`, Android `CandidateClickHandler.kt`
//! `resolveUnmarkedCommit` / `resolveMarkedCellCommit`, desktop
//! `taigi-desktop-core` `composing/document_text.rs` `resolved_commit` /
//! `resolved_alternate` (macOS `CandidateDocumentText.swift`).
//!
//! TPS is not rendered here: the Bopomofo bracket romanization and the
//! Hanji-less TPS commit land with R5 PR-b (an open USER decision).

use protos::engine::{AppConfig, CandidateDisplayMode, CommitOutcome, CommitResolution};

use crate::api::CommitScript;

/// One pick's document text, and whether writing it puts romanization in
/// the document — the verdict the auto-space gate reads (§23).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResolvedCommit {
    pub(crate) text: String,
    pub(crate) wrote_romanization: bool,
}

impl ResolvedCommit {
    fn romanization(text: &str) -> Self {
        Self {
            text: text.to_owned(),
            wrote_romanization: true,
        }
    }

    fn hanji(text: &str) -> Self {
        Self {
            text: text.to_owned(),
            wrote_romanization: false,
        }
    }
}

/// The output settings a commit renders under, after the Candidate Display
/// projections (§42): Romanization Only masks every stored script flag;
/// Hanji with Romanization forces only the Hanji lead. The platforms already
/// send the projected flags; applying them here too keeps the resolver right
/// for any caller.
struct OutputScripts {
    hanji_leads: bool,
    annotate_in_brackets: bool,
    shows_hanji: bool,
}

impl OutputScripts {
    fn of(config: &AppConfig) -> Self {
        if config.is_roman_only_display() {
            return Self {
                hanji_leads: false,
                annotate_in_brackets: false,
                shows_hanji: false,
            };
        }
        Self {
            hanji_leads: renders_hanji_first(config),
            annotate_in_brackets: config.output_both_scripts,
            shows_hanji: true,
        }
    }
}

/// Whether a commit leads with the Hanji: the swap, or Hanji with
/// Romanization's forced lead. The one place the lead is decided.
fn renders_hanji_first(config: &AppConfig) -> bool {
    config.is_translate_swapped || config.candidate_display_mode() == CandidateDisplayMode::Combined
}

/// What committing `script` of the pick `(roman, hanji)` writes, or `None`
/// when that script does not exist (`Other` on a one-script pick or under
/// Romanization Only) — the commit is then ignored. An empty `hanji` is no
/// Hanji.
pub(crate) fn resolve_commit_text(
    script: CommitScript,
    roman: &str,
    hanji: Option<&str>,
    config: &AppConfig,
) -> Option<ResolvedCommit> {
    let scripts = OutputScripts::of(config);
    let hanji = hanji.filter(|hanji| !hanji.is_empty());
    match (script, hanji) {
        (CommitScript::Hanji, Some(hanji)) => {
            Some(if scripts.annotate_in_brackets && !roman.is_empty() {
                ResolvedCommit::romanization(&format!("{hanji} ({roman})"))
            } else {
                ResolvedCommit::hanji(hanji)
            })
        }
        (CommitScript::Roman, _) if !roman.is_empty() => Some(ResolvedCommit::romanization(roman)),
        (CommitScript::Other, None) => None,
        (CommitScript::Other, Some(hanji)) => scripts.shows_hanji.then(|| {
            if scripts.hanji_leads {
                ResolvedCommit::romanization(roman)
            } else {
                ResolvedCommit::hanji(hanji)
            }
        }),
        // Lead, and a split cell whose own script is missing.
        (_, hanji) => Some(lead(roman, hanji, &scripts)),
    }
}

fn lead(roman: &str, hanji: Option<&str>, scripts: &OutputScripts) -> ResolvedCommit {
    let Some(hanji) = hanji else {
        // §34 literal, an OOV name, a roman-only custom row: romanization
        // under every mode.
        return ResolvedCommit::romanization(roman);
    };
    if scripts.annotate_in_brackets {
        // The pair carries the romanization whichever half leads — even an
        // empty one (`台語 ()`, Android `resolveUnmarkedCommit`).
        let text = if scripts.hanji_leads {
            format!("{hanji} ({roman})")
        } else {
            format!("{roman} ({hanji})")
        };
        return ResolvedCommit::romanization(&text);
    }
    if scripts.hanji_leads {
        ResolvedCommit::hanji(hanji)
    } else {
        ResolvedCommit::romanization(roman)
    }
}

/// The wire answer for an R5 commit: `resolved` is what the pick wrote,
/// unless `outcome` says nothing changed. The auto-space verdict is the
/// platforms' `shouldAppendAutoSpace` minus the live setting, on a final
/// commit only.
pub(crate) fn commit_resolution(
    outcome: CommitOutcome,
    resolved: Option<ResolvedCommit>,
) -> CommitResolution {
    let Some(resolved) = resolved.filter(|_| outcome != CommitOutcome::Ignored) else {
        return CommitResolution {
            outcome: CommitOutcome::Ignored as i32,
            ..CommitResolution::default()
        };
    };
    let earns_auto_space = outcome == CommitOutcome::Finalized
        && resolved.wrote_romanization
        && !resolved.text.ends_with('-');
    CommitResolution {
        outcome: outcome as i32,
        document_text: resolved.text,
        wrote_romanization: resolved.wrote_romanization,
        earns_auto_space,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(swapped: bool, brackets: bool, display: CandidateDisplayMode) -> AppConfig {
        AppConfig {
            input_mode: "tl".into(),
            is_translate_swapped: swapped,
            output_both_scripts: brackets,
            candidate_display_mode: display as i32,
            ..AppConfig::default()
        }
    }

    #[test]
    fn truth_table() {
        use CandidateDisplayMode::{Combined as Mixed, RomanOnly, SideBySide as Pair};
        use CommitScript::{Hanji, Lead, Other, Roman};
        const R: &str = "tâi-gí";
        const H: Option<&str> = Some("台語");
        let roman = |text: &str| Some((text.to_owned(), true));
        let hanji = |text: &str| Some((text.to_owned(), false));
        // (script, swapped, brackets, display, roman, hanji) → (text, wrote_romanization)
        #[rustfmt::skip]
        let rows = [
            // trace: UnmarkedCommitResolverTest (Android) / formatOutputText (iOS)
            // romanLed_… / hanjiLed_… / brackets_writeThePairEitherWayRound_…
            (Lead, false, false, Pair, R, H, roman(R)),
            (Lead, true, false, Pair, R, H, hanji("台語")),
            (Lead, true, true, Pair, R, H, roman("台語 (tâi-gí)")),
            (Lead, false, true, Pair, R, H, roman("tâi-gí (台語)")),
            // Android `bracketedCommit` with an empty roman keeps its brackets.
            (Lead, true, true, Pair, "", H, roman("台語 ()")),
            // noHanji_commitsTheRomanizationUnderEveryMode; document_text.rs
            // a_candidate_with_no_hanji_always_carries_romanization
            (Lead, true, false, Pair, R, None, roman(R)),
            (Lead, true, true, Pair, R, None, roman(R)),
            (Lead, false, true, Pair, R, Some(""), roman(R)),
            (Lead, true, false, Pair, R, Some(""), roman(R)),
            // document_text.rs roman_only_mode_shows_and_writes_the_romanization_alone
            (Lead, true, true, RomanOnly, R, H, roman(R)),
            // trace: MarkedCellCommitResolverTest / ActionHandlerMarkedCellCommitTests
            // hanjiCell_bracketsOff_… / hanjiCell_bracketsOn_… / …_missingRoman_…
            (Hanji, true, false, Mixed, R, H, hanji("台語")),
            (Hanji, true, true, Mixed, R, H, roman("台語 (tâi-gí)")),
            (Hanji, true, true, Mixed, "", H, hanji("台語")),
            // test_INVARIANT_roman_cell_commits_bare_roman_even_with_brackets_on
            (Roman, true, false, Mixed, R, H, roman(R)),
            (Roman, true, true, Mixed, R, H, roman(R)),
            // defectiveMarkers_failOpenToTheUnmarkedPath: the lead instead
            (Hanji, true, true, Mixed, R, None, roman(R)),
            (Roman, true, false, Mixed, "", H, hanji("台語")),
            // trace: document_text.rs the_alternate_verdict_inverts_the_mode /
            // alternate_text_follows_the_display_mode_not_the_cell
            (Other, true, false, Pair, R, H, roman(R)),
            (Other, false, false, Pair, R, H, hanji("台語")),
            (Other, false, false, Mixed, R, H, roman(R)),
            (Other, true, false, Pair, R, None, None),
            (Other, true, false, Pair, R, Some(""), None),
            (Other, true, true, RomanOnly, R, H, None),
        ];
        for (script, swapped, brackets, display, roman, hanji, expected) in rows {
            let config = config(swapped, brackets, display);
            let actual = resolve_commit_text(script, roman, hanji, &config)
                .map(|resolved| (resolved.text, resolved.wrote_romanization));
            assert_eq!(
                actual, expected,
                "{script:?} swapped={swapped} brackets={brackets} {display:?} {roman:?} {hanji:?}"
            );
        }
    }

    #[test]
    fn resolution_earns_auto_space_only_on_a_romanized_final_commit() {
        // trace: ActionHandler.shouldAppendAutoSpace /
        // MarkedCellCommitResolverTest.shouldAppendAutoSpace_requiresSettingRomanizationAndNoHyphenTail,
        // minus the live setting; the caller's final-commit gate folded in.
        let pick = |text: &str, wrote_romanization| {
            Some(ResolvedCommit {
                text: text.to_owned(),
                wrote_romanization,
            })
        };
        let finalized = commit_resolution(CommitOutcome::Finalized, pick("tâi-gí", true));
        assert_eq!(finalized.outcome, CommitOutcome::Finalized as i32);
        assert_eq!(finalized.document_text, "tâi-gí");
        assert!(finalized.wrote_romanization && finalized.earns_auto_space);
        assert!(!commit_resolution(CommitOutcome::Nailed, pick("tâi-gí", true)).earns_auto_space);
        assert!(!commit_resolution(CommitOutcome::Finalized, pick("台語", false)).earns_auto_space);
        assert!(!commit_resolution(CommitOutcome::Finalized, pick("tâi-", true)).earns_auto_space);
        let ignored = commit_resolution(CommitOutcome::Ignored, pick("tâi-gí", true));
        assert_eq!(
            ignored,
            CommitResolution {
                outcome: CommitOutcome::Ignored as i32,
                ..CommitResolution::default()
            }
        );
    }
}
