# 臺羅模式 — tone key commits without a candidate window (MOE parity)

Status: **research only, nothing implemented, low priority** (USER 2026-09-21: 「這個功能不緊急也不重要,需求也不多,所以priority比較後面」).
No round is open; no release scope is implied.

## Request (community suggestion, relayed by USER 2026-09-21)

> 全羅的輸入體驗會當參考Mac版本的教育部輸入法，只要聲調有揤就會直接送出去

Pure-romanization typists want the syllable written the moment its tone key is pressed: `tai5` → `tâi`
lands in the document with no Enter and no candidate window. The USER framed the underlying
question first as "is marked text necessary at all?" — that part is answered below (keep it); the
tone-commit idea survives as a separate mode.

## Today (grounded in code)

| Platform | Space mid-composition | Enter needed in 全羅? |
|---|---|---|
| iOS / Android | commits the converted preedit + writes a space (`ios/.../ActionHandler+KeyActions.swift` `handleSpaceAction`, Android `TextInputKeyHandler` mirror) | No — `tai5gi2␣` |
| macOS / Windows | Space = commit the highlighted cell in the OTHER script (`macos/.../Controller/ComposingAction.swift` `commitAlternateScript`, since 2026-08-25). Under 候選詞顯示 = 羅馬字 there is no other script, so Space is `.ignored` (S26 pin, `TaigiInputController.swift` `commitPresented`) | **Yes** — `tai5gi2⏎`; punctuation also commits (`ComposingKeyIntent.swift` `.commitThenInsert`), Space does not |

So the complaint is real on the desktops only: one extra key per word, plus a candidate window that
pops up for every word even when the typist never picks from it.

## Marked text stays (decided in discussion, 2026-09-21)

Dropping the preedit entirely (commit every keystroke, rewrite on tone) was considered and rejected:

- Tone digits rewrite the syllable retroactively (`siann5` → `siânn`); whole-buffer re-segmentation
  (`kikhilai` → 記起來, `ss` → 鎖匙) rewrites even more. Without a preedit every keystroke becomes
  delete-N + insert: host undo stack, spell-check, URL-bar / search-as-you-type / @mention parsers all
  see the intermediate raw text.
- Cursor moves and tap-away have defined semantics for a preedit (iOS floating marked text is
  discarded; Android `reconcileWithHost`, `behavioral-invariants.md` §368–372). For committed text
  the IME would have to remember "I wrote N characters there" with no reliable host signal — the
  stale `onUpdateSelection` incident of 2026-09-12 is the failure shape, and the failure deletes the
  user's own text.
- Backspace semantics split: inside the preedit ⌫ edits the raw buffer; on committed `siânn` it
  removes one grapheme and the engine state no longer matches the document.
- Every reference IME in `references/` (librime, khiin-rs, McBopomofo, azooKey, MOE) keeps a preedit.
  "Live conversion" IMEs (rakukan, macOS Japanese) remove the conversion step, not the composition.

The tone-commit request does not need the preedit removed: the preedit lives for the few letters
before the tone key, then the tone key commits it.

## What MOE ships

MOE FAQ (臺灣閩南語漢字輸入法 疑難解答, `language.moe.gov.tw/.../blgsujip_1060706.pdf`, Q6):

> 若您需要打出全篇的羅馬拼音，則可選擇「臺羅模式」，在此模式下便無漢字選字的功能。

臺羅模式 is a **separate mode with no candidate function at all**, toggled by `` ` ``. The FAQ does not
say how tone 1 / tone 4 syllables (no digit) are ended, how hyphens are typed, or how a wrong tone is
corrected; the Mac manual link on the MOE site is 404. **Unverified** — to be checked by installing
the MOE Mac IME and typing `tai5`, `tai` + space, `tai5-gi2`.

## Options

| | A — Space commits under roman-only display | B — 臺羅模式 (MOE parity) |
|---|---|---|
| Behaviour | macOS / Windows: when Space would be `.ignored` (no alternate script), commit the highlighted cell + write a space, as mobile does | Under the mode: a TL/POJ tone digit (and Telex tone letter, TPS tone mark) commits the converted preedit immediately; no candidate window; a toneless tail commits raw on space / `-` / punctuation |
| Result for `tâi-gí` | `tai5gi2␣` (auto-hyphen kept) | `tai5-gi2` (hyphen typed by hand, as in MOE) |
| Keeps | candidate window, whole-buffer candidates, learned phrases, auto-hyphen | nothing of the above — by design of the mode, not a loss |
| Size | macOS + Windows ~30 LOC each + S26 pin change | engine `transition.rs` (tone key → `CommitTextReplacingPreedit(derived)` + reset in the mode) + mode flag + four platforms + i18n |
| Independent? | Yes — Space is a dead key for roman-only users today regardless of B | Includes A's behaviour for the toneless tail |

Assessment: B is what the request asks for and what MOE ships; the gain is real for desktop 全羅
typists (one key per word + no window). A is a small wart fix worth doing on its own. Earlier in the
discussion the feature was judged "no keystroke saved" — that judgement rested on mobile Space
behaviour and does not hold on the desktops.

## Open decisions (USER-owned, when the round opens)

1. **Where the mode lives** — reuse 候選詞顯示 = 羅馬字 (already means "no Hanji wanted", zero new
   settings; leaning here) or add a dedicated toggle.
2. **Tone 1 / tone 4 ending** — no digit to trigger on; space / `-` / punctuation commit the raw tail
   (`tai` → `tai`). Confirm MOE's behaviour first.
3. **Mobile parity** — the mode is a shared setting; on mobile the visible change is only that the
   tone key commits earlier (Space already commits). Apply on all four or desktop only.

## Residual costs of B (accepted in principle, same as MOE)

- A tone-key commit must not fire 自動閬一个縫 (auto-space, `behavioral-invariants.md` §23); the
  typist writes spaces and hyphens.
- A wrong tone is already committed: ⌫ removes one grapheme, the raw buffer is gone.

## Related

- `docs/roadmap.md` § Out of scope / deferred — one-line pointer.
- `docs/reports/2026-08-30-hanlo-together-mode-research.md` — 候選詞顯示 = 羅馬字 design (the
  mode B would most likely hang off).
- Project memory `project_taile_mode_tone_commit.md`.
