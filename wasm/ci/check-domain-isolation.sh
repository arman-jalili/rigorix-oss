#!/usr/bin/env bash
# check-domain-isolation.sh — criterion 18 as a gate.
#
# `domain/` must reach no ambient capability and must not name the layers that depend on
# it. This is the machine check for CI; `tests/unit/.../domain-isolation_test.rs` asserts
# the same property from inside `cargo test`, so a contributor sees it before pushing.
#
# Comment lines are skipped on purpose: the docs legitimately *name* what is forbidden
# ("no std::time here") — that is how the rule is explained. Code may not contain it.
#
# Fail-closed: if `domain/` cannot be read, this exits non-zero rather than reporting OK.
set -euo pipefail

CRATE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DOMAIN_DIR="$CRATE_DIR/src/domain"

if [ ! -d "$DOMAIN_DIR" ]; then
    echo "❌ no src/domain/ — the gate cannot look, so it does not pass" >&2
    exit 1
fi

# Ambient capabilities: reaching any of these means time, entropy, files, network or the
# process environment entered the domain.
CAPABILITIES=(
    "std::time"
    "SystemTime"
    "std::env"
    "std::fs"
    "std::net"
    "std::process"
    "Uuid::new_v4"
    "uuid::Uuid"
    "thread_rng"
    "OsRng"
    "getrandom"
    "reqwest"
    "tokio"
)

# Outward references: domain may not name the layers that depend on it.
OUTWARD=(
    "crate::infrastructure"
    "crate::interfaces"
    "crate::application"
)

strip_comments() {
    # Drop whole-line comments and doc comments; inline `//` tails are left alone so a
    # capability hidden after a statement is still caught.
    grep -vE '^[[:space:]]*(//|\*|/\*)' "$@" || true
}

FAILED=0

echo "== ambient capabilities in src/domain/ =="
for needle in "${CAPABILITIES[@]}"; do
    hits="$(strip_comments "$DOMAIN_DIR"/*.rs "$DOMAIN_DIR"/*/*.rs 2>/dev/null | grep -F "$needle" || true)"
    if [ -n "$hits" ]; then
        echo "❌ $needle"
        printf '%s\n' "$hits" | head -3
        FAILED=1
    else
        echo "✅ $needle absent"
    fi
done

echo
echo "== layer direction =="
for needle in "${OUTWARD[@]}"; do
    hits="$(strip_comments "$DOMAIN_DIR"/*.rs "$DOMAIN_DIR"/*/*.rs 2>/dev/null | grep -F "$needle" || true)"
    if [ -n "$hits" ]; then
        echo "❌ domain references $needle"
        printf '%s\n' "$hits" | head -3
        FAILED=1
    else
        echo "✅ domain does not reference $needle"
    fi
done

echo
if [ "$FAILED" -ne 0 ]; then
    echo "Domain isolation FAILED: a capability or an outward dependency entered domain/." >&2
    exit 1
fi
echo "Domain isolation OK: the domain reaches the world only through its ports."
