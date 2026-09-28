"""`variants.csv` row reader shared by the variants stage and the association writer.

Columns: `hanzi` (教典 recommended form), `variant` (異用字 spelling), `tl`
(one reading, or several joined by `/`). One row is yielded per reading, TL
lower-cased, fields stripped; a row with a blank or missing field is skipped.
"""

from __future__ import annotations

import csv
from collections.abc import Iterator
from pathlib import Path

VARIANTS_CSV = Path(__file__).resolve().parent.parent / "supplementary" / "variants" / "data" / "variants.csv"


def read_variant_rows(path: Path = VARIANTS_CSV) -> Iterator[tuple[str, str, str]]:
    """Yield `(hanzi, variant, tl)` per reading of every row in `path`."""
    with path.open(encoding="utf-8", newline="") as f:
        for row in csv.DictReader(f):
            # A short row leaves the trailing fields as None.
            hanzi = (row["hanzi"] or "").strip()
            variant = (row["variant"] or "").strip()
            if not hanzi or not variant:
                continue
            for tl in (row["tl"] or "").split("/"):
                tl = tl.strip().lower()
                if tl:
                    yield hanzi, variant, tl
