//! Literal tone placement for unseparated input. Tone digits close a segment.
//! A segment that is one valid syllable takes the same mark as
//! `convert_syllable` (`tl::place_tl_tone_mark` / `poj::place_poj_tone_mark`);
//! any other segment hands its last vowel cluster through the segment end to
//! those rules, matching Taigi Telex's last-cluster placement
//! (`TelexRules.findTonePosition`). This is a preview affordance, never a
//! dictionary spelling conversion.

use crate::api::InputMode;
use crate::normalization::is_combining_tone_mark;
use crate::poj::place_poj_tone_mark;
use crate::syllable::is_valid_syllable;
use crate::tables::{poj_tone_mark, tl_tone_mark};
use crate::tl::place_tl_tone_mark;
use unicode_normalization::{char::canonical_combining_class, UnicodeNormalization};

const POJ_DOT: char = '\u{0358}';
/// Stands in for the tone mark while the shared placement rules run, so the
/// letter they choose can be read back as a position in the segment.
const PLACEMENT_PROBE: char = '\u{E000}';

pub(crate) fn apply(input: &str, mode: InputMode) -> String {
    apply_recording_digits(input, mode, |_| {})
}

/// One entry per `1`–`9` in `input`, in order: `true` when that digit was
/// consumed as a tone. `0` is never a tone and is not recorded, matching the
/// raw-side enumeration in `api::consumed_tone_digit_offsets`.
pub(crate) fn consumed_digits(input: &str, mode: InputMode) -> Vec<bool> {
    let mut consumed = Vec::new();
    apply_recording_digits(input, mode, |is_consumed| consumed.push(is_consumed));
    consumed
}

fn apply_recording_digits(input: &str, mode: InputMode, mut record: impl FnMut(bool)) -> String {
    let mut output = String::with_capacity(input.len());
    let mut segment = String::new();
    let input_chars: Vec<char> = input.chars().collect();
    for (position, &ch) in input_chars.iter().enumerate() {
        if matches!(ch, '1'..='9') {
            // A syllable carries one tone digit, so a digit next to another
            // digit is part of a number (`covid19`, `win10`) and stays as typed.
            let is_in_digit_run = position
                .checked_sub(1)
                .is_some_and(|previous| input_chars[previous].is_ascii_digit())
                || input_chars
                    .get(position + 1)
                    .is_some_and(char::is_ascii_digit);
            let chars: Vec<char> = segment.nfd().collect();
            let target = if is_in_digit_run {
                None
            } else {
                tone_position(&chars, mode)
            };
            record(target.is_some());
            if let Some(target) = target {
                let tone = ch.to_string();
                let mark = if mode == InputMode::Poj {
                    poj_tone_mark(&tone)
                } else {
                    tl_tone_mark(&tone)
                };
                let mut at_target = false;
                for (index, &letter) in chars.iter().enumerate() {
                    if canonical_combining_class(letter) == 0 {
                        at_target = index == target;
                    } else if at_target && is_combining_tone_mark(letter) {
                        // A pasted diacritic plus a new tone digit replaces
                        // that letter's old tone; marks on earlier clusters stay.
                        continue;
                    }
                    output.push(letter);
                    if index == target {
                        output.push_str(mark);
                    }
                }
            } else {
                output.push_str(&segment);
                output.push(ch);
            }
            segment.clear();
        } else if ch.is_alphabetic() || canonical_combining_class(ch) != 0 {
            segment.push(ch);
        } else {
            // Typed separators and punctuation are hard boundaries: a digit
            // after one must not reach back into an earlier word.
            output.push_str(&segment);
            segment.clear();
            output.push(ch);
        }
    }
    output.push_str(&segment);
    output.nfc().collect()
}

/// Index into `chars` (the segment, NFD) of the letter that takes the tone.
fn tone_position(chars: &[char], mode: InputMode) -> Option<usize> {
    // Tone marks already on the segment are ignored when choosing; the POJ
    // dot stays, as the placement rules read `o͘` as one vowel.
    let letters: Vec<(usize, char)> = chars
        .iter()
        .enumerate()
        .filter(|(_, ch)| canonical_combining_class(**ch) == 0 || **ch == POJ_DOT)
        .map(|(index, ch)| (index, ch.to_ascii_lowercase()))
        .collect();
    let spelling: String = letters.iter().map(|&(_, ch)| ch).collect();
    let cluster_start = last_vowel_cluster_start(&letters, mode);
    let start = if is_valid_syllable(&spelling) {
        0
    } else if let Some(start) = cluster_start {
        start
    } else {
        return last_syllabic_consonant(&letters);
    };
    let tail: String = spelling.chars().skip(start).collect();
    let probe = PLACEMENT_PROBE.to_string();
    let is_tl_spelled_ua_ue = cluster_start.is_some_and(|start| starts_tl_ua_ue(&letters[start..]));
    let placed = if mode == InputMode::Poj && !is_tl_spelled_ua_ue {
        place_poj_tone_mark(&tail, &probe)
    } else {
        place_tl_tone_mark(&tail, &probe)
    };
    // The rules only insert the probe after the chosen letter.
    let letters_before_probe = placed.chars().position(|ch| ch == PLACEMENT_PROBE)?;
    letters[start..]
        .get(letters_before_probe.checked_sub(1)?)
        .map(|&(index, _)| index)
}

