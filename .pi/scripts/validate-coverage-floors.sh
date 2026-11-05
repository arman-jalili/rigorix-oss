#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────
# A coverage floor may not be lowered (#1427).
#
# `.pi/coverage-layers.tsv` is a ratchet, and until this script the rule that made it one —
# "raise a floor when a layer improves; never lower one to make a build pass" — was a sentence
# printed in a failure message. A rule that only lives in prose is not a control.
#
# Two failure modes, both named:
#
#   1. a floor DECREASED relative to the base commit — the ratchet was moved backwards, and
#      the layer's real coverage is now unguarded below the point it had reached;
#   2. a layer DISAPPEARED — a row removed from the table stops gating, and a missing row
#      reads as "no floor" rather than as "this layer lost its floor".
#
# The comparison needs the base file and nothing else, so `--base-file`/`--new-file` are the
# whole interface and the git resolution is a thin wrapper around them. That is also what
# makes the self-test possible without history: four pairs of files, four verdicts.
#
# Usage:
#   bash .pi/scripts/validate-coverage-floors.sh                 # against the base commit
#   bash .pi/scripts/validate-coverage-floors.sh --base main     # against a named ref
#   bash .pi/scripts/validate-coverage-floors.sh --self-test      # check the check
# ─────────────────────────────────────────────────────────────────────
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TABLE_REL=".pi/coverage-layers.tsv"
TABLE="$ROOT/$TABLE_REL"

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

# The resolver the self-test plants a base file and a new file into.
compare() {
    local base_file="$1" new_file="$2"
    python3 - "$base_file" "$new_file" <<'PY'
import sys

def floors(path):
    rows = {}
    for line in open(path):
        if not line.strip() or line.startswith("#"):
            continue
        parts = line.rstrip("\n").split("\t")
        if len(parts) < 5:
            continue
        layer, _crate, _target, _slug, floor = parts[:5]
        try:
            rows[layer] = float(floor)
        except ValueError:
            continue
    return rows

base_path, new_path = sys.argv[1:3]
try:
    base = floors(base_path)
    new = floors(new_path)
except OSError as error:
    print(f"❌ cannot read a floors table: {error}", file=sys.stderr)
    sys.exit(1)

failed = 0
for layer, value in new.items():
    if layer not in base:
        print(f"  · {layer}: no floor in the base — a new layer, nothing to lower", file=sys.stderr)
        continue
    if value < base[layer]:
        print(
            f"❌ {layer}: the floor was lowered — {base[layer]:.2f} % → {value:.2f} %",
            file=sys.stderr,
        )
        failed = 1
for layer in base:
    if layer not in new:
        print(
            f"❌ {layer}: the layer lost its row — a floor that is not in the table gates nothing",
            file=sys.stderr,
        )
        failed = 1

if failed:
    print(
        "   a floor is a record of how far a layer got; lowering it un-guards that distance (see the header of\n"
        f"   {sys.argv[2]})",
        file=sys.stderr,
    )
    sys.exit(1)
print("✅ every floor is the same or higher than the base commit's")
PY
}

