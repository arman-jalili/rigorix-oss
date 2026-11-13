#!/usr/bin/env bash
# ============================================================================
# validate-ci.sh — Rust
# ============================================================================
set -euo pipefail

# ---------------------------------------------------------------------------
# Resolve the tree this validator belongs to, rather than trusting the caller's
# cwd. The hardening discovery loop and the CI integration job both invoke these
# from the repository root without `cd`, where a cwd-relative `src/` does not
# exist — so the checks either failed spuriously or passed while silently
# skipping the work they claim to do.
# ---------------------------------------------------------------------------
_TREE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
cd "$_TREE_ROOT" || exit 1
if [ -f Cargo.toml ] && [ ! -d src ]; then
    echo "⏭️  Workspace root (no src/) — the per-crate copies validate each crate."
    exit 0
fi

PASS_COUNT=0
ERRORS=()
WARNINGS=()

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; WARNINGS+=("$1"); }

echo "============================================"
echo "  CI/MR Validation (Rust)"
echo "============================================"
echo ""

# ---------------------------------------------------------------------------
# Cargo.toml detection
# ---------------------------------------------------------------------------
echo "--- Project Detection ---"
# Scope the cargo commands to this crate when the manifest names a package, so
# a per-tree copy validates only its own crate instead of the whole workspace.
CRATE_NAME=""
if [ -f "Cargo.toml" ]; then
    CRATE_NAME=$(grep -E '^name[[:space:]]*=' Cargo.toml | head -1 | sed -E 's/^name[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/')
fi
SCOPE_FLAG=""
if [ -n "$CRATE_NAME" ]; then
    SCOPE_FLAG="-p $CRATE_NAME"
fi

if [ -f "Cargo.toml" ]; then
    pass "Cargo.toml found (crate: ${CRATE_NAME:-workspace})"
    # Extract workspace info if available
    if grep -q '^\[workspace\]' Cargo.toml; then
        echo "  Workspace project detected"
    fi
else
    fail "No Cargo.toml found (not a Rust project)"
    echo ""
    echo "============================================"
    echo "  Summary"
    echo "============================================"
    echo -e "  Passed:   ${GREEN}${PASS_COUNT}${NC}"
    echo -e "  Failed:   ${RED}${#ERRORS[@]}${NC}"
    echo ""
    if [ ${#ERRORS[@]} -gt 0 ]; then
        echo "FAILURES:"
        for err in "${ERRORS[@]}"; do
            echo "  - $err"
        done
    fi
    exit 1
fi

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------
echo ""
echo "--- Build ---"
if cargo build $SCOPE_FLAG --quiet 2>/dev/null; then
    pass "Build succeeded"
else
    fail "Build failed"
fi

# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------
echo ""
echo "--- Tests ---"
if cargo test $SCOPE_FLAG --quiet 2>/dev/null; then
    pass "All tests passed"
else
    fail "Tests failed"
fi

# ---------------------------------------------------------------------------
# Lint
# ---------------------------------------------------------------------------
echo ""
echo "--- Lint ---"
if command -v cargo &>/dev/null && cargo clippy --version &>/dev/null; then
    if cargo clippy $SCOPE_FLAG --all-targets -- -D warnings 2>/dev/null; then
        pass "Clippy passed"
    else
        fail "Clippy found issues"
    fi
else
    warn "cargo clippy not available, skipping lint"
fi

# ---------------------------------------------------------------------------
# Format
# ---------------------------------------------------------------------------
echo ""
echo "--- Format ---"
if command -v cargo &>/dev/null && cargo fmt --version &>/dev/null; then
    if cargo fmt $SCOPE_FLAG --check 2>/dev/null; then
        pass "Format check passed"
    else
        fail "Format check failed (run 'cargo fmt')"
    fi
else
    warn "cargo fmt not available, skipping format check"
fi

# ---------------------------------------------------------------------------
# Security Audit
# ---------------------------------------------------------------------------
echo ""
echo "--- Security Audit ---"
# The policy is `hardening.yml`'s, not this script's: `cargo audit --deny warnings` makes every
# advisory fatal — a vulnerability and an "unsound" warning alike — so the local gate answers the
# same question the same way (#1522). An allowed suppression in `.cargo/audit.toml` still prints
# "allowed warning", which is why the previous grep for "No known vulnerabilities" reported
# findings on a clean tree.
# The audit's subject is the *workspace* lockfile, which is not in a member crate's directory:
# run from `engine/`, `cargo audit` answers "Couldn't load Cargo.lock" and the gate says nothing
# about the tree. Resolve the workspace root, not the crate root.
_WS_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || printf '%s' "$_TREE_ROOT")"
if command -v cargo &>/dev/null && cargo audit --version &>/dev/null; then
    if AUDIT_OUT=$(cd "$_WS_ROOT" && cargo audit --deny warnings 2>&1); then
        pass "No advisories (cargo audit --deny warnings, the pipeline's policy)"
    else
        # `|| true`: an advisory's output that matches none of these patterns must not end the
        # script inside a command substitution — that failure is silent, which is how a gate
        # reports nothing at all.
        DETAIL=$(printf '%s\n' "$AUDIT_OUT" | grep -E "^(Crate|Version|ID|Title):|denied warning" | head -5 | sed 's/^/    /' || true)
        fail "cargo audit --deny warnings failed (hardening.yml denies the same finding):
$DETAIL
    remedy: bump the dependency the advisory names, or record a suppression with an expiry in
    .cargo/audit.toml — a suppression is a decision, not a silence"
    fi
elif command -v cargo &>/dev/null && cargo deny --version &>/dev/null; then
    if (cd "$_WS_ROOT" && cargo deny check 2>/dev/null); then
        pass "cargo deny passed"
    else
        fail "cargo deny reported issues (the pipeline denies advisories)"
    fi
else
    warn "No Rust audit tools available (cargo audit / cargo deny) — the pipeline still denies advisories"
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "============================================"
echo "  Summary"
echo "============================================"
echo -e "  Passed:   ${GREEN}${PASS_COUNT}${NC}"
echo -e "  Failed:   ${RED}${#ERRORS[@]}${NC}"
echo ""

if [ ${#ERRORS[@]} -gt 0 ]; then
    echo "FAILURES:"
    for err in "${ERRORS[@]}"; do
        echo "  - $err"
    done
    exit 1
fi

echo -e "${GREEN}All CI checks passed.${NC}"
exit 0
