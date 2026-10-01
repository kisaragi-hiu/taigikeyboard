# -*- coding: utf-8 -*-
"""Abbreviation helper functions."""

import unicodedata
import pandas as pd


def remove_diacritics(char: str) -> str:
    """Strip diacritics (combining marks) from a character via NFD decomposition."""
    decomposed = unicodedata.normalize("NFD", char)
    return "".join(c for c in decomposed if unicodedata.category(c) != "Mn")


# TL and POJ initials, longest first so a prefix scan takes `tsh` before
# `ts` before `t`, `chh` before `ch`, `ng` before `n`
# (`docs/phonetics/taigi-phonetics-reference.md` §2: TL `ts` / `tsh` ↔ POJ
# `ch` / `chh`, the rest shared). One table for both scripts: `ch` never
# starts a TL syllable, and a traditional-POJ `ts…` spelling reads as the
# same `ts` unit it would in TL, so the union changes no verdict. Mirror of
# `phonetics::derivation::ABBREV_INITIALS`.
ABBREV_INITIALS = (
    "tsh", "chh", "ts", "ch", "ph", "th", "kh", "ng",
    "p", "m", "b", "t", "n", "l", "k", "g", "s", "j", "h",
)


def _leading_unit(syllable: str) -> str:
    """The leading spelling unit of one bare (lowercased, diacritic-stripped)
    syllable: its initial, or its first char when it has none (zero initial).
    """
    for initial in ABBREV_INITIALS:
        if syllable.startswith(initial):
            return initial
    return syllable[:1]


def extract_abbrev(syllable_string: str) -> str:
    """Abbreviation face of a hyphen/space-separated TL or POJ reading —
    one leading spelling unit per syllable: the longest initial the
    syllable starts with (`ph`, `th`, `kh`, `tsh` / `chh`, `ng` stay whole)
    or its first letter for a zero-initial syllable. `phi-thâu-kin` →
    `phthk`, `tshut-khì` → `tshkh`, `chia̍h-pn̄g` → `chp`, `só-sî` → `ss`,
    `âng-enn-á` → `aea`. Diacritics stripped, lowercased. Returns "" for
    a single syllable.

    Same rule as TPS, whose one-glyph initials always were the whole
    initial (`ㄆㄊㄍ`), and what a typist means by `phthk` (USER
    2026-09-18). Runtime mirror: `phonetics::derive_abbrev`; parity over
    every CSV row is pinned by `engine/lexicon/tests/roman_num_face_parity.rs`.
    """
    if not syllable_string or pd.isna(syllable_string):
        return ""

    import re
    syllables = re.split(r"[-\s]+", str(syllable_string))
    syllables = [s for s in syllables if s]

    if len(syllables) < 2:
        return ""

    return "".join(_leading_unit(remove_diacritics(s).lower()) for s in syllables)


def extract_tps_abbrev(tl_syllable_string: str, tps_per_syllable: list[str]) -> str:
    """Per-syllable TPS initial-char concatenation; "" for <2 syllables.

    Mirrors `extract_abbrev` (first ASCII char per TL syllable, minus
    diacritics) for the TPS family. The first char of every TPS
    syllable is its leading initial / vowel Bopomofo glyph by
    construction (per `to_zhuyin` output shape — see
    `engine/phonetics/src/tps.rs:210`), so no diacritic stripping is
    needed; we take the first `char` directly.

    Args:
        tl_syllable_string: hyphenated TL (used only for syllable-count
            gate — at least 2 syllables to emit an abbrev).
        tps_per_syllable: list of TPS strings, one per TL syllable
            (caller already converted via `convert_tl_to_tps_strict`
            on each TL syllable). Same length as the TL split.

    Returns:
        Concatenated first-char-per-syllable TPS abbrev, or "" when the
        TL has fewer than 2 syllables OR any per-syllable TPS is empty.
    """
    if not tl_syllable_string or pd.isna(tl_syllable_string):
        return ""

    import re
    tl_syllables = re.split(r"[-\s]+", str(tl_syllable_string))
    tl_syllables = [s for s in tl_syllables if s]

    if len(tl_syllables) < 2:
        return ""
    if len(tps_per_syllable) != len(tl_syllables):
        return ""
    if any(not s for s in tps_per_syllable):
        return ""

    return "".join(s[0] for s in tps_per_syllable)
