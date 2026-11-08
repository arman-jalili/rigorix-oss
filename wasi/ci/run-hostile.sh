#!/usr/bin/env bash
# run-hostile.sh — the artifact's memory boundary against eleven named inputs (#1472).
#
#   bash wasi/ci/run-hostile.sh              run the corpus, one line per case
#   bash wasi/ci/run-hostile.sh --self-test  prove the corpus still depends on what it claims
#
# # Why a corpus and not an annotation
#
# `wasi/src/memory.rs` is the artifact's whole memory-safety surface — three `unsafe` blocks,
# all three baselined with a reason. A reason is not a test. The design under test is that the
# **registry is authoritative**: an unregistered pointer is never dereferenced, a length above
# the record is never read, a second free finds nothing. So the corpus walks the edges of the
# registry rather than pretending to fuzz arbitrary memory.
#
# Each case prints its own name, so a failure names the input instead of "1 failed".
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
MODE="run"

while [ $# -gt 0 ]; do
  case "$1" in
    --self-test) MODE="self-test"; shift ;;
    -h|--help) sed -n '2,18p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "run-hostile: unknown argument '$1'" >&2; exit 2 ;;
  esac
done

run_corpus() {
  # `--format=terse` prints one line per case with its name; `--test-threads=1` keeps the
  # registry-per-thread behaviour of case 9 from interleaving with the cap test in case 10
  # (they share a registry within one test binary, and a cap test that runs beside an
  # allocation-heavy one is a test of the scheduler).
  cargo test -p rigorix-gate-wasi --locked --test hostile -- --test-threads=1
}

if [ "$MODE" = "run" ]; then
  cd "$ROOT"
  out="$(run_corpus 2>&1)"
  printf '%s\n' "$out" | grep -E '^test hostile_[0-9]+' | sed 's/^test /  ✓ /; s/ \.\.\. ok$//'
  case_count="$(printf '%s\n' "$out" | grep -cE '^test hostile_[0-9]+_')"
  if [ "$case_count" -lt 11 ]; then
    echo "✗ the corpus ran $case_count case(s); it claims eleven" >&2
    printf '%s\n' "$out" | tail -20 >&2
    exit 1
  fi
  printf '%s\n' "$out" | grep -qE 'test result: ok' || { printf '%s\n' "$out" | tail -20 >&2; exit 1; }
  echo "✓ $case_count hostile input(s), none trapped"
  exit 0
fi

# ── self-test: the corpus must depend on what it says it does ────────────────────
#
# Honest limit, stated rather than papered over: a *mutation* build (delete the refusal, rebuild,
# watch case 5 fail) needs a second compilation of the crate and a writable copy of the tree, and
# this script will not edit the source it is checking. What it does instead is assert that the
# refusals the corpus names are the ones in the source, so deleting one fails here immediately
# and names the file — and that all eleven cases are present and passing, so a case silently
# removed from the file is a failure rather than a smaller number.
fail=0
require() {
  local file="$1" needle="$2" why="$3"
  if grep -qF -- "$needle" "$file"; then
    echo "✓ self-test: $why"
  else
    echo "✗ self-test: $why — '$needle' is not in $file" >&2
    fail=$((fail + 1))
  fi
}

require "$ROOT/wasi/src/memory.rs" "len > MAX_REQUEST_BYTES" "case 5's ceiling refusal is in the source"
require "$ROOT/wasi/src/memory.rs" "the request pointer was not allocated by this module" \
  "case 1/2's registry check is in the source"
require "$ROOT/wasi/src/memory.rs" "the request is longer than the buffer it names" \
  "case 3's length check is in the source"
require "$ROOT/wasi/src/memory.rs" "MAX_OUTSTANDING_BUFFERS" "case 10's cap is in the source"
require "$ROOT/wasi/src/memory.rs" "if len.saturating_add(4) > capacity" \
  "case 11's prefix check is in the source"
require "$ROOT/wasi/ci/run-conformance.mjs" "leaves no room for a length prefix" \
  "the host-side read rule is asserted, not described"

cd "$ROOT"
out="$(run_corpus 2>&1)"
for case_name in 01 02 03 04 05 06 07 08 09 10 11; do
  if printf '%s\n' "$out" | grep -qE "^test hostile_${case_name}_"; then
    echo "✓ self-test: case $case_name is present"
  else
    echo "✗ self-test: case $case_name is missing from the corpus" >&2
    fail=$((fail + 1))
  fi
done
if ! printf '%s\n' "$out" | grep -qE 'test result: ok'; then
  echo "✗ self-test: the corpus does not pass" >&2
  printf '%s\n' "$out" | tail -20 >&2
  fail=$((fail + 1))
fi

[ "$fail" -eq 0 ] || { echo "✗ $fail seed(s) did not hold" >&2; exit 1; }
echo
echo "✅ hostile corpus self-test: 11 cases present, and every refusal they name is in the source"
