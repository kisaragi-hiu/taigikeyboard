"""One CJK-ideograph test for the dictionary pipeline.

Unified Ideographs + Extensions A–G (through U+3134F) + Compatibility
Ideographs; `dictionary.csv` carries 44 rows beyond Extension E (max
U+30E6C), so every consumer must reach Extension G. `0x20000–0x3134F` also
spans Extension I, the Compatibility Ideographs Supplement and the unassigned
gaps; Extension H (U+31350) and later stay out. The engine's
`lexicon::classification::CJK_RANGES` is the same table — its
`cjk_ranges_parity` test parses `CJK_RANGES` below, so keep it one
`(0xLOW, 0xHIGH),` tuple per line.
"""

from __future__ import annotations

CJK_RANGES: tuple[tuple[int, int], ...] = (
    (0x4E00, 0x9FFF),   # Unified Ideographs
    (0x3400, 0x4DBF),   # Extension A
    (0x20000, 0x3134F), # Extensions B–G, I + Compatibility Supplement
    (0xF900, 0xFAFF),   # Compatibility Ideographs
)


def is_cjk(char: str) -> bool:
    code = ord(char)
    return any(low <= code <= high for low, high in CJK_RANGES)
