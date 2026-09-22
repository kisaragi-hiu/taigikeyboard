#!/usr/bin/env python3
r"""Random dogfood sentences from corpus/taigi-typing.

Usage: pick.py [pattern] [count]
  pattern = regex (case-insensitive) matched against the TL *and* the hanji:
            --lai · siong- · 放重利 · (\w+)-\1 (reduplication) · nn|ⁿ
Prints: hanji / tailo / keys (diacritics stripped, hyphens kept) / dic_url
"""
import json, random, re, sys, unicodedata
from pathlib import Path

SRC = Path(__file__).resolve().parents[3] / "corpus/taigi-typing/exampleSentences.js"
pattern = sys.argv[1] if len(sys.argv) > 1 else ""
count = int(sys.argv[2]) if len(sys.argv) > 2 else 5

raw = SRC.read_text(encoding="utf-8")
body = raw[raw.index("[") : raw.rindex("]") + 1]
rows = json.loads(re.sub(r",(\s*[\]}])", r"\1", body))
rows = [s for group in rows for s in group]
rx = re.compile(pattern, re.I)
hits = [s for s in rows if rx.search(s["tailo"]) or rx.search(s["hanji"])]
if not hits:
    sys.exit(f"no sentence matches {pattern!r} — widen it")

def keys(tl):
    bare = "".join(c for c in unicodedata.normalize("NFD", tl) if not unicodedata.combining(c))
    bare = bare.replace("o͘", "oo").replace("ⁿ", "nn")
    return re.sub(r"[^A-Za-z\- ]", "", bare).lower()

for s in random.sample(hits, min(count, len(hits))):
    print(f'{s["hanji"]}\n  {s["tailo"]}\n  keys: {keys(s["tailo"])}\n  {s["dic_url"]}\n')
print(f"({len(hits)} of {len(rows)} sentences match {pattern!r})")