# ── self-test: the four verdicts, each shown to happen ──────────────────────
self_test() {
    local tmp failures=0
    tmp="$(mktemp -d)"
    trap 'rm -rf "${tmp:-/nonexistent}"' EXIT

    write_table() {
        local path="$1" domain="$2" interface="$3" application="$4"
        {
            echo "# layer	crate	target	slug	floor	target	measured-on"
            echo "domain	rigorix-gate-wasm	unit	src/(a)/	$domain	95.00	2026-11-05"
            echo "application	rigorix-gate-wasm	unit	src/(b)/	$application	90.00	2026-11-05"
            [ -n "$interface" ] && echo "interface	rigorix-gate-wasm	unit	src/(c)/	$interface	85.00	2026-11-05"
        } > "$path"
    }

    expect() {
        local label="$1" want="$2" base="$3" new="$4" needle="$5" out status
        out="$(compare "$base" "$new" 2>&1)" && status=0 || status=$?
        if [ "$status" -ne "$want" ]; then
            echo "❌ ${label}: exit ${status}, wanted ${want}"
            printf '%s\n' "$out" | sed 's/^/     /'
            failures=$((failures + 1))
            return
        fi
        if ! printf '%s' "$out" | grep -qF "$needle"; then
            echo "❌ ${label}: output does not name '${needle}'"
            printf '%s\n' "$out" | sed 's/^/     /'
            failures=$((failures + 1))
            return
        fi
        echo "  ✅ ${label}"
    }

    write_table "$tmp/base.tsv" 92.45 88.70 100.00
    write_table "$tmp/lowered.tsv" 92.45 87.70 100.00
    write_table "$tmp/raised.tsv" 93.10 89.00 100.00
    write_table "$tmp/dropped.tsv" 92.45 "" 100.00

    expect "a floor lowered by 1.00 fails, naming the layer and both values" 1 \
        "$tmp/base.tsv" "$tmp/lowered.tsv" "interface: the floor was lowered — 88.70 % → 87.70 %"
    expect "a raised floor passes" 0 "$tmp/base.tsv" "$tmp/raised.tsv" "same or higher"
    expect "an unchanged table passes" 0 "$tmp/base.tsv" "$tmp/base.tsv" "same or higher"
    expect "a removed layer fails: a floor not in the table gates nothing" 1 \
        "$tmp/base.tsv" "$tmp/dropped.tsv" "interface: the layer lost its row"

    if [ "$failures" -ne 0 ]; then
        echo -e "${RED}❌ coverage-floors self-test failed (${failures})${NC}"
        return 1
    fi
    echo -e "${GREEN}✅ coverage-floors self-test: 4 verdicts${NC}"
    return 0
}

if [ "${1:-}" = "--self-test" ]; then
    self_test
    exit $?
fi

BASE_REF="${BASE_REF:-origin/main}"
while [ "$#" -gt 0 ]; do
    case "$1" in
        --base)
            BASE_REF="$2"
            shift 2
            ;;
        --base-file)
            BASE_FILE="$2"
            shift 2
            ;;
        --new-file)
            NEW_FILE="$2"
            shift 2
            ;;
        *)
            echo "❌ unknown argument: $1" >&2
            exit 2
            ;;
    esac
done

NEW_FILE="${NEW_FILE:-$TABLE}"

if [ -z "${BASE_FILE:-}" ]; then
    # Compare against `origin/main`; when that IS this commit (a run on `main` itself), the
    # previous commit is the base. A base that cannot be read is a named failure, not a skip:
    # "could not check" must never read as "nothing was lowered".
    base_commit() {
        local candidate="$1"
        git -C "$ROOT" rev-parse --verify --quiet "$candidate^{commit}" 2>/dev/null
    }
    base_of() {
        git -C "$ROOT" rev-parse --verify --quiet "$1" 2>/dev/null
    }
    head="$(base_commit HEAD)"
    resolved="$(base_of "$BASE_REF")"
    if [ -z "$resolved" ]; then
        echo -e "${RED}❌ cannot resolve the base ref '${BASE_REF}' — the floors cannot be compared${NC}" >&2
        echo "   fetch it (git fetch --depth=1 origin main:refs/remotes/origin/main) or pass --base <ref>" >&2
        exit 1
    fi
    if [ "$resolved" = "$head" ]; then
        resolved="$(base_of "HEAD~1")"
        if [ -z "$resolved" ]; then
            echo -e "${RED}❌ ${BASE_REF} is this commit and HEAD~1 is unavailable — nothing to compare${NC}" >&2
            exit 1
        fi
    fi
    BASE_FILE="$(mktemp)"
    if ! git -C "$ROOT" show "${resolved}:${TABLE_REL}" > "$BASE_FILE" 2>/dev/null; then
        echo -e "${RED}❌ ${resolved} does not carry ${TABLE_REL} — the base has no floors to compare${NC}" >&2
        exit 1
    fi
    echo "── coverage floors vs $(git -C "$ROOT" log -1 --format='%h %s' "$resolved") ──"
fi

compare "$BASE_FILE" "$NEW_FILE"
