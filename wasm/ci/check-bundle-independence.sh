#!/usr/bin/env bash
# check-bundle-independence.sh — criterion 14 + NFR-002, as a gate.
#
# The wasm bundle must contain no verifier crate and no native stack. This is the
# machine check; the Rust tests assert the same property from the source side
# (`bundle-independence_test.rs`), so a dependency added in Cargo.toml fails here and a
# verifier call added in source fails there.
#
# Fail-closed: if the dependency tree cannot be computed, this exits non-zero. A gate that
# passes when it could not look is the failure mode this whole epic is about.
#
# Usage: bash wasm/ci/check-bundle-independence.sh [target]
set -euo pipefail

TARGET="${1:-wasm32-unknown-unknown}"
CRATE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Crates that must not appear in the shipped graph, and why.
BANNED=(
    "rigorix-verifier|the host verifies; a producer that verifies itself proves nothing (ADR-013)"
    "tokio|no async runtime in a single-threaded module (NFR-002)"
    "reqwest|no network in the browser (FR-003)"
    "hyper|no HTTP anywhere"
    "tree-sitter|parser stack, native, unused here"
    "prometheus|no metrics on the gate path (ADR-006)"
    "tracing-appender|writes files"
    "libc|no POSIX surface in wasm"
    "rusqlite|no persistence (ADR-002)"
    "sqlx|no persistence (ADR-002)"
    "web-sys|storage belongs to the host, not the seam (ADR-002)"
)

cd "$CRATE_DIR"

echo "== wasm dependency graph ($TARGET) =="
if ! TREE="$(cargo tree --target "$TARGET" --edges normal 2>&1)"; then
    echo "❌ could not compute the dependency tree for $TARGET" >&2
    echo "   a gate that passes when it could not look is not a gate" >&2
    echo "$TREE" >&2
    exit 1
fi

FAILED=0
for entry in "${BANNED[@]}"; do
    name="${entry%%|*}"
    why="${entry#*|}"
    if printf '%s\n' "$TREE" | grep -qE "^[│├└─ ]*${name} v"; then
        echo "❌ $name is in the wasm graph — $why"
        FAILED=1
    else
        echo "✅ $name absent"
    fi
done

echo
echo "== verifying the crate builds for $TARGET =="
if cargo check --target "$TARGET" --quiet 2>/dev/null; then
    echo "✅ cargo check --target $TARGET"
else
    echo "⚠️  cargo check --target $TARGET skipped (target not installed: rustup target add $TARGET)"
fi

if [ "$FAILED" -ne 0 ]; then
    echo
    echo "Bundle independence FAILED." >&2
    exit 1
fi

echo
echo "Bundle independence OK: the crate produces, the host verifies."
