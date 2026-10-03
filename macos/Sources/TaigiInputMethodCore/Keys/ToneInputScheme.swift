// Which keys type a tone — and, by the same choice, which keys pick a candidate.

/// Which keys type a tone — the half of the key contract that decides, in
/// turn, which keys pick a candidate.
///
/// Standard and Telex are the same eight letters and the same nine digits,
/// swapped (USER 2026-09-08). Under `standard` a digit is the TL/POJ tone
/// marker (`tai5`), so the letters no syllable spells are free to pick
/// candidates; under `telex` those letters carry the tones, so the digits
/// are free to pick. Neither half is chosen on its own: the slot key set is
/// derived here (`slotKeySet`) rather than stored, so the two can never
/// disagree about who owns `v` or `3`.
///
/// The letter table is the kahiok scheme (madmaxieee/taigi-telex) minus its
/// `c` → `tsh` key, because POJ spells `ch` / `chh` with `c`. Tone 6 has no
/// free letter and is not offered. The semantics — which tone each key
/// writes, how `z` resolves by input mode, what a repeated key does — live in
/// the engine (`engine/composing/src/telex.rs`); this side only decides which
/// keys reach it.
enum ToneInputScheme: String, CaseIterable, Sendable {
    /// Digits type tones; `q w d f z x v y ;` pick candidates. The shipped
    /// default, and today's behaviour before the scheme existed.
    case standard
    /// Letters type tones; `1`…`9` pick candidates.
    case telex

    /// The keys that pick a candidate under this scheme.
    var slotKeySet: CandidateSlotKeySet {
        self == .telex ? .digits : .bareKeys
    }
}
