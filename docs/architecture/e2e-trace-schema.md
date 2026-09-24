# E2E Trace Schema

> **Type**: Reference (contract between the test-build trace writers, the drivers and `tools/e2e/analyze.py`)
> **Keywords**: `e2e`, `trace`, `JSON Lines`, `test mode`, `schema_version`, `TAIGI_E2E_TRACE_V1`
> **Status**: Active — schema version 1 (engine layer, PR1 of `e2e-testing-roadmap.md`); platform-layer events are added by the platform PRs
> **Siblings**: `e2e-testing-roadmap.md` (design D1/D2, PR table)

---

## Test mode only

The trace exists only in builds compiled with the test switch: cargo feature `e2e-trace` on `engine/dispatch` (forwarded by `swift-ffi`, `android-jni` and, in their PRs, the desktop crates). A build without the feature holds no trace code, opens no file and writes nothing (USER 2026-09-24: the production IME emits no logs).

Proof that a shipped library is clean:

| Check | Where |
|---|---|
| Marker `TAIGI_E2E_TRACE_V1` (`dispatch::trace::MARKER`, `#[used]`) absent | `engine/scripts/lib/e2e-trace-guard.sh`, called by `build-xcframework.sh`, `build-macos-xcframework.sh`, `build-android-libs.sh` on every library they stage |
| `dispatch feature "e2e-trace"` absent from the shipped crates' feature graph | `.github/workflows/engine.yml` job `e2e-trace-guard` |
| Positive control: a traced release build DOES carry the marker | same job |

## Opening the trace

The platform resolves a path inside its own sandbox and calls, from test-build code only:

| Platform | Call |
|---|---|
| iOS / macOS | `e2e_trace_open(path)` (swift-bridge; returns false in a build without the feature) |
| Android | `RustEngineBridge.e2eTraceOpen(path)` (JNI symbol exported only with the feature) |
| Windows / Linux | `dispatch::trace::open(path)` through the desktop crates (their PRs) |

The file is append-created; each `open` writes a `trace_open` line and restarts `t_us` at 0.

## Line format

JSON Lines, UTF-8, one object per line. Every line starts with the common fields:

| Field | Type | Meaning |
|---|---|---|
| `t_us` | integer | microseconds since the latest `trace_open` of this process (monotonic clock); for an `engine_request`, the moment the request entered `process_request` |
| `pid` | integer | OS process id |
| `tid` | integer | Rust thread id number (process-local) |
| `event` | string | event kind, below |

Readers ignore fields they do not know, so adding a field is not a version bump; changing a field's meaning is.

### `trace_open`

| Field | Meaning |
|---|---|
| `schema_version` | `1` |
| `marker` | `TAIGI_E2E_TRACE_V1` |
| `layer` | `engine` |
| `engine_version` | `dispatch` crate version |
| `wall_ms` | Unix time in milliseconds — anchors this process's `t_us` for cross-process alignment |

### `engine_request` — one per `dispatch::process_request`

| Field | Meaning |
|---|---|
| `req_id`, `generation` | from the `Response` (`envelope.proto` `Request.id` / `generation`); 0 when the request did not decode |
| `domain` | `phonetics` / `composing` / `lexicon` / `nextword` / `case` (the `Request.payload` field, tags 10–14), `none` when absent or undecodable |
| `method_tag` | field number of the sub-request's `oneof method` (every sub-request holds only that oneof); the analyzer names it from `engine/protos/proto/*.proto` |
| `error` | `ErrorCode` value (0 = OK) |
| `dur_us` | decode + dispatch + encode time; excludes the trace write |
| `req_bytes`, `resp_bytes` | encoded sizes |

### `engine_panic`

A panic caught at the dispatch boundary: `domain`, `method_tag`, `req_bytes`. The caller still receives a `FAIL_INTERNAL` response.

### `adapter_reject`

A request an FFI adapter refused before dispatch: `reason` (`oversize`), `req_bytes`.

## Planned (platform PRs)

Platform-layer events — key injected / received, preedit, candidate list as `(漢字, canonical-TL)`, commit, text observed by the host, memory sample, key-geometry manifest — are specified here by the PR that first writes them (roadmap § Trace schema).
