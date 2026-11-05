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
#        bash wasm/ci/check-bundle-independence.sh --self-test
#
# This file is the **only** list of forbidden crates (#1412). `.github/workflows/ci.yml` calls
# it rather than carrying a second, shorter copy whose success line counted neither list.
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
    # #1412: the inline CI list named `ring` and this one did not, so a tree carrying it failed
    # in one place and passed in the other. It joins the list deliberately: `ring` is declared
    # under `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]` (`engine/Cargo.toml:50`),
    # so its absence is a property of that target gate rather than of luck. A `ring` in the
    # wasm32 graph therefore means the gate was lifted — a native crypto provider in a module
    # that only produces evidence.
    "ring|native crypto provider; present in wasm32 only if the target gate was lifted (ADR-013)"
)

# One matcher, used by the gate below and by the self-test, so a seed cannot drift from the
# check it is supposed to exercise (#1412). The graph text names a package as `<name> v<semver>`.
banned_hit() {
    local entry="$1" tree="$2"
    printf '%s\n' "$tree" | grep -qE "^[│├└─ ]*${entry%%|*} v"
}

# Every banned name the tree carries, one per line. Used by the self-test.
banned_violations() {
    local tree="$1" entry
    for entry in "${BANNED[@]}"; do
        if banned_hit "$entry" "$tree"; then printf '%s\n' "${entry%%|*}"; fi
    done
}

# The success line, and the count, come from the array — not from a sentence (#1412).
banned_summary() {
    echo "✅ none of the ${#BANNED[@]} forbidden crates in the wasm32 graph"
    echo "   checked: ${BANNED[@]%%|*}"
}

if [ "${1:-}" = "--self-test" ]; then
    failures=0
    expect_violations() {
        local label="$1" expected="$2" tree="$3" got
        got="$(banned_violations "$tree" | tr '\n' ' ' | sed 's/ *$//')"
        if [ "$got" = "$expected" ]; then
            echo "  ✅ $label → [${got}]"
        else
            echo "  ❌ $label → expected [${expected}], got [${got}]" >&2
            failures=$((failures + 1))
        fi
    }

    clean='rigorix-gate-wasm v1.12.1
├── serde v1.0.0
├── sha2 v0.11.0
└── wasm-bindgen v0.2.130'

    # Seed 1: the tree this gate is meant to pass.
    expect_violations "clean tree passes" "" "$clean"
    # Seed 2: `sqlx` — the crate the deleted inline list did not name at all (#1412).
    expect_violations "sqlx fails" "sqlx" "$clean
├── sqlx v0.8.6"
    # Seed 3: `ring` — the crate the inline list named and this one did not.
    expect_violations "ring fails" "ring" "$clean
└── ring v0.17.14"
    # Seed 4: the count in the message is derived from the array, so a removed entry moves it.
    summary_count="$(banned_summary | head -1 | sed -nE 's/.*none of the ([0-9]+) forbidden.*/\1/p')"
    if [ "$summary_count" = "${#BANNED[@]}" ] && [ "$summary_count" -gt 0 ]; then
        echo "  ✅ derived count → ${summary_count}"
    else
        echo "  ❌ derived count → message says '${summary_count}', array has ${#BANNED[@]}" >&2
        failures=$((failures + 1))
    fi
    if [ "$(banned_violations "$clean" | wc -l | tr -d ' ')" -ne 0 ]; then
        echo "  ❌ a clean tree produced violations" >&2
        failures=$((failures + 1))
    fi

    if [ "$failures" -ne 0 ]; then
        echo "❌ check-bundle-independence self-test failed ($failures)" >&2
        exit 1
    fi
    echo "✅ check-bundle-independence self-test: 3 seeds fired, 1 clean pass, derived count"
    exit 0
fi

cd "$CRATE_DIR"

echo "== wasm dependency graph ($TARGET) =="
if ! TREE="$(cargo tree --target "$TARGET" --edges normal 2>&1)"; then
    echo "❌ could not compute the dependency tree for $TARGET" >&2
    echo "   a gate that passes when it could not look is not a gate" >&2
    echo "$TREE" >&2
    exit 1
fi

FAILED=0
BANNED_FAILED=0
for entry in "${BANNED[@]}"; do
    name="${entry%%|*}"
    why="${entry#*|}"
    if banned_hit "$entry" "$TREE"; then
        echo "❌ $name is in the wasm graph — $why"
        FAILED=1
        BANNED_FAILED=1
    else
        echo "✅ $name absent"
    fi
done
if [ "$BANNED_FAILED" -eq 0 ]; then
    banned_summary
fi

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
