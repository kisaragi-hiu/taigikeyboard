"""build/corpus_bigrams.py — aligner, sub-segmentation, counting, output.

Every fixture is a failure class the 2026-09-28 spike hit on real corpus text
(docs/reports/2026-09-28-bigram-corpus-spike.md § 1). The pure functions need
no node; the conversion and counting tests use the real taigi-converter
bridge, which CI has (.github/workflows/python.yml checks out the submodule +
node 22).

Run from `dictionary/`:
    PYTHONPATH=. python3 -m pytest tests/test_corpus_bigrams.py -q
"""

from __future__ import annotations

import hashlib
import sys
import tempfile
import unittest
from collections import Counter
from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(BASE_DIR))

from build import corpus_bigrams as cb
from build.corpus_bigrams import Token
from common.romanization import to_numeric_tone

TL_BY_WORD = {
    ("講", "kong2"): "kóng",
    ("講--的", "kong2e5"): "kóng--ê",
    ("的", "e5"): "ê",
    ("家己", "ka1ki7"): "ka-kī",
    ("新聞", "sin1bun5"): "sin-bûn",
    ("新聞報", "sin1bun5po3"): "sin-bûn-pò",
    ("報導", "po3to7"): "pò-tō",
    ("導", "to7"): "tō",
    ("報", "po3"): "pò",
    ("民視", "bin5si7"): "bîn-sī",
    ("歹", "phainn2"): "pháinn",
    ("歹人", "phainn2lang5"): "pháinn-lâng",
    ("人", "lang5"): "lâng",
    ("佇", "ti7"): "tī",
    ("我", "gua2"): "guá",
    ("好", "ho2"): "hó",
}
LEXICON = cb.Lexicon(
    tl_by_word=TL_BY_WORD,
    frequency={("新聞", "sin1bun5"): 500, ("報導", "po3to7"): 400, ("新聞報", "sin1bun5po3"): 30, ("導", "to7"): 10},
    hanji_by_romanized_reading={"ti7": "佇"},
)


def kinds(tokens: list[Token]) -> list[str]:
    return [token.kind for token in tokens]


class AlignerTests(unittest.TestCase):
    def test_hanji_word_per_syllable_and_sentence_end_from_text_side(self):
        tokens = cb.align_unit("紅嬰仔哭甲一身軀汗。", "Âng-enn-á khàu kah tsi̍t sin-khu kuānn.")
        self.assertEqual([t.hanji for t in tokens if t.kind == "word"], ["紅嬰仔", "哭", "甲", "一", "身軀", "汗"])
        self.assertEqual(tokens[-1].kind, "end")

    def test_khinsiann_mirrored_into_hanji(self):
        tokens = cb.align_unit("伊講的", "i kóng--ê")
        self.assertEqual(tokens[1], Token("word", "kóng--ê", "講--的"))

    def test_standalone_khinsiann_token_attaches_to_previous_word(self):
        self.assertEqual(cb.tl_tokens("kàu --ah"), [Token("latin", "kàu--ah")])

    def test_standalone_and_leading_punctuation_are_breaks(self):
        self.assertEqual(kinds(cb.tl_tokens("hó ， guá")), ["latin", "break", "latin"])
        self.assertEqual(kinds(cb.tl_tokens("「Lín ná")), ["break", "latin", "latin"])
        self.assertEqual(kinds(cb.align_unit("「你哪」", "「Lí ná」")), ["word", "word", "break"])

    def test_mixed_word_romanized_and_han_syllables(self):
        tokens = cb.align_unit("將pháiⁿ人hiat入去", "chiong pháiⁿ-lâng hiat ji̍p-khì")
        self.assertEqual(tokens[0], Token("word", "chiong", "將"))
        self.assertEqual(tokens[1], Token("mixed", "pháiⁿ-lâng", "pháiⁿ人"))

    def test_hyphenated_romanized_run_stays_one_word(self):
        tokens = cb.align_unit("M7-thang hian2 來", "M7-thang hian2 lai5")
        self.assertEqual(kinds(tokens), ["latin", "latin", "word"])

    def test_presegmented_news_line_aligns_word_by_word(self):
        tokens = cb.align_unit("Obama 大勝 美國 ，", "Obama tua7-sing3 bi2-kok4 ，")
        self.assertEqual(kinds(tokens), ["latin", "word", "word", "break"])
        self.assertEqual(tokens[1].hanji, "大勝")

    def test_latin_run_mismatch_raises(self):
        with self.assertRaises(cb.AlignError):
            cb.align_unit("伊kap我", "i kah guá")

    def test_char_count_mismatch_raises(self):
        with self.assertRaises(cb.AlignError):
            cb.align_unit("伊講", "i kóng ê")

    def test_leftover_hanji_raises(self):
        with self.assertRaises(cb.AlignError):
            cb.align_unit("伊講的", "i kóng")

    def test_digits_consumed_on_both_sides(self):
        tokens = cb.align_unit("2.活動e5時間", "2. Oah8-tong7 e5 si5-kan")
        self.assertEqual(kinds(tokens), ["digit", "break", "word", "latin", "word"])

    def test_clause_punctuation_is_a_break_not_an_end(self):
        tokens = cb.align_unit("好，我去", "hó, guá khì")
        self.assertEqual(kinds(tokens), ["word", "break", "word", "word"])

    def test_text_side_period_ends_even_when_romanization_uses_comma(self):
        tokens = cb.align_unit("好。我去", "hó, guá khì")
        self.assertEqual(kinds(tokens), ["word", "end", "word", "word"])

    def test_romanization_period_without_text_punctuation_is_a_break(self):
        self.assertEqual(kinds(cb.align_unit("好 我去", "hó. guá khì")), ["word", "break", "word", "word"])

    def test_split_sentences_keeps_delimiter(self):
        self.assertEqual(cb.split_sentences("好。你呢？走"), ["好。", "你呢？", "走"])


