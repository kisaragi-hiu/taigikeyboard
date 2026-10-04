//! Hanji conversion with no lexicon installed: nothing to walk, so the
//! preedit stays the glyphs and the state holds no conversion.

use composing::{Engine, Intent, Phase};

use crate::common::config_converting;

#[test]
fn without_a_lexicon_the_tail_stays_glyphs() {
    let config = config_converting("tps");
    let mut engine = Engine::new();
    let response = engine.apply(
        Intent::Start {
            text: "ㄒㄧˋ".into(),
        },
        &config,
    );
    assert_eq!(response.preedit.expect("preedit").display_text, "ㄒㄧˋ");
    assert!(matches!(
        engine.snapshot_state().phase,
        Phase::Continuous {
            conversion: None,
            ..
        }
    ));
}
