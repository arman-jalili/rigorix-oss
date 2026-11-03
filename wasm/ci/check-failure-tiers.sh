#!/usr/bin/env bash
# check-failure-tiers.sh — criterion 26 / ADR-012: three tiers, three promises.
#
# Tier 1  logical failure  -> a typed refusal (tested in the unit suite)
# Tier 2  program defect   -> must not exist on the gate path: no panic, no unwrap, no todo
# Tier 3  trap             -> fatal and host-visible; never rendered as a refusal or an allow
#
# This gate enforces tier 2 statically and pins tier 3's shape. It fails closed.
set -euo pipefail

CRATE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$CRATE_DIR"

FAILED=0

# --- tier 2: no panicking constructs on the gate path ------------------------
# `wasm32-unknown-unknown` has no unwinder. A panic aborts the instance: JavaScript gets a
# RuntimeError, linear memory is unusable, and no refusal value is ever constructed.
BANNED=(
    'unwrap('
    'expect('
    'panic!'
    'unreachable!'
    'todo!'
    'unimplemented!'
    'catch_unwind'
    '.unwrap_or_else(|| panic'
)

echo "== tier 2: panicking constructs in src/ =="
for pattern in "${BANNED[@]}"; do
    # Comment lines are skipped: the docs explain the rule by naming it.
    hits="$(grep -rnF "$pattern" --include='*.rs' src/ 2>/dev/null \
        | grep -vE ':[0-9]+:[[:space:]]*(//|\*|/\*)' \
        | grep -vF 'unwrap_or_default' \
        | grep -vF 'unwrap_or(' \
        || true)"
    if [ -n "$hits" ]; then
        echo "❌ $pattern"
        printf '%s\n' "$hits" | head -3
        FAILED=1
    else
        echo "✅ no $pattern"
    fi
done

# --- tier 2: no `catch_unwind` anywhere, for the reason above ----------------
# Comment lines are skipped: the crate root explains *why* there is none, and that sentence
# contains the word.
if grep -rnF 'catch_unwind' --include='*.rs' src/ 2>/dev/null \
    | grep -vE ':[0-9]+:[[:space:]]*(//|\*|/\*)' | grep -q .; then
    echo "❌ catch_unwind: there is no unwinder to catch (ADR-012)"
    FAILED=1
else
    echo "✅ no catch_unwind in code"
fi

# --- tier 3: the trap must be representable and must not read as a verdict ---
if ! grep -q 'Aborted' src/domain/engine_seam.rs; then
    echo "❌ EngineBindingError has no Aborted variant: a trap would be unrepresentable" >&2
    FAILED=1
else
    echo "✅ EngineBindingError::Aborted exists (tier 3 is representable)"
fi

echo
if [ "$FAILED" -ne 0 ]; then
    echo "Failure tiers FAILED: a panicking construct is on the gate path." >&2
    echo "ADR-012: a refusal is always a decision; a crash is always visible." >&2
    exit 1
fi
echo "Failure tiers OK: tier 2 has no construct left, tier 3 is representable."
