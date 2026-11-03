#!/usr/bin/env bash
# ============================================================================
# coverage.sh — workspace line-coverage measurement (cargo-llvm-cov).
#
# Produces the REAL coverage number (not heuristic grep checks):
#   cargo llvm-cov --workspace --summary-only
#
# Usage:
#   bash .pi/scripts/coverage.sh                 # measure + print summary
#   bash .pi/scripts/coverage.sh --lcov          # also write target/coverage.lcov
#   bash .pi/scripts/coverage.sh --gate          # fail if workspace LINES < COVERAGE_THRESHOLD
#   bash .pi/scripts/coverage.sh --gate-layers   # fail if any layer slice regresses below its floor
#   bash .pi/scripts/coverage.sh --self-test     # prove the parser before trusting it
#
# Env:
#   COVERAGE_THRESHOLD  minimum workspace line-coverage % (default: 60)
#
# ── The metric (#1348) ───────────────────────────────────────────────────────
# `--summary-only` prints one row per file and a TOTAL row whose groups are
#
#   Regions · Missed Regions · Cover · Functions · Missed Functions · Executed ·
#   Lines · Missed Lines · Cover · Branches · Missed Branches · Cover
#
# so the FIRST percentage on the TOTAL row is *region* coverage. This script
# used to take it and print it as "Line coverage" — measured on the wasm
# application slice: regions 89.64 %, lines 93.49 %, reported 89.64 %. The
# column is now located from the header (not from a fixed position), so a row
# whose groups are in a different order is read by name.
#
# ── Per-layer floors (#1348) ─────────────────────────────────────────────────
# `.pi/coverage-layers.tsv` holds one row per layer: the crate, the test target,
# the file filter, the floor and the documented target. A floor is a ratchet —
# the measured value at the time of writing — so a layer cannot regress while
# the crate-wide number stays green. The target is printed beside it with the
# gap, so a criterion the code does not yet meet reads as a gap rather than as
# a threshold something enforces.
#
# Baseline: 78.92% workspace line coverage (2026-08-31).
# ============================================================================
set -uo pipefail

COVERAGE_THRESHOLD="${COVERAGE_THRESHOLD:-60}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
GATE=0
LCOV=0
LAYERS=0
SELFTEST=0

for arg in "$@"; do
  case "$arg" in
    --gate) GATE=1 ;;
    --lcov) LCOV=1 ;;
    --gate-layers) LAYERS=1 ;;
    --self-test) SELFTEST=1 ;;
    *) echo "unknown arg: $arg" >&2; exit 2 ;;
  esac
done

# ── The parser ──────────────────────────────────────────────────────────────
# Reads a `--summary-only` table on stdin and prints the Lines "Cover" cell of
# the TOTAL row.
#
# The header names its groups with a mix of one- and two-word labels
# ("Regions", "Missed Regions", "Cover", "Functions", "Missed Functions",
# "Executed", "Lines", "Missed Lines", "Branches", "Missed Branches"), while
# every data row has exactly one cell per column. So the header is folded into
# columns first — a "Missed <X>" pair counts as one column — and the Lines
# column is then found *by name*: its Cover cell is two columns further on
# (Lines, Missed Lines, Cover). A row whose groups are in another order is read
# by name, not by position, which is what makes this a fix for #1348 and not a
# different magic number.
LINE_PARSER='
  NR == 1 {
    for (i = 1; i <= NF; i++) {
      if ($i == "Missed") { col++; i++; continue }   # "Missed X" is one column
      col++
      if ($i == "Lines" && lines == 0) { lines = col }
    }
    cover = lines + 2
    next
  }
  $1 == "TOTAL" && cover > 0 { print $cover; exit }
'

# The Lines "Cover" cell of the TOTAL row, or nothing when the table has no
# TOTAL row (the caller turns that into an explicit failure).
coverage_lines_from() {
  awk "$LINE_PARSER"
}

