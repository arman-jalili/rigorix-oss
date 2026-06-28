#!/usr/bin/env bash
# Stage 4: Static Analysis
#
# Verifies:
# - Type checking (mypy/tsc/cargo check/go vet)
# - Import boundary checks
# - Architecture sanity checks
# - Settings/env collision checks

set -euo pipefail

FAIL=0
PASS=0

log_pass() { echo "  ✓ PASS: $1"; PASS=$((PASS + 1)); }
log_fail() { echo "  ✗ FAIL: $1 — $2"; FAIL=$((FAIL + 1)); }

echo "  Running static analysis..."

# Type checking
echo "  Type checking..."
if command -v mypy &>/dev/null && [[ -d "app" ]]; then
    if mypy app --ignore-missing-imports 2>/dev/null; then
        log_pass "mypy type check"
    else
        log_fail "mypy type check" "Type errors found"
    fi
elif command -v tsc &>/dev/null && [[ -f "tsconfig.json" ]]; then
    if tsc --noEmit 2>/dev/null; then
        log_pass "tsc type check"
    else
        log_fail "tsc type check" "Type errors found"
    fi
elif command -v cargo &>/dev/null && [[ -f "Cargo.toml" ]]; then
    if cargo check 2>/dev/null; then
        log_pass "cargo check"
    else
        log_fail "cargo check" "Compilation errors"
    fi
elif command -v go &>/dev/null && [[ -f "go.mod" ]]; then
    if go vet ./... 2>/dev/null; then
        log_pass "go vet"
    else
        log_fail "go vet" "Vet errors found"
    fi
else
    echo "  No type checker found, skipping."
fi

# Import boundary checks
echo "  Checking import boundaries..."
if [[ -f ".pi/scripts/ci/check_import_boundaries.py" ]]; then
    if python3 ".pi/scripts/ci/check_import_boundaries.py" 2>/dev/null; then
        log_pass "import boundary check"
    else
        log_fail "import boundary check" "Boundary violations found"
    fi
else
    # Basic check: no domain→infrastructure imports
    violations=0
    if [[ -d "app/domain" ]]; then
        while IFS= read -r file; do
            if grep -qE "from.*infrastructure|from.*api|import.*infrastructure" "$file" 2>/dev/null; then
                ((violations++))
                log_fail "import boundary" "$(basename "$file") imports from infrastructure/api"
            fi
        done < <(find app/domain -name "*.py" 2>/dev/null || true)
    fi
    if [[ $violations -eq 0 ]]; then
        log_pass "import boundary check (no cross-layer violations)"
    fi
fi

# Architecture sanity
echo "  Running architecture sanity checks..."
if [[ -f ".pi/scripts/ci/check_arch_sanity.py" ]]; then
    if python3 ".pi/scripts/ci/check_arch_sanity.py" 2>/dev/null; then
        log_pass "architecture sanity"
    else
        log_fail "architecture sanity" "Sanity checks failed"
    fi
else
    log_pass "architecture sanity (no script found, skipping)"
fi

# Settings/env collision
echo "  Checking settings/env collision..."
if [[ -f ".env.example" && -f ".env" ]]; then
    collisions=0
    while IFS= read -r line; do
        var_name=$(echo "$line" | cut -d= -f1)
        if [[ -n "$var_name" && "$var_name" != \#* ]]; then
            example_val=$(grep "^${var_name}=" ".env.example" 2>/dev/null | cut -d= -f2-)
            actual_val=$(grep "^${var_name}=" ".env" 2>/dev/null | cut -d= -f2-)
            if [[ "$example_val" == "$actual_val" && -n "$example_val" ]]; then
                ((collisions++))
                log_fail "env collision" "${var_name} in .env matches .env.example value"
            fi
        fi
    done < <(grep -v "^#" ".env.example" 2>/dev/null | grep "=" || true)
    if [[ $collisions -eq 0 ]]; then
        log_pass "settings/env collision check (no collisions)"
    fi
else
    log_pass "settings/env collision check (no .env files)"
fi

if [[ $FAIL -gt 0 ]]; then
    echo "  Static analysis FAILED (${FAIL} failure(s))"
    exit 1
fi

echo "  Static analysis passed (${PASS} check(s))"
exit 0
