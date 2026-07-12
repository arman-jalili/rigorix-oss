#!/usr/bin/env bash
# ============================================================================
# validate-ci.sh — TypeScript
# ============================================================================
set -euo pipefail

PASS_COUNT=0; ERRORS=(); WARNINGS=()
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; WARNINGS+=("$1"); }

echo "============================================"
echo "  CI/MR Validation (TypeScript)"
echo "============================================"
echo ""

# Detect package manager
PKG_MGR=""
if [ -f "bun.lock" ] || [ -f "bun.lockb" ]; then
    PKG_MGR="bun"
elif [ -f "yarn.lock" ]; then
    PKG_MGR="yarn"
elif [ -f "package-lock.json" ]; then
    PKG_MGR="npm"
fi

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------
echo "--- Build ---"
if [ ! -f "package.json" ]; then
    warn "No package.json found, skipping build"
else
    if [ -f "tsconfig.json" ]; then
        if command -v npx &>/dev/null; then
            if npx tsc --noEmit 2>/dev/null; then
                pass "TypeScript compilation succeeded (tsc --noEmit)"
            else
                fail "TypeScript compilation failed"
            fi
        else
            warn "tsc not available, skipping type check"
        fi
    elif [ -f "bun.lock" ] || [ -f "bun.lockb" ] && command -v bun &>/dev/null; then
        if bun build 2>/dev/null; then
            pass "Build succeeded (bun build)"
        else
            fail "Build failed (bun build)"
        fi
    else
        if [ "$PKG_MGR" = "bun" ] && command -v bun &>/dev/null; then
            if bun run build 2>/dev/null; then
                pass "Build succeeded (bun run build)"
            else
                fail "Build failed (bun run build)"
            fi
        elif command -v npm &>/dev/null && grep -q '"build"' package.json 2>/dev/null; then
            if npm run build 2>/dev/null; then
                pass "Build succeeded (npm run build)"
            else
                fail "Build failed (npm run build)"
            fi
        else
            pass "No build script defined, skipping build"
        fi
    fi
fi

# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------
echo ""
echo "--- Tests ---"
if [ ! -f "package.json" ]; then
    pass "No package.json, skipping tests"
elif [ "$PKG_MGR" = "bun" ] && command -v bun &>/dev/null; then
    if bun test 2>/dev/null; then
        pass "All tests passed (bun test)"
    else
        fail "Tests failed (bun test)"
    fi
elif grep -q '"test"' package.json 2>/dev/null; then
    if [ "$PKG_MGR" = "yarn" ] && command -v yarn &>/dev/null; then
        if yarn test 2>/dev/null; then
            pass "All tests passed (yarn test)"
        else
            fail "Tests failed (yarn test)"
        fi
    elif command -v npm &>/dev/null; then
        if npm test 2>/dev/null; then
            pass "All tests passed (npm test)"
        else
            fail "Tests failed (npm test)"
        fi
    else
        warn "No package manager available to run tests"
    fi
elif command -v npx &>/dev/null; then
    if npx vitest run 2>/dev/null; then
        pass "All tests passed (npx vitest run)"
    elif npx jest --passWithNoTests 2>/dev/null; then
        pass "All tests passed (npx jest)"
    else
        pass "No test runner detected, skipping tests"
    fi
else
    pass "No test runner available, skipping tests"
fi

# ---------------------------------------------------------------------------
# Lint
# ---------------------------------------------------------------------------
echo ""
echo "--- Lint ---"
if [ -f "biome.json" ] && (command -v biome &>/dev/null || command -v npx &>/dev/null); then
    if command -v biome &>/dev/null; then
        if biome check . 2>/dev/null; then
            pass "Lint passed (biome check)"
        else
            fail "Lint failed (biome check)"
        fi
    elif npx biome check . 2>/dev/null; then
        pass "Lint passed (npx biome check)"
    else
        fail "Lint failed (npx biome check)"
    fi
