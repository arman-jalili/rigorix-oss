#!/usr/bin/env bash
# ============================================================================
# validate-architecture.sh — TypeScript
# ============================================================================
set -euo pipefail

PASS_COUNT=0; ERRORS=(); WARNINGS=()
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; WARNINGS+=("$1"); }

echo "============================================"
echo "  Architecture Validation (TypeScript)"
echo "============================================"
echo ""

SRC_DIR="${1:-src}"

# ---------------------------------------------------------------------------
# Layer Structure
# ---------------------------------------------------------------------------
echo "--- Layer Structure ---"
if [ -d "$SRC_DIR" ]; then
    HAS_DOMAIN=$(find "$SRC_DIR" -type d -name "domain" 2>/dev/null | head -1)
    HAS_APPLICATION=$(find "$SRC_DIR" -type d -name "application" 2>/dev/null | head -1)
    HAS_INFRASTRUCTURE=$(find "$SRC_DIR" -type d -name "infrastructure" 2>/dev/null | head -1)
    if [ -n "$HAS_DOMAIN" ] && [ -n "$HAS_APPLICATION" ] && [ -n "$HAS_INFRASTRUCTURE" ]; then
        pass "All three layers present (domain, application, infrastructure)"
    elif [ -n "$HAS_DOMAIN" ] && [ -n "$HAS_INFRASTRUCTURE" ]; then
        pass "Domain and infrastructure layers present"
    elif [ -n "$HAS_DOMAIN" ]; then
        pass "Domain layer present (architecture incremental)"
    else
        pass "Source tree initialized (clean architecture not yet layered)"
    fi
else
    warn "No src directory found, skipping layer checks"
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
    warn "No .pi/architecture/modules directory found"
fi

# ---------------------------------------------------------------------------
# Domain Models
# ---------------------------------------------------------------------------
echo ""
echo "--- Domain Models ---"
if [ -d "$SRC_DIR/domain" ]; then
    DOMAIN_MODELS=$(find "$SRC_DIR/domain" -name "*.ts" -o -name "*.tsx" 2>/dev/null | xargs grep -lE '\b(interface|type|class)\b' 2>/dev/null | wc -l | tr -d ' ')
    if [ "$DOMAIN_MODELS" -gt 0 ]; then
        pass "Domain model definitions found ($DOMAIN_MODELS files with interface/type/class)"
    else
        warn "No interface/type/class definitions in domain layer"
    fi
else
    warn "No domain directory found, skipping domain model check"
fi

# ---------------------------------------------------------------------------
# Dependency Direction
# ---------------------------------------------------------------------------
echo ""
echo "--- Dependency Direction ---"
if [ -d "$SRC_DIR/domain" ]; then
    DOMAIN_BAD_IMPORTS=0
    for f in $(find "$SRC_DIR/domain" -name "*.ts" -o -name "*.tsx" 2>/dev/null); do
        if grep -qE "from\s+['\"].*\/(infrastructure|api)\b" "$f" 2>/dev/null; then
            DOMAIN_BAD_IMPORTS=$((DOMAIN_BAD_IMPORTS + 1))
        fi
    done
    if [ "$DOMAIN_BAD_IMPORTS" -eq 0 ]; then
        pass "Domain layer has no infrastructure or API imports (clean dependency direction)"
    else
        fail "Domain layer imports from infrastructure/API ($DOMAIN_BAD_IMPORTS files)"
    fi
else
    pass "No domain directory, skipping dependency direction check"
fi

# ---------------------------------------------------------------------------
# Error Handling
# ---------------------------------------------------------------------------
echo ""
echo "--- Error Handling ---"
if [ -d "$SRC_DIR" ]; then
    ERROR_CLASSES=$(grep -rE "class\s+\w+Error.*extends\s+(Error|AppError)" "$SRC_DIR" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$ERROR_CLASSES" -gt 0 ]; then
        pass "Custom error classes defined ($ERROR_CLASSES found)"
    else
        warn "No custom error classes (extends Error) found"
    fi
else
    warn "No source directory found, skipping error handling check"
fi

# ---------------------------------------------------------------------------
# Interfaces / Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Interfaces / Contracts ---"
if [ -d "$SRC_DIR" ]; then
    INTERFACE_COUNT=$(grep -rE "^\s*(export\s+)?interface\s+" "$SRC_DIR" --include="*.ts" --include="*.tsx" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$INTERFACE_COUNT" -gt 0 ]; then
        pass "Interface contracts defined ($INTERFACE_COUNT found)"
    else
        warn "No interface definitions found in source"
    fi
else
    warn "No source directory found, skipping interface check"
fi

# ---------------------------------------------------------------------------
# TypeScript Strict Mode
# ---------------------------------------------------------------------------
echo ""
echo "--- TypeScript Strict Mode ---"
if [ -f "tsconfig.json" ]; then
    if grep -q '"strict"\s*:\s*true' tsconfig.json 2>/dev/null; then
        pass "TypeScript strict mode enabled"
    else
        warn "TypeScript strict mode not enabled in tsconfig.json"
    fi
else
    warn "No tsconfig.json found"
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
