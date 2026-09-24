---
name: e2e
description: Run the end-to-end test system — build the test-mode IME, type every e2e/scenarios/*.json into a real text field on a VM / container, and read the analyzer's bug + perf report. Use when asked to "run e2e", test typing end to end, check a fix on a real Linux session, or look for engine errors / latency regressions. Args: a platform (default linux) and optionally one scenario id. Reports findings; never opens a fix round itself.
disable-model-invocation: false
---

# E2E run

Design: `docs/architecture/e2e-testing-roadmap.md`. Contract (trace events, scenario format, run layout, finding kinds): `docs/architecture/e2e-trace-schema.md`.

## 1. Run

```sh
make e2e PLATFORM=linux                                   # every scenario, Fcitx5 + IBus
make e2e PLATFORM=linux E2E_RUN=e2e/runs/<name>           # named run dir (default e2e/runs/<timestamp>)
```

| Platform | Where it runs | Status |
|---|---|---|
| `linux` | the macOS UTM guest (`TAIGI_E2E_LINUX_HOST`, default `binhian@192.168.64.2`): test-mode build into `~/taigi-e2e/prefix`, never the VM's own install. CI twin: `.github/workflows/linux-e2e.yml` | available |
| others | their roadmap PRs (macOS PR5, Windows PR6, Android PR7, iOS PR8) | not yet |

The Linux VM is unreachable → every scenario is `skipped` with the reason (not a failure). The first run compiles the release engine inside the VM (slow); later runs are incremental. One scenario only: `make e2e PLATFORM=linux E2E_ONLY=<scenario-id>`.

To compare against an earlier run: `python3 tools/e2e/analyze.py --run <new> --baseline <old>/report.json`.

## 2. Read the report

Open `<run>/report.md`. Order of attention:

1. **`failed` scenarios** — the detail shows the committed text vs the expectation. Open `<run>/<platform>/<scenario>/trace.jsonl`: the `key` → `preedit` → `candidates` → `commit` events show where it diverged. `framework.log` has the daemon's own output.
2. **`error` scenarios** — the driver could not finish (candidate not offered, timeout, daemon down). The reason names the step. A `(hanji, tl) not offered` error lists the first cells actually offered — that is often the real finding.
3. **`bug` findings** — engine panic / error code / adapter reject, with domain + method.
4. **`perf` findings** — per-op p95 / max over `e2e/budgets.json`, or a regression vs baseline. First runs on a new device calibrate the budget; never loosen one to hide a regression.
5. **`trace` / `unverified`** — harness gaps (missing header, an expectation whose events the platform does not write yet), not product bugs.

## 3. Report, do not fix

- A failure is an **observed failure** (`~/.claude/rules/diagnosis-discipline.md`): report the scenario, the trace excerpt and the first divergent event to the USER. A bugfix round needs root cause + USER approval first (CLAUDE.md Core Principle #4).
- Rule out the harness before blaming the IME: a `trace` finding, a timeout on the first key (lexicon load), or a window-focus error point at the driver.
- Never edit a scenario's expectation to match what the run produced. Expectations come from the `source` the scenario names (USER-quoted dogfood items, `knowledge/`).

## 4. Add a scenario

`e2e/scenarios/<id>.json` — `id` = file name, a `source`, intent-level `settings`, `steps` (`text` / `key` with neutral names `enter` `space` `backspace` `escape` `0`–`9` / `pick` by `hanji` + `tl`), `expect` (`committed`, `first_hanji_candidate`). Take sentences from `corpus/README.md` (the `/dogfood` skill greps it). `python3 -m unittest analyze_test` in `tools/e2e/` validates every scenario file.
