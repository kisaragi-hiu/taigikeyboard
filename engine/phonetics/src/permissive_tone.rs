//! Literal tone placement without syllable validation. Tone digits close a
//! segment; the last vowel cluster in that segment carries its tone, matching
//! Taigi Telex's `TelexRules.findTonePosition` / `applyToneMark` behavior.
//! This is a preview affordance, never a dictionary spelling conversion.

use crate::api::InputMode;
use crate::normalization::is_combining_tone_mark;
use crate::tables::{poj_tone_mark, tl_tone_mark};
use unicode_normalization::{char::canonical_combining_class, UnicodeNormalization};

pub(crate) fn apply(input: &str, mode: InputMode) -> String {
    apply_recording_digits(input, mode, |_| {})
}

pub(crate) fn consumed_digits(input: &str, mode: InputMode) -> Vec<bool> {
    let mut consumed = Vec::new();
    apply_recording_digits(input, mode, |is_consumed| consumed.push(is_consumed));
    consumed
}

fn apply_recording_digits(input: &str, mode: InputMode, mut record: impl FnMut(bool)) -> String {
    let mut output = String::with_capacity(input.len());
    let mut segment = String::new();
    for ch in input.chars() {
        if matches!(ch, '1'..='9') {
            let chars: Vec<char> = segment.nfd().collect();
            let target = tone_position(&chars, mode);
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

fn tone_position(chars: &[char], mode: InputMode) -> Option<usize> {
    // Ignore tone combining marks when locating a cluster, but keep the POJ
    // dot as a boundary: o-dot before another vowel starts a separate cluster.
    let letters: Vec<(usize, char)> = chars
        .iter()
        .enumerate()
        .filter(|(_, ch)| canonical_combining_class(**ch) == 0 || **ch == '\u{0358}')
        .map(|(index, ch)| (index, ch.to_ascii_lowercase()))
        .collect();
    let mut cluster = Vec::new();
    for (pos, &(index, ch)) in letters.iter().enumerate().rev() {
        if mode == InputMode::Poj && ch == '\u{0358}' {
            if cluster.is_empty() {
                return letters
                    .get(pos.checked_sub(1)?)
                    .and_then(|&(index, ch)| (ch == 'o').then_some(index));
            }
            break;
        }
        if "aeiou".contains(ch) {
            cluster.push((index, ch));
        } else if !cluster.is_empty() {
            break;
        }
    }
    if cluster.is_empty() {
        return letters
            .iter()
            .enumerate()
            .rev()
            .find_map(|(pos, &(index, ch))| {
                (ch == 'm'
                    || (ch == 'n' && letters.get(pos + 1).is_some_and(|&(_, next)| next == 'g')))
                .then_some(index)
            });
    }
    cluster.reverse();
    let text: String = cluster.iter().map(|&(_, ch)| ch).collect();
    let end = cluster.last()?.0;
    let suffix: String = letters
        .iter()
        .filter(|&&(index, _)| index > end)
        .map(|&(_, ch)| ch)
        .collect();
    let exception = match mode {
        InputMode::Tl if matches!(text.as_str(), "iu" | "ui") => Some(1),
        InputMode::Poj if text == "oai" => Some(1),
        InputMode::Poj if text == "oa" && suffix.starts_with("ng") => Some(0),
        InputMode::Poj if text == "oa" && suffix.starts_with(['n', 't', 'h']) => Some(1),
        InputMode::Poj if text == "oe" && suffix.starts_with('h') => Some(1),
        _ => None,
    };
    if let Some(pos) = exception {
        return cluster.get(pos).map(|&(index, _)| index);
    }
    let priority = if mode == InputMode::Poj {
        "oeaui"
    } else {
        "aeoui"
    };
    priority.chars().find_map(|vowel| {
        cluster
            .iter()
            .find_map(|&(index, ch)| (ch == vowel).then_some(index))
    })
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
            ("ngm2", "ngḿ"),
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
            ("oang2", "óang"),
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
}
