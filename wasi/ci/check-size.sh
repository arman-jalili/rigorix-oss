#!/usr/bin/env bash
# check-size.sh — the artifact against the recorded ceiling.
#
#   bash wasi/ci/check-size.sh [<module.wasm>]
#
# The ceiling lives in `budget.txt` with its `set_from`, because a number without the
# measurement it came from cannot be compared to the number it should be. A breach names
# both: the artifact's size and the measurement the ceiling was set from.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
BUDGET="$HERE/budget.txt"
MODULE="${1:-$ROOT/target/wasm32-wasip1/release/rigorix_gate_wasi.wasm}"

field() {
  sed -n "s/^$1:[[:space:]]*//p" "$BUDGET" | head -1
}

ceiling="$(field ceiling_bytes)"
set_from="$(field set_from)"
state="$(field state)"

[ -n "$state" ] || { echo "✗ $BUDGET has no state: line — readiness must be stated, not inferred" >&2; exit 1; }
[ -n "$ceiling" ] || { echo "✗ $BUDGET has no ceiling_bytes: line" >&2; exit 1; }
[ -n "$set_from" ] || { echo "✗ $BUDGET has no set_from: line — a ceiling without a measurement cannot be compared" >&2; exit 1; }
case "$ceiling" in
  ''|*[!0-9]*) echo "✗ ceiling_bytes is not a number: $ceiling" >&2; exit 1 ;;
esac
[ -f "$MODULE" ] || { echo "✗ $MODULE does not exist (bash wasi/ci/build.sh)" >&2; exit 1; }

size="$(wc -c < "$MODULE" | tr -d ' ')"
if [ "$size" -gt "$ceiling" ]; then
  echo "✗ $MODULE is $size bytes, over the $ceiling byte ceiling" >&2
  echo "  the ceiling was set from: $set_from" >&2
  echo "  a raise needs a new set_from — a ceiling moved without a measurement is a wish" >&2
  exit 1
fi
echo "✓ artifact is $size bytes (ceiling $ceiling, state $state)"
