"""One CJK-ideograph test for the dictionary pipeline.

Unified Ideographs + Extensions A–G (through U+3134F) + Compatibility
Ideographs; `dictionary.csv` carries 44 characters beyond Extension E
(max U+30E6C), so every consumer must reach Extension G. The engine's
`lexicon::classification::is_hanji` still stops at Extension E — a separate
concern recorded in the bigram LM roadmap (P3), not changed here.
"""

from __future__ import annotations

CJK_RANGES: tuple[tuple[int, int], ...] = (
    (0x4E00, 0x9FFF),   # Unified Ideographs
    (0x3400, 0x4DBF),   # Extension A
    (0x20000, 0x3134F), # Extensions B–G
    (0xF900, 0xFAFF),   # Compatibility Ideographs
)


def is_cjk(char: str) -> bool:
    code = ord(char)
    return any(low <= code <= high for low, high in CJK_RANGES)
