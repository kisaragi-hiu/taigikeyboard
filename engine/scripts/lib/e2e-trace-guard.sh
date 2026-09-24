#!/usr/bin/env bash
# Marker checks for the test-build-only engine trace
# (docs/architecture/e2e-trace-schema.md § Test mode only). Every traced build
# carries `dispatch::trace::MARKER`; the platform build scripts build shipped
# libraries, so a hit there means the feature leaked into the release graph.
#
# Source it, then call e2e_trace_assert_absent / e2e_trace_assert_present <library>...

E2E_TRACE_MARKER="TAIGI_E2E_TRACE_V1"

e2e_trace_assert_absent() {
    local lib
    for lib in "$@"; do
        if [[ ! -f "$lib" ]]; then
            echo "error: e2e trace guard: $lib does not exist" >&2
            exit 1
        fi
        if LC_ALL=C grep -aq "$E2E_TRACE_MARKER" "$lib"; then
            echo "error: $lib contains $E2E_TRACE_MARKER — the e2e-trace feature leaked into a shipped build" >&2
            exit 1
        fi
    done
}

# Positive control: a traced build must carry the marker, or the absence
# check proves nothing.
e2e_trace_assert_present() {
    local lib
    for lib in "$@"; do
        if ! LC_ALL=C grep -aq "$E2E_TRACE_MARKER" "$lib"; then
            echo "error: $lib lacks $E2E_TRACE_MARKER — a traced build lost the marker" >&2
            exit 1
        fi
    done
}
