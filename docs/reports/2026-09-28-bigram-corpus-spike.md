# Bigram corpus spike — P1 (read-only)

> **Type**: Report (dated snapshot, frozen)
> **Date**: 2026-09-28
> **Plan**: `docs/architecture/bigram-lm-roadmap.md` phase P1; opens the P2 decision
> **Method**: scratchpad scripts only (no branch, no product code); corpus = `corpus/taigi-corpus` @ `5ba9509` (after PR #1); dictionary = `dictionary/output/dictionary.csv` (168 451 `(hanzi, tl)` rows); every romanization → TL went through `taigi-converter` (`convert` + `toToneNumberAscii`)

## Summary (recommendation, not a decision — opening P2 is the USER's call)

Eight aligned sources yield 2.32 M cross-word pair tokens and 260 k distinct `(w₁ → w₂)` pairs seen at least twice. Of the dictionary's 168 451 `(hanzi, tl)` rows, 41 489 (24.6 %) get at least one continuation and 21 765 (12.9 %) at least one seen twice; weighted by dictionary frequency that is 90.5 % of the top 1 000 words and 78 % of the top 5 000. The projected `association.bin` v2 word-key section is **2.81 MB** at K = 30 / min count 2 (breakdown in § 2), beside today's 3.3 MB character table. Sentence-start (`$`) has 19.8 k distinct openers. Top-K truncation keeps 50 % of the pair-token mass at K = 30 / min 2 (§ 2) — the tail is long, as expected for a 2.3 M-token corpus.

My reading: the numbers support opening P2 for the next-word track (the top-frequency words the strip shows are covered); they do not by themselves say anything about the composing track (P5/P6), which needs the per-span candidate data P2 produces. Codex (ANALYSIS-ONLY, 2026-09-28) read the same numbers as "worth evaluating P2", and asked for the dictionary denominator and the size breakdown now included.

Three things the roadmap did not spell out, each measured below, are offered as P2 design inputs (§ 8): mirror `--` into the hanji key, split coarse corpus "words" with the dictionary (§ 1), and how romanized function words in Han-Lo text are recovered (§ 4). One finding outside this plan's scope: the dictionary `frequency` column carries default fills that tie (§ 6).

## 1. Per-source alignment

Token identity = `(hanji, numeric ASCII TL)` = dictionary `tl_num` (hyphens dropped, tones explicit). "In-vocab" = token found in `dictionary.csv`; an OOV token breaks the pair chain.

| Source | Units | Aligned | Dropped | Aligned % | Han tokens | In-vocab % | OOV rate % | OOV word | Reading mismatch | Mixed Han/roman | Romanized, unrecovered | Sentences | Distinct pairs | Pair tokens |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| icorpus_hanji | 64 110 lines | 64 093 | 17 | 100.0 | 413 414 | 98.2 | 1.8 | 1 655 | 4 538 | 0 | 1 142 | 64 170 | 198 867 | 336 066 |
| moe_kautian 例句 | 17 906 lines | 17 900 | 6 | 100.0 | 110 058 | 95.6 | 4.4 | 4 611 | 219 | 2 | 7 | 18 029 | 43 347 | 78 721 |
| sinpak_900leku | 821 lines | 765 | 56 | 93.2 | 5 624 | 94.5 | 5.5 | 271 | 39 | 0 | 0 | 772 | 3 221 | 4 157 |
| kok4hau7 | 1 197 lines | 828 | 369 | 69.2 | 6 687 | 95.7 | 4.3 | 161 | 128 | 0 | 0 | 888 | 3 768 | 4 605 |
| taigi_bible_nt | 23 822 verses | 22 551 | 1 271 | 94.7 | 335 593 | 67.6 | 32.4 | 2 035 | 3 371 | 14 277 | 138 783 | 24 650 | 50 886 | 172 721 |
| kipsupin_2009 | 63 844 sentences* | 53 269 | 10 575 | 83.4 | 938 834 | 88.6 | 11.4 | 16 877 | 26 225 | 9 349 | 63 353 | 55 577 | 291 082 | 648 940 |
| nmtl_dadwt | 59 844 sentences* | 47 993 | 11 851 | 80.2 | 893 301 | 74.4 | 25.6 | 6 534 | 13 038 | 39 136 | 242 430 | 50 462 | 209 401 | 516 946 |
| khinhoan_pojbh (aligned tag) | 38 926 sentences* | 27 037 | 11 889 | 69.5 | 753 935 | 90.6 | 9.4 | 4 537 | 12 023 | 8 519 | 51 000 | 29 255 | 197 017 | 556 182 |
| **Total** | 270 470 | 234 436 | 36 034 | 86.7 | **3.46 M** | | | | | | | **243 803** | **782 931** | **2 318 338** |

\* paragraph pairs were split into sentences on `。！？!?` when both sides split into the same count; otherwise the paragraph was aligned whole (48 778 paragraphs). "Dropped" = units the aligner rejected (a rejected paragraph counts once). Failure classes over all sources: Latin-run mismatch 16 785, hanji/syllable count 11 968, hanlo leftover 7 264. OOV rate = 100 − in-vocab % over all tokens on the TL side (Han words, mixed words and romanized words); the OOV columns break it down.

Reading of the columns:

- **icorpus_hanji is the anchor**: word-aligned upstream, 98 % in-vocab after sub-segmentation, one third of all pair tokens.
- **Bible / nmtl in-vocab is low because their Han-Lo writes function words in romanization** (`ê`, `kap`, `tī`, `hō͘`, `in`…): 138 k and 242 k romanized tokens are not `(hanji, TL)` tokens and break the chain. § 4 measures the recovery options.
- **Reading mismatch** = the hanji word exists in the dictionary with a different reading. Two thirds are orthography-era conventions (POJ `tio̍h` written tone 8 where the dictionary has tone 7 `就是/tio7si7`; `個 kò͘` vs `kò`; `一個月 koo3` vs `ko3`); the rest are real dialect readings (`足濟 tsue7` vs `tse7`, `這陣 tin7` vs `tsun7`). Not recoverable by rule; left OOV.
- **Mixed** = one word written half in hanji, half in romanization (`pháiⁿ人`, `to3-tng2來`). Aligned correctly (the aligner works per syllable) but has no hanji form → OOV.

### What the aligner had to handle (all found by running, none assumed)

| Problem | Where | Fix in the spike | Effect |
|---|---|---|---|
| Dictionary hanzi carries `--` where TL has khin-siann (`講--的`, `來--矣`); corpus hanji has none | every source | insert `--` into the hanji at the TL's `--` position | moe OOV 5 186 → 4 611; `講的`, `來矣`, `好矣` now in-vocab |
| Upstream iCorpus "words" are coarser than the dictionary (`家己的`, `一名`, `民視新聞報導`) | icorpus_hanji | greedy longest-match split of an OOV word into dictionary words, syllable-aligned | OOV 92 803 → 1 655; 91 148 tokens recovered; pair tokens 111 k → 336 k |
| Han-Lo romanizes part of a word (`pháiⁿ人` = `pháiⁿ-lâng`) | Bible, nmtl, kipsupin | consume TL syllables one by one against hanji chars **or** Latin syllables | Bible aligned 7 290 → 22 551 verses |
| POJ side ends sentences with `.`; hanji side with `。` — sentence counts differ | nmtl, khinhoan, kipsupin | fall back to aligning the whole paragraph | nmtl aligned 25 467 → 47 993 |
| Numeric POJ writes nasal as capital `N` (`chiaN2`) | icorpus, nmtl, ungian | `N` → `nn` before `taigi-converter` | converter alone gives `tsian2`, should be `tsiann2` — **candidate fix for taigi-converter**, USER decides |

Greedy longest-match has a visible artifact: `民視新聞報導` → `民視 + 新聞報 + 導` (the pair `新聞報 → 導` has count 1 899). P2 should split with a frequency-aware DP (the engine walker's own `edge_cost` is the natural reuse) rather than greedy, or drop tokens whose split contains a single-character remainder.

## 2. Pair statistics (all sources, strict recovery)

| Measure | Value |
|---|---:|
| Pair tokens (cross-word, in-vocab both sides) | 2 318 338 |
| Distinct pairs | 782 931 |
| Distinct pairs with count ≥ 2 / ≥ 5 | 260 062 / 68 277 |
| Count histogram (1 / 2 / 3 / 4 / 5–9 / ≥ 10) | 522 869 / 120 525 / 47 783 / 23 477 / 39 607 / 28 670 |
| Distinct `w₁` with ≥ 1 continuation / with ≥ 1 continuation of count ≥ 2 | 41 489 / 21 765 |
| Unigram types (in-vocab) | 52 223 |
| `$` sentence-start distinct openers | 19 828 |
| Pairs seen in ≥ 2 sources | 143 051 |
| Distinct trigrams / with count ≥ 2 | 1 138 420 / 198 586 |

Top-K coverage = share of pair-token mass kept when each `w₁` keeps only its K most frequent continuations with count ≥ min:

| K | min 1 | min 2 | min 3 |
|---|---:|---:|---:|
| 10 | 41.7 % | 37.0 % | 34.1 % |
| 30 | 58.3 % | 50.1 % | 45.4 % |
| 60 | 68.4 % | 57.4 % | 51.5 % |

Dictionary coverage: 41 489 of 168 451 `(hanzi, tl)` rows (24.6 %) have ≥ 1 continuation; 21 765 (12.9 %) have one with count ≥ 2. By dictionary frequency rank: top 1 000 = **90.5 %**, top 5 000 = **78.1 %**, top 20 000 = 31.8 %. Of the 5 662 single-character keys in today's `association.bin`, 4 099 occur as a single-character word with a continuation.

Top pairs: `一/tsi̍t → 個/ê` 5 397 (POJ-era hanji 個 for the classifier; the dictionary has both 個/ê and 个/ê — P2 must fold variant hanji per `is_variant`/kautian recommended form), `有/ū → 一/tsi̍t` 4 331, `伊/i → 的/ê` 3 327, `的/ê → 人/lâng` 3 187, `伊/i → 講/kóng` 3 002, `攏/lóng → 無/bô` 2 298, `若/nā → 無/bô` 1 830, `這/tsit → 款/khuán` 1 673. Top `$` openers: 我 6 639, 伊 5 849, 你 3 625, 有 3 100, 因為 2 646, 這 2 641, 民視 2 033 (news boilerplate), 耶穌 1 937 (Bible), 佇 1 831, 咱 1 586.

### Projected `association.bin` v2 (word keys `hanji\u{1}tl` + `$`, entry layout unchanged)

Computed from the actual UTF-8 lengths of every kept key and entry (TL taken from the dictionary's diacritic `tl` column), using the TKWA v1 layout (`create_association_bin.py`): header 20 B; offset table 4 B per key; key = 1 length byte + `hanji\u{1}tl` bytes + 4 B entry offset + 2 B entry count; entry = 2 B bitmask + 4 B count + 2 length bytes + hanji bytes + tl bytes. The `$` key keeps its 30 most frequent openers, priced at the average entry size.

| Component (K = 30, min count 2, strict recovery) | Count | Bytes |
|---|---:|---:|
| Header | 1 | 20 |
| Offset table | 21 766 keys (21 765 word keys + `$`) | 87 064 |
| Key section | 21 766 | 486 171 |
| Entry section | 127 352 entries (127 322 + 30 for `$`) | 2 237 761 |
| — of which fixed 8 B per entry | 127 322 | 1 018 576 |
| — hanji bytes | | 505 265 |
| — tl bytes | | 713 393 |
| **Total** | | **2 811 016 B = 2.81 MB** (avg entry 17.6 B) |

With the curated function-word recovery of § 4 the same computation gives 23 613 word keys, 141 408 entries, 3.06 MB. Plus the existing 3.3 MB character section → ~6 MB mmapped file. No effect on the iOS 64 MB extension budget (mmap, not heap).

## 3. Hand check (10 random pairs per source, count ≥ 2)

Every pair below was read by hand; wrong ones are marked.

- icorpus_hanji: 教會→信徒, 錢→是, 人→載, 拍→車窗, 風情→咖啡館, 到→中晝, 的→鐘, 病毒→的, 這个→學校, 食→街頭 — all plausible news text. (Boilerplate `新聞報→導` is a greedy-split artifact, see § 1.)
- moe_kautian: 閣→敢, 外口→較, 伊→萬項, 直直→去, 三→斗, 成→賊, 閣→予, 喙齒→會, 你→去 (57), 規日→無 — all correct.
- sinpak: 人→咧, 揣→伊, 按怎→做, 佮→你, 有→咧, 𪜶→兜, 軀→衫, 就→是, 的→代誌, 代誌→愛 — correct; `軀/su → 衫` is the classifier split from `一軀衫`.
- kok4hau7: 真→好耍, 的→聲音, 咬→一, 攏→講, 猶→有, 大人→囡仔, 的→所在, 我→嘛, 貓→貓, 咱→台灣 — correct.
- Bible: 伊→按手 (10), 先知→起, 塊→寫字, 你→本, 先知→起來, 宇宙→是, 知→本身, 祭司→該, 世間人→有, 所以→上帝 — correct but register-bound (按手, 祭司, 上帝).
- kipsupin: 啊→您, 暗→傷悲 (11), 別→族, 枯→不, 來→弄, 媠→醜, 相→放, 分→我, 好→若, 三→聲 (34) — correct; 褒歌/歌仔冊 verse gives 暗→傷悲, 三→聲.
- nmtl: 一生→信, 那→離開, 著→雄雄, 張持→著, 因為→救主, 咱→用, 報→互 (POJ-era 互 for hōo), 見證→講, 受→聖神, 分→幾若 — correct; 互/hōo and 予/hōo are the same word in two hanji → P2 variant fold.
- khinhoan: 心肝→清氣, 天國→的 (30), 海→的, 欲→合, 案內→來, 工→攏, 咱→這霎, 感→趣味, 準備→通, 的→功勞 — correct.

No wrong pair in the 80 sampled; the systematic issues are boilerplate (news, scripture) and hanji variants, not alignment errors.

## 4. Romanized function words in Han-Lo text (P2 design input)

In Bible, nmtl, kipsupin and khinhoan the commonest words are written in romanization inside hanji text. Top keys across nmtl + Bible: `ê` 109 k, `tī` 20 k, `in` 14 k, `hō͘` 14 k, `kap` 12 k, `beh` 11 k, `lín` 11 k, `koh` 10 k, `kā` 10 k, `teh` 8 k, `tuì` 7 k, `hit` 7 k, `thang` 6 k, `tio̍h` 5 k, `án-ni` 5 k, `m̄` 4 k. Each breaks the chain twice.

Three options measured:

| Option | Pair tokens | Distinct ≥ 2 | Notes |
|---|---:|---:|---|
| Strict: recover only when the reading maps to exactly one dictionary hanzi | 2 318 338 | 260 062 | 4 401 + 1 037 + 1 126 + 1 032 tokens recovered — almost nothing, because every common reading has several hanzi rows |
| Best dictionary frequency per reading | 3 037 600 (+31 %) | 306 016 | **Unsafe**: frequency ties resolve arbitrarily — `kap` → 鴿/甲/洽 (佮 not first), `teh` → 塊/在/咧, `beh` → 要/欲/卜, `kā` → 給/共; `ê` → 的 mislabels the classifier 个 (`一 ê` → 一的). Not to be used. |
| Curated table for the top ~30 readings — **draft, every row still to be checked against `knowledge/taigi-phonetics-reference.md` and the 教典 recommended forms per `.claude/rules/phonetics.md` before use** (`tī`→佇, `in`→𪜶, `hō͘`→予, `kap`→佮, `beh`→欲, `lín`→恁, `koh`→閣, `kā`→共, `teh`→咧, `tuì`→對, `hit`→彼, `thang`→通, `tio̍h`→著, `án-ni`→按呢, `m̄`→毋, `bē/buē`→袂, `i`→伊, `ài`→愛, `lóng`→攏, `tsit`→這, `ah`→矣, `hia`→遐, `lah`→啦, `mā`→嘛, `bat`→捌, `m̄-thang`→毋通, `tuà`→蹛) | between the two (not run; ≤ +31 %) | | My recommendation for P2: deterministic and reviewable. `ê` has no single hanji (的 / 个) and stays a chain break under this option |

## 5. P7 input — dictionary reading prior vs gold (iCorpus hanji, 98 462 polyphonic tokens, 5 535 words)

For a hanji word with several dictionary readings, which reading does a rule pick, versus the human-corrected reading?

| Prior | Accuracy on polyphonic tokens |
|---|---:|
| Highest `frequency` (what `associations.py` effectively uses today) | 57.9 % |
| `kautian_main` first, then frequency | 72.5 % |
| `kautian_main`, then `taihoa`, then frequency | 82.3 % |
| **`taihoa`, then `kautian_main`, then frequency** | **83.0 %** |
| Corpus majority per word (oracle = what P2's own counts give) | 96.2 % |

Monophonic words are 100 % by construction (105 447 tokens); 92 803 tokens were not dictionary words before sub-segmentation. Worst cases under the frequency-only prior: 佮 kah/kap (1 437), 和 hōo/hâm (729), 警方 kìng/kíng (706), 會使 uē/ē-sái (581) — ties and variant rows, see § 6. **What this says for P7**: on this iCorpus subset a dictionary rule reaches 83 % and the corpus majority 96 % for words the corpus has seen. It does not measure an LLM at all; whether the LLM spike from the 2026-09-28 discussion is run, and when, stays the USER's call — these numbers only show where a rule already suffices.

## 6. Findings outside this plan (not acted on)

1. **`dictionary.csv` `frequency` ties**: many rows carry default fills (25 / 50 / 152 / 699…) shared by main and variant readings (`會使 uē-sái 699 = ē-sái 699`, `中國 ting-kok 152 = tiong-kok 152`, `警方 25 = 25`), and some variants outrank the main reading (`佮 kah 26 700 > kap 2 668`). Paths that read this column and could be touched by ties: candidate sort (`engine/ranking/src/score.rs`, `engine/lexicon/src/continuous/sort_key.rs`), the walker's `edge_cost` (`engine/composing/src/lattice/cost.rs`), and `dictionary/build/associations.py` counts. **Whether a tie changes any visible candidate order was not measured here**; this report only shows the ties exist and that they mislead a "most frequent reading" rule (§ 5). Not this plan's scope; recorded for the USER.
2. **taigi-converter**: numeric POJ nasal `N` (`chiaN2`) is not recognised; `nn` is. One-line candidate fix; USER decides.
3. Hanji variants for one word (個/个, 互/予, 着/著) are separate dictionary rows; the bigram table will split their counts unless P2 folds them (the dictionary has `is_variant` and kautian recommended forms to fold to).

## 7. 意傳 `拆文分析器` comparison (D2 "reuse before writing")

Installed `tai5-uan5_gian5-gi2_kang1-ku7` in a scratch venv; `拆文分析器.建立句物件(漢字, 台羅)` was run beside the spike aligner on random samples.

| Source (sample) | Both succeed | Boundaries agree | 意傳 only | Spike only |
|---|---:|---:|---:|---:|
| moe_kautian (3 000) | 2 997 | 2 362 (79 %) | 3 | 0 |
| sinpak (821) | 758 | 517 | 0 | 7 |
| kipsupin (3 000) | 2 513 | 2 239 (89 %) | 290 | 15 |
| khinhoan (2 000) | 1 303 | 973 | 579 | 35 |

Every disagreement inspected was one of two kinds: 意傳 keeps `--` inside the word form (`𨂿--著`, matching the dictionary — the spike now does the same), or 意傳 tokenizes standalone digits (`2.`, `1927年`) where the spike drops them. Neither tool does whitelisting, variant folding or counting. **Decision input for D2**: the spike aligner (≈ 200 lines, stdlib + `taigi-converter`) already exceeds the library on the failure classes in § 1 and carries no CPAL / Django-era dependency; P2 should keep its own aligner and adopt the two 意傳 behaviours above.

## 8. Inputs offered to P2 (recommendations; scope and order are the USER's)

1. Aligner rules from § 1 (syllable-level consumption, `--` mirroring, paragraph fallback, `N` → `nn`), rewritten as `dictionary/build/corpus_bigrams.py` with the § 1 failure classes as unit-test fixtures.
2. Frequency-aware sub-segmentation instead of greedy (§ 1 artifact).
3. Curated romanized-function-word table (§ 4), `ê` excluded.
4. Hanji variant folding (§ 6.3).
5. Per-source weight: Bible and news boilerplate dominate some keys (`$ → 民視` 2 033, `$ → 耶穌` 1 937); this is the evidence behind the roadmap's lower Bible weight, and suggests filtering `民視新聞報導`-style bylines by exact-sentence frequency.
6. Outputs: `word_bigrams.tsv` (~260 k rows at min 2), `word_unigrams.tsv` (52 k), `$` rows.

7. USER 2026-09-28: `corpus/taigi-typing/articles.js` (35 articles, not ingested into `taigi-corpus`) is read directly by the local build as training input; its text is never committed, only counts. Not included in this spike's numbers.

## 9. Held-out next-word evaluation (added 2026-09-28, USER asked "will accuracy improve?")

Method: the spike sentences were split per source by a hash (10 % held out); the bigram table was built from the other 90 % (K = 30, min count 2, 40 341 keys). For every held-out in-vocab pair `(w₁ → w₂)`: **old** = today's `association.bin` looked up by the last character of `w₁` (what `predict.rs` does), **new** = word-key lookup with character-key backoff (D4). Hit@N = the true `w₂` is among the first N predictions. User-learned pairs are not modelled (both sides equal there). This measures corpus text, not real typing.

| Test set | Pairs | old hit@1 → new | old hit@5 → new | old hit@30 → new | word key used |
|---|---:|---:|---:|---:|---:|
| All sources, 10 % held out | 230 524 | 1.1 % → **10.8 %** | 3.4 % → **24.9 %** | 9.5 % → **43.0 %** | 97 % |
| icorpus_hanji (news) | 32 366 | 0.2 % → 9.3 % | 0.9 % → 18.6 % | 2.9 % → 29.3 % | 94 % |
| moe_kautian 例句 | 7 688 | 1.1 % → 9.4 % | 4.5 % → 21.3 % | 11.0 % → 38.5 % | 95 % |
| sinpak_900leku | 406 | 2.2 % → 10.6 % | 5.4 % → 22.9 % | 13.3 % → 43.3 % | 97 % |
| kok4hau7 | 332 | 0.3 % → 13.3 % | 6.0 % → 23.8 % | 12.7 % → 42.8 % | 97 % |
| taigi_bible_nt | 17 465 | 1.0 % → 15.2 % | 2.5 % → 34.5 % | 9.5 % → 55.9 % | 99 % |
| kipsupin_2009 | 63 428 | 1.4 % → 9.2 % | 4.1 % → 22.0 % | 11.2 % → 39.4 % | 97 % |
| nmtl_dadwt | 52 123 | 1.5 % → 10.7 % | 3.5 % → 26.3 % | 10.8 % → 46.7 % | 98 % |
| khinhoan_pojbh | 56 716 | 1.0 % → 12.5 % | 3.9 % → 28.0 % | 9.8 % → 48.2 % | 99 % |
| **Cross-domain**: train without moe, test all moe 例句 | 78 721 | 1.2 % → 8.0 % | 4.7 % → 18.7 % | 11.7 % → 35.3 % | 95 % |
| **Cross-domain**: train without iCorpus, test all iCorpus news | 336 066 | 0.3 % → 2.9 % | 1.4 % → 5.9 % | 3.3 % → 11.0 % | 79 % |

Both tables show a prediction for ~100 % of pairs, so the gain is in ranking, not in coverage of the strip.

Reading:

- Today's table is a word-completion table (`台 → 灣`), so it almost never contains the next *word*: 1 % top-1, 9.5 % within 30. That is the measured form of brainstorm gap #1.
- The bigram table puts the true next word first about 1 in 9 times and in the first five about 1 in 4, on text of the same kind as its training data. Cross-domain (the honest number for a user's own writing): top-5 19 % on 教典 sentences, 6 % on news when no news was in training — news vocabulary (people, places, agencies) is domain-bound.
- Expected on-device: between the two rows. Real typing is closer to 教典 / prose than to news; the user layer (which this test ignores) adds the user's own pairs on top.

What the numbers do not show: candidate re-ranking (P5) and segmentation (P6) — this test is next-word only.

Spike code: scratchpad `spike/spike.py` + `conv.mjs` + `eval.py` (session-local, not committed). Codex ANALYSIS-ONLY review 2026-09-28: three revisions requested (dropped / OOV columns + dictionary denominator, size breakdown, decision wording) — all applied above.
