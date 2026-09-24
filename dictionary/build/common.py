# -*- coding: utf-8 -*-
"""Shared constants + helpers for dictionary build scripts.

Each build script needs the same BASE_DIR / OUTPUT_DIR / logger / build
id, so centralise here. Scripts import from this module instead of
rebuilding the boilerplate prelude.
"""

from __future__ import annotations

import sys
import zlib
from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent.parent
OUTPUT_DIR = BASE_DIR / "output"
LOG_DIR = BASE_DIR / "logs"
MERGED_CSV = OUTPUT_DIR / "dictionary.csv"

# Build scripts live under dictionary/build/; importing common.* requires
# dictionary/ on sys.path. Each build script calls this once at import time.
if str(BASE_DIR) not in sys.path:
    sys.path.insert(0, str(BASE_DIR))


def build_id() -> int:
    """The u32 written into the `build_ts` header slot of both binaries.

    CRC-32 of the merged `output/dictionary.csv` — the one input both
    `dictionary.bin` and `association.bin` are built from — so the same
    dictionary always produces byte-identical binaries, and the two binaries
    of one build always carry the same value. It was `time.time()`, which
    made every rebuild differ from the committed artifact even when nothing
    else had changed. The engine stores the slot but never reads it.
    """
    return zlib.crc32(MERGED_CSV.read_bytes())