/// Position in `letters` where the last run of vowels starts. In POJ the
/// dot ends a run (`cho͘a` → `a`), and a run that is only `o͘` starts at its `o`.
fn last_vowel_cluster_start(letters: &[(usize, char)], mode: InputMode) -> Option<usize> {
    let mut start = None;
    for (position, &(_, ch)) in letters.iter().enumerate().rev() {
        if mode == InputMode::Poj && ch == POJ_DOT {
            if start.is_some() {
                break;
            }
            let o_position = position.checked_sub(1)?;
            return (letters[o_position].1 == 'o').then_some(o_position);
        }
        if "aeiou".contains(ch) {
            start = Some(position);
        } else if start.is_some() {
            break;
        }
    }
    start
}

/// A vowel-less segment that is not one syllable: the last syllabic `m` or
/// `ng` takes the tone (`hngm2` → `hngḿ`).
fn last_syllabic_consonant(letters: &[(usize, char)]) -> Option<usize> {
    letters
        .iter()
        .enumerate()
        .rev()
        .find_map(|(position, &(index, ch))| {
            (ch == 'm'
                || (ch == 'n'
                    && letters
                        .get(position + 1)
                        .is_some_and(|&(_, next)| next == 'g')))
            .then_some(index)
        })
}

/// A vowel cluster spelled TL-style `ua…` / `ue…`. Typed in POJ mode it keeps
/// TL placement (`gua2` → `guá`); POJ's own pair rule would give `gúa`, a
/// form neither system writes (USER 2026-10-08). The learning key keeps the
/// old `gúa` (`api::literal_learning_text`), so stored rows do not fork.
fn starts_tl_ua_ue(cluster: &[(usize, char)]) -> bool {
    matches!(cluster, [(_, 'u'), (_, 'a' | 'e'), ..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telex_reference_last_cluster_samples() {
        // Numeric equivalents of Taigi Telex's Multiple Vowel Clusters and
        // Syllabic Consonants fixtures; spelling stays literal in this engine.
        for (input, expected) in [
            ("taigi2", "taigí"),
            ("haksa2", "haksá"),
            ("bunhua2", "bunhuá"),
            ("tainan2", "tainán"),
            ("sengli2", "senglí"),
            ("ng2", "ńg"),
            ("m2", "ḿ"),
            ("png2", "pńg"),
            ("hngm2", "hngḿ"),
        ] {
            for mode in [InputMode::Tl, InputMode::Poj] {
                assert_eq!(apply(input, mode), expected, "{mode:?}: {input}");
            }
        }
    }

    #[test]
    fn telex_reference_vowel_and_dot_exceptions() {
        for (input, expected) in [
            ("iu2", "iú"),
            ("ui2", "uí"),
            ("ere5", "erê"),
            ("iri5", "irî"),
        ] {
            assert_eq!(apply(input, InputMode::Tl), expected, "{input}");
        }
        for (input, expected) in [
            ("goa2", "góa"),
            ("oai2", "oái"),
            ("oang2", "oáng"),
            ("oan2", "oán"),
            ("oat2", "oát"),
            ("oah2", "oáh"),
            ("oeh2", "oéh"),
            ("cho͘a2", "cho͘á"),
            ("ho͘e2", "ho͘é"),
            ("ko͘ai2", "ko͘ái"),
            ("pho͘an2", "pho͘án"),
            ("cho͘i2", "cho͘í"),
            ("ho͘2", "hó͘"),
        ] {
            assert_eq!(apply(input, InputMode::Poj), expected, "{input}");
        }
    }

    #[test]
    fn normalize_tone_is_permissive_by_default_and_keeps_poj_affordances() {
        use crate::api::normalize_tone;
        use protos::engine::AppConfig;
        let mut config = AppConfig {
            input_mode: "tl".into(),
            ..Default::default()
        };
        assert_eq!(normalize_tone("tai5gi2", &config), "tâigí");
        for mode in ["english", "tps"] {
            config.input_mode = mode.into();
            assert_eq!(normalize_tone("tai5gi2", &config), "tai5gi2");
        }
        config.input_mode = "poj".into();
        config.oo_doubletap_enabled = true;
        config.nn_doubletap_enabled = true;
        assert_eq!(normalize_tone("hoo2ann5", &config), "hó͘âⁿ");
        assert_eq!(normalize_tone("HOO2ANN5", &config), "HÓ͘Âᴺ");
        config.force_lowercase_nasal_marker = true;
        assert_eq!(normalize_tone("HOO2ANN5", &config), "HÓ͘Âⁿ");
    }

    #[test]
    fn digits_close_segments_without_inserting_separators() {
        for (input, expected) in [
            ("tai5gi2", "tâigí"),
            ("tai5g", "tâig"),
            ("goa2ai3li2", "goáàilí"),
            ("Tai5GI2", "TâiGÍ"),
            ("tâigi2", "tâigí"),
            ("á3", "à"),
            ("á1", "a"),
            ("tai1gi4", "taigi"),
            ("a6", "ǎ"),
            ("tai5--gi2", "tâi--gí"),
            ("tai5 gi2", "tâi gí"),
            ("tai-2", "tai-2"),
            ("tai 2", "tai 2"),
            ("t2", "t2"),
            ("123", "123"),
            ("a0", "a0"),
        ] {
            assert_eq!(apply(input, InputMode::Tl), expected, "{input}");
        }
        assert_eq!(apply("a9", InputMode::Tl), "a̋");
        assert_eq!(apply("a9", InputMode::Poj), "ă");
    }

    #[test]
    fn existing_tones_do_not_hide_codas_or_syllabic_ng() {
        for (input, expected) in [
            ("oán5", "oân"),
            ("oàn1", "oan"),
            ("oáh5", "oâh"),
            ("oéh5", "oêh"),
        ] {
            assert_eq!(apply(input, InputMode::Poj), expected, "{input}");
        }
        for mode in [InputMode::Tl, InputMode::Poj] {
            assert_eq!(apply("n̂g2", mode), "ńg");
        }
    }

    #[test]
    fn valid_syllables_keep_the_canonical_mark() {
        // `ngm` is one table syllable, so it takes `convert_syllable`'s mark
        // (`ng` before `m`) rather than the last syllabic consonant.
        assert_eq!(apply("ngm2", InputMode::Tl), "ńgm");
        // POJ oa / oe before a coda mark the second vowel, as
        // `taigi-converter` does (`uang2` → `oáng`, `uannh5` → `oâⁿh`).
        for (input, expected) in [
            ("hoang2", "hoáng"),
            ("boak2", "boák"),
            ("hoa\u{207f}h5", "ho\u{e2}\u{207f}h"),
            ("hoe\u{207f}h3", "ho\u{e8}\u{207f}h"),
            ("hoenn2", "hoénn"),
            ("hoe\u{207f}2", "hóe\u{207f}"),
        ] {
            assert_eq!(apply(input, InputMode::Poj), expected, "{input}");
        }
    }

    #[test]
    fn poj_mode_marks_tl_spelled_ua_ue_like_tl() {
        for (input, expected) in [
            ("gua2", "guá"),
            ("guan2", "guán"),
            ("bue2", "bué"),
            ("kuai2", "kuái"),
            ("gua9", "gu\u{103}"),
            ("taigua2", "taiguá"),
        ] {
            assert_eq!(apply(input, InputMode::Poj), expected, "{input}");
        }
    }

    #[test]
    fn digit_runs_stay_as_typed() {
        for (input, expected) in [
            ("covid19", "covid19"),
            ("win10", "win10"),
            ("a23", "a23"),
            ("a22", "a22"),
            ("tai55gi2", "tai55gí"),
            ("a01b2", "a01b2"),
            ("a1b2", "ab2"),
        ] {
            assert_eq!(apply(input, InputMode::Tl), expected, "{input}");
        }
        // `0` is never recorded; digits inside a run are recorded unconsumed.
        assert_eq!(consumed_digits("a10b2", InputMode::Tl), vec![false, false]);
        assert_eq!(
            consumed_digits("tai5gi22", InputMode::Tl),
            vec![true, false, false]
        );
    }

    /// Every table syllable, in each case form, renders exactly as the
    /// single-syllable `convert_syllable` path (`api::to_tone_marks`) does,
    /// except a TL `ua` / `ue` spelling typed in POJ mode.
    #[test]
    fn every_valid_syllable_matches_convert_syllable() {
        use crate::api::to_tone_marks;
        use crate::tables::{TL_FINALS, TL_INITIALS};
        let mut compared = 0;
        for initial in TL_INITIALS.iter() {
            for final_str in TL_FINALS.iter() {
                let tl = format!("{initial}{final_str}");
                let poj = crate::poj::to_poj(initial, final_str, "1");
                let mut cases = vec![(InputMode::Tl, tl.clone()), (InputMode::Poj, poj)];
                // A TL `ua` / `ue` spelling in POJ mode takes TL placement on
                // purpose (`poj_mode_marks_tl_spelled_ua_ue_like_tl`).
                if !(final_str.starts_with("ua") || final_str.starts_with("ue")) {
                    cases.push((InputMode::Poj, tl));
                }
                for (mode, spelling) in cases {
                    for tone in ["2", "3", "5", "6", "7", "8", "9"] {
                        for typed in [
                            spelling.clone(),
                            spelling.to_uppercase(),
                            crate::api::capitalize_first(&spelling),
                        ] {
                            let input = format!("{typed}{tone}");
                            let expected = to_tone_marks(&input, mode);
                            if expected == input {
                                continue;
                            }
                            assert_eq!(apply(&input, mode), expected, "{mode:?}: {input}");
                            compared += 1;
                        }
                    }
                }
            }
        }
        assert!(compared > 10_000, "only {compared} syllables compared");
    }
}
