#!/usr/bin/env bash
# ============================================================================
# validate-security.sh — Rust
#
# Tree- and cwd-agnostic. The same script is installed in every tree
# (.pi/, engine/.pi/, mcp/.pi/, cli/.pi/, actions/.pi/) and resolves which
# crate(s) it owns from its OWN path, never from the caller's cwd.
#
# Why: the hardening workflow discovers `*/.pi/scripts/validate-*.sh` and runs
# each one from the repo root WITHOUT cd-ing into the crate. A script that
# assumed cwd == crate root therefore aborted (exit 1, truncated output, no
# finding) — the failure looked like a result. Resolving from the script's own
# location makes both invocations identical:
#
#   * tree root is a crate (engine/, mcp/, …)  -> validate that crate
#   * tree root is the workspace (repo root)   -> validate every crate with src/
#
# Advisory policy, stated once: `cargo audit --deny warnings` **fails** — the same command
# `hardening.yml` runs, so a vulnerability and an "unsound" warning alike stop the run (#1522).
# The remaining heuristics (secret scan, SQL-pattern scan, unsafe surface) warn; their
# classification is deliberately unchanged. `hardening.yml` is the authority on the advisory
# policy; this script agrees with it rather than offering a second opinion.
#
# What its exit code reaches: `local-ci.sh` Stage 4 runs it with real exit codes, so a failure
# stops a local pre-merge run. In CI it fails no job — `hardening.yml` invokes every validator
# with `continue-on-error: true` and `|| true` — which is why `.pi/guards.tsv` still classifies
# this as `advisory`, per that file's own definition, and says so in the note (#1531).
# ============================================================================
set -euo pipefail

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

# ── self-test: a non-advisory audit failure must print, not abort ─────────────
# The bug this proves gone: `DETAIL=$(… | grep …)` under `set -euo pipefail` ends the script when
# the audit's output matches none of the patterns, so the gate prints nothing at all. The stub
# below is a `cargo` whose audit answers exactly that — a fetch failure — and the assertions are
# that the run exits non-zero *and* says why (#1531).
if [ "${1:-}" = "--self-test" ]; then
    scratch="$(mktemp -d)"
    # shellcheck disable=SC2064
    trap "find '$scratch' -type f -delete 2>/dev/null; find '$scratch' -depth -type d -exec rmdir {} + 2>/dev/null || true" EXIT
    cat > "$scratch/cargo" <<'STUB'
#!/usr/bin/env bash
case "$*" in
  *audit*--version*) echo "cargo-audit 0.21.0"; exit 0 ;;
  *audit*)           echo "error: failed to fetch advisory database" >&2; exit 2 ;;
  *)                 exit 0 ;;
esac
STUB
    chmod +x "$scratch/cargo"
    set +e
    out="$(PATH="$scratch:$PATH" "$0" 2>&1)"
    status=$?
    set -e
    ok=1
    [ "$status" -eq 1 ] || { echo "✗ self-test: exit $status, wanted 1" >&2; ok=0; }
    printf '%s' "$out" | grep -q "cargo audit --deny warnings failed" \
        || { echo "✗ self-test: the failure printed no FAIL line — it aborted silently" >&2; ok=0; }
    printf '%s' "$out" | grep -q "remedy:" \
        || { echo "✗ self-test: the failure printed no remedy" >&2; ok=0; }
    if [ "$ok" -eq 1 ]; then
        echo "✅ a non-advisory audit failure prints and fails (exit ${status}, FAIL line, remedy)"
        exit 0
    fi
    printf '%s\n' "$out" | tail -20 >&2
    exit 1
fi

echo "============================================"
echo "  Security Validation (Rust)"
echo "============================================"
echo ""

# ---------------------------------------------------------------------------
# Resolve the crate(s) this tree owns — from this script's own location.
# Layout: <tree>/.pi/scripts/languages/rust/validate-security.sh
# ---------------------------------------------------------------------------
SELF_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TREE_ROOT="$(cd "${SELF_DIR}/../../../.." && pwd)"

CRATES=""
if [ -f "$TREE_ROOT/Cargo.toml" ] && [ -d "$TREE_ROOT/src" ]; then
    # Single-crate tree (engine/, mcp/, cli/, actions/).
    CRATES="$TREE_ROOT"