# ── Self-test ───────────────────────────────────────────────────────────────
# A guard that cannot be shown to read the right column is not a guard. Three
# cases: the real layout, a layout whose groups are swapped (the parser must
# follow the header, not a position), and no TOTAL row (the caller's error path).
run_self_test() {
  local failures=0 got

  # (a) The real layout, with Regions and Lines deliberately different: the
  #     Lines value must come out, not the first percentage on the row — which
  #     is exactly what the gate used to report (89.64 as "line coverage").
  local real_layout
  real_layout='Filename   Regions   Missed Regions   Cover   Functions   Missed Functions   Executed   Lines   Missed Lines   Cover   Branches   Missed Branches   Cover
----------------------------------------------------------------------------------------------------
TOTAL   251   26   89.64%   30   8   73.33%   169   11   93.49%   0   0   -'
  got="$(printf '%s\n' "$real_layout" | coverage_lines_from)"
  if [ "$got" != "93.49%" ]; then
    echo "  ✗ SELF-TEST: regions 89.64% / lines 93.49% yielded '${got}', want 93.49%" >&2
    failures=$((failures + 1))
  fi

  # (b) A layout whose groups are in another order: the header still says which
  #     column is Lines, so the answer follows the name.
  local swapped_header
  swapped_header='Filename   Lines   Missed Lines   Cover   Regions   Missed Regions   Cover
-----------------------------------------------------------------------------
TOTAL   169   11   93.49%   251   26   89.64%'
  got="$(printf '%s\n' "$swapped_header" | coverage_lines_from)"
  if [ "$got" != "93.49%" ]; then
    echo "  ✗ SELF-TEST: swapped layout yielded '${got}', want the Lines column 93.49%" >&2
    failures=$((failures + 1))
  fi

  # (c) A fabricated row with the values swapped under the standard header: the
  #     parser must report the cell the header calls Lines (89.64), never the
  #     number that happens to sit first on the row (93.49).
  local fabricated
  fabricated='Filename   Regions   Missed Regions   Cover   Functions   Missed Functions   Executed   Lines   Missed Lines   Cover   Branches   Missed Branches   Cover
----------------------------------------------------------------------------------------------------
TOTAL   169   11   93.49%   30   8   73.33%   251   26   89.64%   0   0   -'
  got="$(printf '%s\n' "$fabricated" | coverage_lines_from)"
  if [ "$got" = "93.49%" ]; then
    echo "  ✗ SELF-TEST: a row with the columns swapped still yielded the first percentage — the parser is positional" >&2
    failures=$((failures + 1))
  fi
  if [ "$got" != "89.64%" ]; then
    echo "  ✗ SELF-TEST: swapped values yielded '${got}', want the cell under the Lines header (89.64%)" >&2
    failures=$((failures + 1))
  fi

  # (d) No TOTAL row: nothing to report, and the caller fails loudly instead of
  #     comparing an empty string.
  local no_total
  no_total='Filename   Regions   Cover
------------------------------
file.rs   10   2   80.00%'
  got="$(printf '%s\n' "$no_total" | coverage_lines_from)"
  if [ -n "$got" ]; then
    echo "  ✗ SELF-TEST: a summary with no TOTAL row yielded '${got}', want nothing" >&2
    failures=$((failures + 1))
  fi

  if [ "$failures" -gt 0 ]; then
    echo "❌ coverage parser self-test FAILED (${failures} case(s))" >&2
    return 1
  fi
  echo "  ✓ Self-test: 5 assertions (real layout, swapped layout, swapped values, no TOTAL row)"
  return 0
}

if [ "$SELFTEST" -eq 1 ]; then
  echo "═══ coverage.sh parser self-test ═══"
  run_self_test || exit 1
  exit 0
fi

# ── Tooling ─────────────────────────────────────────────────────────────────
if ! command -v cargo-llvm-cov >/dev/null 2>&1; then
  echo "cargo-llvm-cov not found — run .pi/scripts/install_coverage_tools.sh" >&2
  exit 1
fi