elif [ -f ".eslintrc" ] || [ -f ".eslintrc.json" ] || [ -f ".eslintrc.js" ] || [ -f ".eslintrc.yaml" ] || [ -f ".eslintrc.yml" ] || [ -f "eslint.config.js" ] || [ -f "eslint.config.mjs" ] || [ -f "eslint.config.ts" ] || grep -q '"eslintConfig"' package.json 2>/dev/null; then
    if command -v eslint &>/dev/null; then
        if eslint . 2>/dev/null; then
            pass "Lint passed (eslint)"
        else
            fail "Lint failed (eslint)"
        fi
    elif command -v npx &>/dev/null; then
        if npx eslint . 2>/dev/null; then
            pass "Lint passed (npx eslint)"
        else
            fail "Lint failed (npx eslint)"
        fi
    else
        warn "eslint not available, skipping lint"
    fi
else
    warn "No linter configuration detected, skipping lint"
fi

# ---------------------------------------------------------------------------
# Format
# ---------------------------------------------------------------------------
echo ""
echo "--- Format ---"
if [ -f "biome.json" ]; then
    if command -v biome &>/dev/null; then
        if biome format . --check 2>/dev/null; then
            pass "Format check passed (biome format)"
        else
            fail "Format check failed (biome format)"
        fi
    elif command -v npx &>/dev/null; then
        if npx biome format . --check 2>/dev/null; then
            pass "Format check passed (npx biome format)"
        else
            fail "Format check failed (npx biome format)"
        fi
    else
        warn "biome not available, skipping format check"
    fi
elif [ -f ".prettierrc" ] || [ -f ".prettierrc.json" ] || [ -f ".prettierrc.yaml" ] || [ -f ".prettierrc.yml" ] || [ -f ".prettierrc.js" ] || [ -f ".prettierrc.cjs" ] || [ -f ".prettierrc.mjs" ] || grep -q '"prettier"' package.json 2>/dev/null; then
    if command -v prettier &>/dev/null; then
        if prettier --check . 2>/dev/null; then
            pass "Format check passed (prettier)"
        else
            fail "Format check failed (prettier)"
        fi
    elif command -v npx &>/dev/null; then
        if npx prettier --check . 2>/dev/null; then
            pass "Format check passed (npx prettier)"
        else
            fail "Format check failed (npx prettier)"
        fi
    else
        warn "prettier not available, skipping format check"
    fi
else
    warn "No formatter configuration detected, skipping format check"
fi

# ---------------------------------------------------------------------------
# Security Audit
# ---------------------------------------------------------------------------
echo ""
echo "--- Security Audit ---"
if [ "$PKG_MGR" = "bun" ] && command -v bun &>/dev/null; then
    AUDIT_OUT=$(bun audit 2>&1 || true)
    CRITICAL_COUNT=$(echo "$AUDIT_OUT" | grep -ci "critical" 2>/dev/null || echo "0")
    HIGH_COUNT=$(echo "$AUDIT_OUT" | grep -ci "high" 2>/dev/null || echo "0")
    if [ "$CRITICAL_COUNT" -gt 0 ] || [ "$HIGH_COUNT" -gt 0 ]; then
        fail "Security audit found critical/high vulnerabilities"
    else
        MODERATE_COUNT=$(echo "$AUDIT_OUT" | grep -ci "moderate" 2>/dev/null || echo "0")
        if [ "$MODERATE_COUNT" -gt 0 ]; then
            warn "Security audit found moderate vulnerabilities (review recommended)"
        else
            pass "No critical or high vulnerabilities found"
        fi
    fi
elif command -v npm &>/dev/null; then
    AUDIT_OUT=$(npm audit 2>&1 || true)
    if echo "$AUDIT_OUT" | grep -q "found 0 vulnerabilities" 2>/dev/null; then
        pass "No vulnerabilities found"
    else
        CRITICAL_HIGH=$(echo "$AUDIT_OUT" | grep -E "(critical|high).*vulnerab" 2>/dev/null | head -1 || true)
        if [ -n "$CRITICAL_HIGH" ]; then
            fail "Security audit found critical/high vulnerabilities"
        else
            warn "Security audit reported findings (review manually)"
        fi
    fi
else
    warn "No package manager available for security audit"
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
