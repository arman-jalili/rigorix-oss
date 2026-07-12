#!/usr/bin/env bash
# ============================================================================
# validate-architecture.sh — Go
# ============================================================================
set -euo pipefail

SRC_DIR="${1:-.}"
PASS_COUNT=0; ERRORS=(); WARNINGS=()
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; WARNINGS+=("$1"); }

echo "============================================"
echo "  Architecture Validation (Go)"
echo "============================================"
echo ""

# ---------------------------------------------------------------------------
# Layer Structure
# ---------------------------------------------------------------------------
echo "--- Layer Structure ---"
HAS_INTERNAL=false
HAS_PKG=false
[ -d "internal" ] && HAS_INTERNAL=true
[ -d "pkg" ] && HAS_PKG=true
if [ "$HAS_INTERNAL" = true ] && [ "$HAS_PKG" = true ]; then
    pass "Domain (internal/) and shared (pkg/) layers present"
elif [ "$HAS_INTERNAL" = true ]; then
    pass "Internal layer present (clean architecture pattern)"
elif [ "$HAS_PKG" = true ]; then
    pass "Pkg layer present (shared library pattern)"
else
    warn "No internal/ or pkg/ directory (consider Go clean architecture layout)"
fi

# ---------------------------------------------------------------------------
# Canonical References
# ---------------------------------------------------------------------------
echo ""
echo "--- Canonical References ---"
if [ -d ".pi/architecture/modules" ]; then
    MODULE_COUNT=$(find .pi/architecture/modules -name "*.md" 2>/dev/null | wc -l | tr -d ' ')
    pass "Canonical architecture documents found ($MODULE_COUNT modules)"
else
    warn "No canonical architecture directory (.pi/architecture/modules) found"
fi

# ---------------------------------------------------------------------------
# Domain Models
# ---------------------------------------------------------------------------
echo ""
echo "--- Domain Models ---"
DOMAIN_FILES=0
for dir in "internal/domain" "pkg/domain"; do
    if [ -d "$dir" ]; then
        COUNT=$(grep -rlE 'type\s+\w+\s+struct' "$dir" --include="*.go" 2>/dev/null | wc -l | tr -d ' ')
        DOMAIN_FILES=$((DOMAIN_FILES + COUNT))
    fi
done
if [ "$DOMAIN_FILES" -gt 0 ]; then
    pass "Domain model struct definitions found ($DOMAIN_FILES files)"
else
    warn "No domain model struct definitions in internal/domain/ or pkg/domain/"
fi

# ---------------------------------------------------------------------------
# Dependency Direction
# ---------------------------------------------------------------------------
echo ""
echo "--- Dependency Direction ---"
VIOLATIONS=0
for dir in "internal/domain" "pkg/domain"; do
    if [ -d "$dir" ]; then
        # Domain should not import from infrastructure or cmd
        VIOLATIONS=$((VIOLATIONS + $(grep -rlE 'import.*"(.*/infrastructure|/cmd)"' "$dir" --include="*.go" 2>/dev/null | wc -l | tr -d ' ')))
    fi
done
if [ "$VIOLATIONS" -eq 0 ]; then
    pass "Domain layer has no infrastructure/cmd import violations"
else
    fail "Domain layer imports from infrastructure/cmd ($VIOLATIONS violations)"
fi

# ---------------------------------------------------------------------------
# Error Handling
# ---------------------------------------------------------------------------
echo ""
echo "--- Error Handling ---"
if [ -f "go.mod" ]; then
    ERROR_VARIABS=$(grep -rE 'var\s+Err\w+\s*=\s*(errors\.New|fmt\.Errorf)' . --include="*.go" 2>/dev/null | wc -l | tr -d ' ')
    ERROR_STRUCTS=$(grep -rE 'type\s+\w*Error\w*\s+struct' . --include="*.go" 2>/dev/null | wc -l | tr -d ' ')
    TOTAL=$((ERROR_VARIABS + ERROR_STRUCTS))
    if [ "$TOTAL" -gt 0 ]; then
        pass "Custom error types defined ($TOTAL definitions)"
    else
        warn "No custom error types found (consider using sentinel errors or error types)"
    fi
else
    warn "Not in a Go module, skipping error handling check"
fi

# ---------------------------------------------------------------------------
# Interfaces
# ---------------------------------------------------------------------------
echo ""
echo "--- Interfaces ---"
if [ -f "go.mod" ]; then
    INTERFACE_COUNT=$(grep -rE 'type\s+\w+\s+interface' . --include="*.go" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$INTERFACE_COUNT" -gt 0 ]; then
        pass "Interface definitions found ($INTERFACE_COUNT interfaces)"
    else
        warn "No interface definitions found (consider defining contracts)"
    fi
else
    warn "Not in a Go module, skipping interface check"
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

echo -e "${GREEN}Architecture validation passed.${NC}"
exit 0
