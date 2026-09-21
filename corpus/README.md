# corpus

Real Taiwanese text for manual testing. Nothing here feeds a build, a
dictionary artifact, or an automated test — the corpus is where dogfood
sentences come from, so an `Sn` item can cite a sentence a person actually
wrote instead of one made up for the test.

## `taigi-typing/` (submodule)

[luke871016/taigi_typing](https://github.com/luke871016/taigi_typing) — a
typing-practice page whose data files carry aligned hanji + TL text:

| File | Content | Shape |
|---|---|---|
| `exampleSentences.js` | 7,787 教典 example sentences | `{hanji, tailo, huagi, audio_url, dic_url}` — one `dic_url` per sentence links the 教典 entry |
| `articles.js` | 35 articles by named authors | `type: "mapped"` (hanji + tailo lines aligned) or `"single"` (hanji only) |

Licensing (from its README): the 教典 sentences are CC BY-ND 3.0 TW — quote,
never rework; the articles are the authors' and may be used only inside that
page — quote a short excerpt in a dogfood item, never copy an article into
this repo or a dictionary source. The page code is CC0.

## Picking a test sentence

Grep the form under test in the TL field, then read the hanji beside it:

```sh
# khinsiann after a verb
grep -o '"tailo": "[^"]*--lâi[^"]*"' corpus/taigi-typing/exampleSentences.js | head
# a word by hanji
grep -B1 '"tailo"' corpus/taigi-typing/exampleSentences.js | grep -A1 '轉來' | head
```

A dogfood item built from the corpus states the sentence (hanji + TL), the
keystrokes, the pick sequence, the expected result, and the `dic_url`.

Bootstrap: `git submodule update --init corpus/taigi-typing` (or clone with
`--recurse-submodules`).