class SubsegmentTests(unittest.TestCase):
    def test_fewest_pieces_then_frequency_not_greedy(self):
        # greedy longest-match gives 新聞報 + 導; DP prefers 新聞 + 報導 (same piece count, higher freq)
        pieces = cb.subsegment("新聞報導", ["sin1", "bun5", "po3", "to7"], LEXICON)
        self.assertEqual(pieces, [("新聞", "sin1bun5"), ("報導", "po3to7")])

    def test_returns_none_when_a_char_is_unknown(self):
        self.assertIsNone(cb.subsegment("新聞報導X", ["sin1", "bun5", "po3", "to7", "x"], LEXICON))

    def test_single_char_is_not_subsegmented(self):
        self.assertIsNone(cb.subsegment("導", ["to7"], LEXICON))


class ClassifyTests(unittest.TestCase):
    def test_words_readings_and_oov(self):
        tokens = [
            Token("word", "guá", "我"),
            Token("latin", "tī"),
            Token("word", "ka-kī", "家己"),
            Token("latin", "xyz"),
            Token("end"),
        ]
        stat = Counter()
        items = cb.classify(tokens, ["gua2", "ti7", "ka1ki7", "xyz1"], LEXICON, stat)
        self.assertEqual(items, [("我", "gua2"), ("佇", "ti7"), ("家己", "ka1ki7"), None, cb.START_WORD])
        self.assertEqual(stat["tok:romanized-mapped"], 1)
        self.assertEqual(stat["tok:romanized-oov"], 1)

    def test_oov_word_is_subsegmented_into_dictionary_words(self):
        stat = Counter()
        items = cb.classify([Token("word", "sin1-bun5-po3-to7", "新聞報導")], ["sin1-bun5-po3-to7"], LEXICON, stat)
        self.assertEqual(items, [("新聞", "sin1bun5"), ("報導", "po3to7")])
        self.assertEqual(stat["tok:subsegmented"], 1)


class CountSourceTests(unittest.TestCase):
    """Readings are numeric TL already, so the bridge is an identity pass (node required)."""

    def test_chain_breaks_and_sentence_start(self):
        # $ 我 好 | (clause) 人 。 $ 好
        counts = cb.Counts()
        cb.count_source("t", [cb.Unit("我好，人。好", "gua2 ho2, lang5. ho2", "tl")], LEXICON, counts)
        self.assertEqual(
            {pair: dict(sources) for pair, sources in counts.bigrams.items()},
            {
                (cb.START_WORD, ("我", "gua2")): {"t": 1},
                (("我", "gua2"), ("好", "ho2")): {"t": 1},
                (cb.START_WORD, ("好", "ho2")): {"t": 1},
            },
        )
        self.assertEqual(counts.unigrams[("好", "ho2")]["t"], 2)
        self.assertEqual(counts.stats["t"]["aligned"], 1)

    def test_identical_sentences_are_capped(self):
        counts = cb.Counts()
        cb.count_source("t", [cb.Unit("好", "ho2", "tl")] * (cb.SENTENCE_CAP + 2), LEXICON, counts)
        self.assertEqual(counts.unigrams[("好", "ho2")]["t"], cb.SENTENCE_CAP)
        self.assertEqual(counts.stats["t"]["units:capped"], 2)


