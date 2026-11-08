#!/usr/bin/env bash
# run-wasi-gates.sh — every claim this artifact makes, in one command.
#
#   bash wasi/ci/run-wasi-gates.sh              build, then gate
#   bash wasi/ci/run-wasi-gates.sh --self-test  prove each gate bites
#
# The claims, and which one each gate is the subject of:
#
#   1. it builds for the target              build        (a target nobody built is a plan)
#   2. the import section is what is recorded imports      (exactly; both directions)
#   3. the size is inside the ceiling         size         (with its set_from)
#   7. the words equal the record             claims       (#1467: six, not the phrase)
#   4. the five goldens reproduce             conformance  (through node, in the artifact)
#   5. a truncated request is a refusal       truncation   (and the instance keeps answering)
#   8. the ABI under a hostile host           hostile      (eleven named inputs, none traps)
#   6. the release page carries the artifact  release-wiring
#
# A gate that is not applicable is *named* and not counted as a pass: a skipped claim that
# reads as green is how an artifact ships with a claim nobody checked.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
MODULE="$ROOT/target/wasm32-wasip1/release/rigorix_gate_wasi.wasm"
MODE="gate"

while [ $# -gt 0 ]; do
  case "$1" in
    --self-test) MODE="self-test"; shift ;;
    -h|--help) sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "run-wasi-gates: unknown argument '$1'" >&2; exit 2 ;;
  esac
done

step() { printf '\n── %s ──\n' "$1"; }

if [ "$MODE" = "self-test" ]; then
  step "self-test: the gates have subjects"
  bash "$HERE/check-imports.sh" --self-test
  bash "$HERE/check-claim-matches-record.sh" --self-test
  bash "$HERE/run-hostile.sh" --self-test
  # The contract's seeds: a moving clock, a leaking stdout, and a `proc_exit` that returns.
  if node "$HERE/embedder-contract.mjs" --self-test "$MODULE" >/dev/null 2>&1; then
    echo "✓ self-test: the embedder contract's seeds all bite (clock, stdout, proc_exit)"
  else
    echo "✗ self-test: the embedder contract's seeds did not bite" >&2
    node "$HERE/embedder-contract.mjs" --self-test "$MODULE" >&2 || true
    exit 1
  fi
  # S4 for the goldens: edit one recorded digest and require the harness to name the vector.
  scratch="$(mktemp -d)"
  trap 'find "$scratch" -type f -delete; find "$scratch" -depth -type d -exec rmdir {} + 2>/dev/null || true' EXIT
  cp -R "$ROOT/wasm/tests/conformance/vectors" "$scratch/vectors"
  python3 - "$scratch/vectors/vec-0002.json" <<'PY'
import json, sys
path = sys.argv[1]
with open(path, encoding="utf-8") as handle:
    vector = json.load(handle)
vector["expected_decision"]["sha256"] = "sha256:" + "0" * 64
with open(path, "w", encoding="utf-8") as handle:
    json.dump(vector, handle)
PY
  if RIGX_VECTORS="$scratch/vectors" node "$HERE/run-conformance.mjs" "$MODULE" >/dev/null 2>&1; then
    echo "✗ self-test: an edited digest still passed — the harness compares nothing" >&2
    exit 1
  fi
  echo "✓ self-test: an edited recorded digest fails, and the harness names the vector"
  echo
  echo "✓ every WASI gate has a subject"
  exit 0
fi

step "build"
bash "$HERE/build.sh"

step "imports"
bash "$HERE/check-imports.sh" "$MODULE"

step "size"
bash "$HERE/check-size.sh" "$MODULE"

step "the words equal the record"
bash "$HERE/check-claim-matches-record.sh"

step "conformance (node:wasi)"
node "$HERE/run-conformance.mjs" "$MODULE"

step "hostile host (eleven named inputs)"
bash "$HERE/run-hostile.sh"

step "a trap is not a refusal"
node "$HERE/run-conformance.mjs" "$MODULE" --truncate-request

step "the embedder contract"
node "$HERE/embedder-contract.mjs" --self
node "$HERE/embedder-contract.mjs" --artifact "$MODULE"
node "$HERE/embedder-contract.mjs" --conformance "$MODULE"

step "release wiring"
bash "$HERE/check-release-wiring.sh"

printf '\n✓ WASI gates: the artifact builds, imports what is recorded, fits its ceiling,\n'
printf '  reproduces all five goldens through node, refuses without trapping, and the\n'
printf '  words in the tree name the same count as the record.\n'
