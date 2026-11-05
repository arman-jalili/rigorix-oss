#!/usr/bin/env bash
# run-conformance.sh — the conformance harness as a gate (ADR-009).
#
# Two things must hold, and they are different things:
#
#   1. every committed vector replays and matches what it says it should;
#   2. the deliberately altered vector **does not** match.
#
# (2) is what proves the comparator can fail. A harness without it is a test that passes
# because it compares nothing, which is the failure mode this gate exists to catch.
#
# Fail-closed: zero tests run is a failure. A skipped vector is a failure. There is no
# "conformance skipped" exit path.
set -euo pipefail

CRATE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VECTOR_DIR="$CRATE_DIR/tests/conformance/vectors"
cd "$CRATE_DIR"

if [ ! -d "$VECTOR_DIR" ]; then
    echo "❌ no conformance vectors — the gate cannot look, so it does not pass" >&2
    exit 1
fi

VECTORS=$(find "$VECTOR_DIR" -name '*.json' | wc -l | tr -d ' ')
echo "== $VECTORS committed vector(s) =="

# Every vector must say where its expected bytes came from, and the label must be checkable
# (#1385). A `cli` label without `produced_by` is a claim nobody can audit; a `fixture` label
# with one is a label and a provenance disagreeing. `seeded` carries its sibling's golden, so
# it records the command that produced those bytes and still must not match.
python3 - "$VECTOR_DIR" <<'PROVENANCE'
import json, sys, pathlib
bad = []
for path in sorted(pathlib.Path(sys.argv[1]).glob("*.json")):
    try:
        d = json.loads(path.read_text())
    except Exception as error:
        bad.append(f"{path.name}: not JSON ({error})")
        continue
    gate = d.get("gate")
    produced = d.get("produced_by")
    name = path.name
    if gate is None:
        bad.append(f"{name}: no `gate` label")
        continue
    if gate not in ("cli", "fixture", "seeded"):
        bad.append(f"{name}: unknown gate label '{gate}'")
    if gate == "cli":
        if not isinstance(produced, dict):
            bad.append(f"{name}: gate 'cli' without `produced_by`")
        else:
            for key in ("command", "commit"):
                if not str(produced.get(key, "")).strip():
                    bad.append(f"{name}: `produced_by.{key}` is empty")
            commit = str(produced.get("commit", ""))
            if commit and (len(commit) != 40 or any(c not in "0123456789abcdef" for c in commit)):
                bad.append(f"{name}: `produced_by.commit` is not a commit id: {commit}")
    if gate == "fixture" and produced is not None:
        bad.append(f"{name}: gate 'fixture' carries `produced_by` — a label and a provenance disagree")
    expected = d.get("expected_decision")
    if not isinstance(expected, dict):
        bad.append(f"{name}: no `expected_decision` — a vector without bytes compares nothing")
        continue
    hexed = str(expected.get("hex", ""))
    if not hexed:
        bad.append(f"{name}: `expected_decision.hex` is empty")
        continue
    try:
        raw = bytes.fromhex(hexed)
    except ValueError:
        bad.append(f"{name}: `expected_decision.hex` is not hex")
        continue
    import hashlib
    digest = "sha256:" + hashlib.sha256(raw).hexdigest()
    if digest != expected.get("sha256"):
        bad.append(f"{name}: `expected_decision.sha256` does not name its bytes ({digest})")
if bad:
    print("\u274c vector provenance:", file=sys.stderr)
    for line in bad:
        print("   " + line, file=sys.stderr)
    sys.exit(1)
print("   provenance ok: every vector names where its bytes came from")
PROVENANCE

# No vector may set `normalised_comparison: true` (ADR-014, #1367). The permission that
# allowed it — "while FR-011 is unimplemented" — lost its precondition when ADR-014
# superseded FR-011, and what it permits is the one thing this harness exists to catch: an
# unpinned envelope is 884 B and a pinned one 877 B for the same decision, so a comparator
# that normalises timestamps reports two different byte strings as equal.
NORMALISED=$(grep -l '"normalised_comparison"[[:space:]]*:[[:space:]]*true' "$VECTOR_DIR"/*.json 2>/dev/null || true)
if [ -n "$NORMALISED" ]; then
    echo "❌ vector(s) set normalised_comparison: true — that is a comparison of the thing" >&2
    echo "   normalised, not of the artifact (ADR-014, #1367):" >&2
    printf '   %s\n' $NORMALISED >&2
    exit 1
fi

# And at least one must be the must-fail vector.
if ! grep -l '"seeded_divergence": true' "$VECTOR_DIR"/*.json >/dev/null 2>&1; then
    echo "❌ no seeded-divergence vector: the comparator is unproven" >&2
    exit 1
fi

echo "== replaying vectors =="
OUTPUT="$(cargo test --quiet --features conformance --test unit conformance 2>&1 || true)"
printf '%s\n' "$OUTPUT" | tail -5

PASSED=$(printf '%s\n' "$OUTPUT" | grep -oE '[0-9]+ passed' | grep -oE '[0-9]+' | head -1 || echo 0)
if [ "${PASSED:-0}" -eq 0 ]; then
    echo "❌ no conformance test ran — a skip is a failure" >&2
    exit 1
fi

if printf '%s\n' "$OUTPUT" | grep -qE '[1-9][0-9]* failed'; then
    echo "❌ conformance failed" >&2
    # A stale golden is regenerated by the command the vector itself records, never by
    # rewriting the expectation from the test run (ADR-009, #1385).
    python3 - "$VECTOR_DIR" >&2 <<'REGENERATE'
import json, sys, pathlib
for path in sorted(pathlib.Path(sys.argv[1]).glob("*.json")):
    data = json.loads(path.read_text())
    produced = data.get("produced_by") or {}
    if produced.get("command"):
        print(f"   {path.name}: {produced['command']}")
REGENERATE
    exit 1
fi

echo
echo "Conformance OK: $PASSED test(s) ran, and the seeded vector diverged as required."