cd "$ROOT"

# The parser is proven before it is used.
run_self_test || exit 1

# ── Per-layer floors ────────────────────────────────────────────────────────
# layer <TAB> crate <TAB> test-target <TAB> ignore-filename-regex <TAB> floor <TAB> target <TAB> measured-on
gate_layers() {
  local table="${ROOT}/.pi/coverage-layers.tsv"
  if [ ! -f "$table" ]; then
    echo "✗ missing ${table}" >&2
    return 1
  fi

  local failed=0 layer crate target slug floor want line value
  while IFS=$'\t' read -r layer crate target slug floor want _measured; do
    case "$layer" in '' | '#'*) continue ;; esac
    echo ""
    echo "── ${layer} (${crate}, floor ${floor}%, target ${want}%) ──"
    line="$(cargo llvm-cov -p "$crate" --test "$target" --summary-only \
      --ignore-filename-regex "$slug" 2> /dev/null | coverage_lines_from)"
    value="${line%\%}"
    if ! [[ "$value" =~ ^[0-9]+([.][0-9]+)?$ ]]; then
      echo "  ✗ ${layer}: no line coverage in the summary (extracted: '${line}')" >&2
      failed=1
      continue
    fi
    local verdict
    verdict="$(awk -v v="$value" -v w="$want" 'BEGIN {
      if (v >= w) printf "target %s%% met (+%.2fpp)", w, v - w
      else printf "target %s%%, gap %.2fpp", w, w - v
    }')"
    if awk -v v="$value" -v f="$floor" 'BEGIN { exit !(v >= f) }'; then
      echo "  ✓ ${layer}: ${value}% lines (floor ${floor}%) — ${verdict}"
    else
      echo "  ✗ ${layer}: ${value}% lines is below the floor ${floor}% (regression)" >&2
      failed=1
    fi
  done < "$table"

  if [ "$failed" -ne 0 ]; then
    echo ""
    echo "✗ A layer regressed below its floor. Floors are ratchets: raise them when a" >&2
    echo "  layer improves (edit ${table}), do not lower them to make a build pass." >&2
    return 1
  fi
  return 0
}

echo "═══ Workspace coverage (cargo llvm-cov) ═══"
SUMMARY="$(cargo llvm-cov --workspace --summary-only)"
if [ "$LCOV" -eq 1 ]; then
  mkdir -p target
  cargo llvm-cov --workspace --lcov --output-path target/coverage.lcov >/dev/null 2>&1
fi

# Print the per-file table + TOTAL row (summary tail), then extract the
# TOTAL line-coverage percentage (the Lines column, found by header).
echo "$SUMMARY" | tail -12
COVERAGE="$(echo "$SUMMARY" | coverage_lines_from | sed 's/%//')"
# Honest failure: without `set -e`, a failed `cargo llvm-cov` (e.g. a failing
# test) leaves $COVERAGE empty, and the awk comparison below then dies with a
# confusing syntax error plus a bogus "% < 60%" message. Fail explicitly.
if ! [[ "$COVERAGE" =~ ^[0-9]+([.][0-9]+)?$ ]]; then
  echo "" >&2
  echo "✗ Could not determine line coverage (extracted: '${COVERAGE}') —" >&2
  echo "  \`cargo llvm-cov --workspace\` likely failed; see the test output above." >&2
  exit 1
fi
echo ""
echo "Line coverage: ${COVERAGE}% (threshold: ${COVERAGE_THRESHOLD}%)"

status=0

if [ "$GATE" -eq 1 ]; then
  if awk "BEGIN { exit !($COVERAGE >= $COVERAGE_THRESHOLD) }"; then
    echo "✓ Workspace line-coverage gate passed"
  else
    echo "✗ Workspace line coverage below threshold (${COVERAGE}% < ${COVERAGE_THRESHOLD}%)" >&2
    status=1
  fi
fi

if [ "$LAYERS" -eq 1 ]; then
  gate_layers || status=1
fi

exit "$status"