class OutputTests(unittest.TestCase):
    def _counts(self):
        counts = cb.Counts()
        a, b, c = ("我", "gua2"), ("好", "ho2"), ("人", "lang5")
        counts.bigrams[(a, b)].update({"s1": 2, "s2": 1})
        counts.bigrams[(a, c)]["s1"] = 1  # below MIN_PAIR_COUNT → dropped
        counts.bigrams[(cb.START_WORD, a)]["s2"] = 3
        counts.unigrams[a]["s1"] = 3
        counts.unigrams[b]["s2"] = 1
        return counts

    def test_rows_sorted_min_count_and_per_source_columns(self):
        with tempfile.TemporaryDirectory() as d:
            bigrams, unigrams = Path(d) / "b.tsv", Path(d) / "u.tsv"
            written = cb.write_outputs(self._counts(), LEXICON, bigrams, unigrams, source_names=["s2", "s1"])
            self.assertEqual(written, (2, 2))
            self.assertEqual(
                bigrams.read_text(encoding="utf-8"),
                "prev_hanji\tprev_tl\tnext_hanji\tnext_tl\tcount\ts1\ts2\n$\t\t我\tguá\t3\t0\t3\n我\tguá\t好\thó\t3\t2\t1\n",
            )
            self.assertEqual(
                unigrams.read_text(encoding="utf-8"),
                "hanji\ttl\tcount\ts1\ts2\n好\thó\t1\t0\t1\n我\tguá\t3\t3\t0\n",
            )

    def test_output_is_byte_stable_across_runs(self):
        with tempfile.TemporaryDirectory() as d:
            digests = set()
            for _ in range(2):
                bigrams, unigrams = Path(d) / "b.tsv", Path(d) / "u.tsv"
                cb.write_outputs(self._counts(), LEXICON, bigrams, unigrams, source_names=["s1", "s2"])
                digests.add(hashlib.sha256(bigrams.read_bytes() + unigrams.read_bytes()).hexdigest())
            self.assertEqual(len(digests), 1)


class InputTests(unittest.TestCase):
    SAMPLE_JS = """// 文章資料庫
const articles = [
  {
    id: 1,
    type: "mapped", // 漢羅對應
    tags: ["詩"],
    new: false,
    hanji: `老硞硞鱷魚
第二行`,
    tailo: `Lāu-khok-khok kho̍k-hî
tē-jī hâng`,
  },
  { id: 2, type: "single", content: `純漢字` },
];
"""

    def test_articles_js_object_literal_subset(self):
        articles = cb.js_array_to_json(self.SAMPLE_JS)
        self.assertEqual([a["id"] for a in articles], [1, 2])
        self.assertEqual(articles[0]["hanji"].split("\n"), ["老硞硞鱷魚", "第二行"])
        self.assertEqual(articles[0]["new"], False)

    def test_articles_js_rejects_template_interpolation(self):
        with self.assertRaises(ValueError):
            cb.js_array_to_json("const a = [{ x: `${y}` }];")

    def test_articles_js_rejects_unknown_syntax(self):
        with self.assertRaises(ValueError):
            cb.js_array_to_json("const a = [{ x: 'single-quoted' }];")

    def test_missing_corpus_file_fails_with_submodule_hint(self):
        saved = cb.CORPUS_DIR
        cb.CORPUS_DIR = Path(tempfile.mkdtemp())
        try:
            with self.assertRaises(FileNotFoundError) as raised:
                next(cb.SOURCES["moe_kautian"]())
        finally:
            cb.CORPUS_DIR = saved
        self.assertIn("git submodule update --init --checkout corpus/taigi-corpus", str(raised.exception))

    def test_mismatched_line_counts_fail_loud(self):
        with self.assertRaises(ValueError):
            list(cb.paired_lines("a\nb", "a", "doc-1"))

    def test_reading_table_rows_must_be_dictionary_words(self):
        with tempfile.TemporaryDirectory() as d:
            readings = Path(d) / "readings.tsv"
            readings.write_text("tl_num\thanji\nzzz9\t無\n", encoding="utf-8")
            with self.assertRaises(ValueError):
                cb.load_lexicon(cb.MERGED_CSV, readings)


class ConverterBridgeTests(unittest.TestCase):
    """Real taigi-converter via common/taigi_bridge (node + submodule required)."""

    def test_numeric_poj_nasal_capital_n_becomes_nn(self):
        numeric = cb.convert_readings(["chiaN2", "khoaN3", "ou-lang5"], "poj")
        self.assertEqual([cb.key_of(n) for n in numeric], ["tsiann2", "khuann3", "oo1lang5"])

    def test_tl_diacritics_to_numeric_keys(self):
        numeric = cb.convert_readings(["Âng-enn-á", "kuānn", "it--lâi"], "tl")
        self.assertEqual([cb.key_of(n) for n in numeric], ["ang5enn1a2", "kuann7", "it4lai5"])

    def test_key_of_matches_the_dictionary_tl_num_rule(self):
        for word in ("Âng-enn-á", "tsi̍t", "kah", "it--lâi", "m̄-bat"):
            numeric = cb.convert_readings([word], "tl")[0]
            self.assertEqual(cb.key_of(numeric), to_numeric_tone(word, ascii_only=True), word)


if __name__ == "__main__":
    unittest.main()
