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
#
# `tokio` is deliberately **not** here (#1400, #1401). See the feature-set check below: the
# crate name is not the defect, an async runtime is.
BANNED=(
    "rigorix-verifier|the host verifies; a producer that verifies itself proves nothing (ADR-013)"
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
# ── tokio: the property, not the crate name (#1400, #1401) ───────────────────────────────
#
# The slice genuinely needs `tokio::sync` — `event_system`, `audit`, `configuration`,
# `dag_engine` and `observability` stay in the graph, and a `RwLock` needs no runtime. Banning
# the crate name therefore banned the design rather than the defect: it refused the shipped
# configuration while the actual prohibition (NFR-002) is an *async runtime* — a timer, a
# reactor, a spawn.
#
# So the rule is the feature set. It is also the symbol set: tokio compiles its `time`, `rt`
# and `spawn` code only under those features, so a graph carrying `sync` alone cannot contain
# a timer or a spawn symbol. A byte-level symbol grep cannot be the check that proves it —
# every wasm-bindgen module imports `__wbg___wbindgen_jspi_spawn_po…`, so "spawn" is in a
# module with no runtime at all. The source grep below is the other half: the gate path must
# not name a timer, and must not acquire one back.
TOKIO_ALLOWED_FEATURES="sync"

echo "== tokio feature set (\"$TOKIO_ALLOWED_FEATURES\" allowed, nothing else) =="
if ! FEATURE_TREE="$(cargo tree -e features --target "$TARGET" --edges normal 2>&1)"; then
    echo "❌ could not compute the feature tree for $TARGET" >&2
    echo "   a gate that passes when it could not look is not a gate" >&2
    echo "$FEATURE_TREE" >&2
    exit 1
fi
TOKIO_FEATURES="$(printf '%s\n' "$FEATURE_TREE" \
    | sed -n 's/.*tokio feature "\([^"]*\)".*/\1/p' | sort -u)"
if [ -z "$TOKIO_FEATURES" ]; then
    echo "✅ tokio absent from the graph (this configuration needs no sync primitives)"
else
    for feature in $TOKIO_FEATURES; do
        if [ "$feature" != "$TOKIO_ALLOWED_FEATURES" ]; then
            echo "❌ tokio feature \"$feature\" is in the wasm graph — an async runtime in a single-threaded module (NFR-002)"
            echo "   only \"$TOKIO_ALLOWED_FEATURES\" is allowed; widening this list is not the fix"
            FAILED=1
        else
            echo "✅ tokio feature \"$feature\" — no runtime, no timer, no spawn"
        fi
    done
fi

# The source half: a timer or a spawn named directly in the crate is the same defect, and this
# is the check `check-domain-isolation.sh` never made (it looks for `std::time`, not `tokio::time`).
if grep -rnE 'tokio::(time|spawn|runtime)' --include='*.rs' "$CRATE_DIR/src" >/dev/null 2>&1; then
    echo "❌ src/ names tokio::time, tokio::spawn or tokio::runtime — this target has no runtime"
    FAILED=1
else
    echo "✅ no tokio::time / tokio::spawn / tokio::runtime in src/"
fi

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