else
    # Workspace tree (repo root): every direct child that is a crate.
    for d in "$TREE_ROOT"/*/; do
        if [ -f "${d}Cargo.toml" ] && [ -d "${d}src" ]; then
            if [ -z "$CRATES" ]; then CRATES="${d%/}"; else CRATES="${CRATES}
${d%/}"; fi
        fi
    done
fi

if [ -z "$CRATES" ]; then
    warn "No Rust crate (Cargo.toml + src/) found under ${TREE_ROOT} — skipping"
    echo ""
    echo "============================================"
    echo "  Summary"
    echo "============================================"
    echo -e "  Passed:   ${GREEN}${PASS_COUNT}${NC}"
    echo -e "  Failed:   ${RED}${#ERRORS[@]}${NC}"
    echo ""
    echo -e "${GREEN}No Rust project detected, nothing to validate.${NC}"
    exit 0
fi

echo "--- Scope ---"
printf '%s\n' "$CRATES" | while IFS= read -r c; do
    [ -n "$c" ] && echo "  ${c#"$TREE_ROOT"/}"
done
echo ""

# ---------------------------------------------------------------------------
# Aggregate over every crate in scope
# ---------------------------------------------------------------------------
SECRETS_FOUND=0
SQL_INJECT=0
UNSAFE_BLOCKS=0
PANIC_CALLS=0
CRYPTO_MODERN=0
CRYPTO_OPENSSL=0

while IFS= read -r cdir; do
    [ -n "$cdir" ] || continue

    n=$(grep -rnE '=\s*"[A-Za-z0-9+/=_\-]{20,}"' --include="*.rs" "$cdir/src" 2>/dev/null \
        | grep -v '_test\.rs' \
        | grep -v '#\[cfg(test)\]' \
        | grep -v 'mod tests' \
        | grep -v 'interfaces/http/' \
        | grep -v 'observability/metrics/' \
        | grep -v 'AKIAIOSFODNN7EXAMPLE' \
        | grep -v 'ghp_secret12345' \
        | wc -l | tr -d ' ' || true)
    SECRETS_FOUND=$((SECRETS_FOUND + n))

    if grep -qE 'sqlx' "$cdir/Cargo.toml" 2>/dev/null; then
        n=$(grep -rE 'query\s*\(\s*(format!|format_args!)' "$cdir/src" --include="*.rs" 2>/dev/null | wc -l | tr -d ' ' || true)
        SQL_INJECT=$((SQL_INJECT + n))
    fi

    n=$(grep -rE 'unsafe\s*\{' --include="*.rs" "$cdir/src" 2>/dev/null | wc -l | tr -d ' ' || true)
    UNSAFE_BLOCKS=$((UNSAFE_BLOCKS + n))

    n=$(grep -rE '\.unwrap\(\)|\.expect\(' --include="*.rs" "$cdir/src" 2>/dev/null \
        | grep -v '#\[cfg(test)\]' \
        | grep -v 'mod tests' \
        | grep -v '_test\.rs' \
        | wc -l | tr -d ' ' || true)
    PANIC_CALLS=$((PANIC_CALLS + n))

    grep -qE '^(ring|rustls)\s*=' "$cdir/Cargo.toml" 2>/dev/null && CRYPTO_MODERN=$((CRYPTO_MODERN + 1)) || true
    grep -qE '^openssl\s*=' "$cdir/Cargo.toml" 2>/dev/null && CRYPTO_OPENSSL=$((CRYPTO_OPENSSL + 1)) || true
done <<< "$CRATES"

# ---------------------------------------------------------------------------
# Hardcoded secrets
# ---------------------------------------------------------------------------
echo "--- Hardcoded Secrets ---"
if [ "$SECRETS_FOUND" -gt 0 ]; then
    warn "Possible hardcoded secrets found ($SECRETS_FOUND occurrences — verify manually, likely false positives)"
else
    pass "No hardcoded secrets detected"
fi

# ---------------------------------------------------------------------------
# SQL injection
# ---------------------------------------------------------------------------
echo ""
echo "--- SQL Injection ---"
if [ "$SQL_INJECT" -gt 0 ]; then
    fail "Potential SQL injection patterns found ($SQL_INJECT occurrences — avoid format! inside query)"
else
    pass "No SQL injection patterns detected"
fi

# ---------------------------------------------------------------------------
# Dependency audit (workspace-level: cargo resolves the lockfile upward)
# ---------------------------------------------------------------------------
echo ""
echo "--- Dependency Audit ---"
# The policy is the pipeline's, not this script's: `hardening.yml` runs
# `cargo audit --deny warnings`, so *every* advisory is a failure — a vulnerability and an
# "unsound" warning alike. This step used to answer the same question with a warning, which is
# how a tree reported itself safe while the pipeline called it unsafe (RUSTSEC-2026-0486,
# rustix 1.1.4, #1522). One policy, one answer; the two gates must not disagree.
# The audit's subject is the *workspace* lockfile, which is not in a member crate's directory: run
# from `engine/`, `cargo audit` answers "Couldn't load Cargo.lock" — a gate that says nothing about
# the tree, and now (since this script fails on audit failure) fails for the wrong reason. Resolve
# the workspace root, not the crate root. Same defect as #1530's, in the sibling file (#1531).
_WS_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || printf '%s' "$TREE_ROOT")"
if command -v cargo &>/dev/null && cargo audit --version &>/dev/null; then
    if AUDIT_OUT=$(cd "$_WS_ROOT" && cargo audit --deny warnings 2>&1); then
        pass "No advisories (cargo audit --deny warnings, the pipeline's policy)"
    else
        # Name what the pipeline names: the crate, the id, and the version that fixes it.
        # `|| true` is load-bearing: when the audit fails for a reason that matches none of these
        # patterns — a fetch failure, a registry outage, a lockfile error — `grep` exits 1, and
        # under `set -euo pipefail` the assignment would end the script *before* `fail` printed.
        # A gate that prints nothing is worse than the warning it replaced (#1522, #1530, #1531).
        DETAIL=$(printf '%s\n' "$AUDIT_OUT" | grep -E "^(Crate|Version|ID|Title):|denied warning" | head -5 | sed 's/^/    /' || true)
        fail "cargo audit --deny warnings failed (hardening.yml denies the same finding):
$DETAIL
    remedy: bump the dependency the advisory names, or record a suppression with an expiry in
    .cargo/audit.toml — a suppression is a decision, not a silence"
    fi
elif command -v cargo &>/dev/null && cargo deny --version &>/dev/null; then
    if (cd "$_WS_ROOT" && cargo deny check 2>/dev/null); then
        pass "No advisories (cargo deny)"
    else
        fail "cargo deny reported issues (the pipeline denies advisories; review the output above)"
    fi
else
    warn "No Rust audit tools available (cargo audit / cargo deny) — the pipeline still denies advisories"
fi

# ---------------------------------------------------------------------------
# Unsafe code
# ---------------------------------------------------------------------------
echo ""
echo "--- Unsafe Code ---"
if [ "$UNSAFE_BLOCKS" -gt 0 ]; then
    warn "Unsafe code blocks found ($UNSAFE_BLOCKS occurrences — recommend manual audit)"
else
    pass "No unsafe code blocks detected"
fi

# ---------------------------------------------------------------------------
# Panic-prone patterns
# ---------------------------------------------------------------------------
echo ""
echo "--- Panic-Prone Patterns ---"
if [ "$PANIC_CALLS" -gt 0 ]; then
    warn "Panic-prone patterns found ($PANIC_CALLS .unwrap()/.expect() calls in non-test code)"
    echo "         (production panic surface is ratcheted by validate-panic-sites.sh)"
else
    pass "No panic-prone patterns in production code"
fi

# ---------------------------------------------------------------------------
# Cryptographic practices
# ---------------------------------------------------------------------------
echo ""
echo "--- Cryptographic Practices ---"
if [ "$CRYPTO_MODERN" -gt 0 ]; then
    pass "Modern crypto libraries detected (ring/rustls)"
elif [ "$CRYPTO_OPENSSL" -gt 0 ]; then
    warn "OpenSSL detected (prefer ring or rustls for new projects)"
else
    pass "No cryptographic library dependencies found"
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

echo -e "${GREEN}Security validation completed.${NC}"
exit 0
