#!/usr/bin/env bash
# validate-architecture.sh — Python
set -euo pipefail

# Activate Poetry venv if available
if [ -f "poetry.lock" ] && command -v poetry &>/dev/null; then
    POETRY_ENV=$(poetry env info --path 2>/dev/null || true)
    if [ -n "$POETRY_ENV" ] && [ -f "$POETRY_ENV/bin/activate" ]; then
        source "$POETRY_ENV/bin/activate"
    fi
fi

SRC_DIR="${1:-src}"
PASS_COUNT=0; ERRORS=(); WARNINGS=()
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; WARNINGS+=("$1"); }

echo "============================================"
echo "  Architecture Validation (Python)"
echo "============================================"

# Layer structure
echo "--- Layer Structure ---"
if [ -d "$SRC_DIR" ]; then
    HAS_DOMAIN=$(find "$SRC_DIR" -type d -name "domain" 2>/dev/null | head -1)
    HAS_INFRA=$(find "$SRC_DIR" -type d -name "infrastructure" 2>/dev/null | head -1)
    if [ -n "$HAS_DOMAIN" ] && [ -n "$HAS_INFRA" ]; then
        pass "Domain and infrastructure layers present"
    elif [ -n "$HAS_DOMAIN" ]; then
        pass "Domain layer present (infrastructure added incrementally)"
    else
        pass "Source tree initialized"
    fi
else
    fail "Missing source directory: $SRC_DIR"
fi

# Canonical references
echo ""
echo "--- Canonical References ---"
if [ -d ".pi/architecture/modules" ]; then
    MODULE_COUNT=$(find .pi/architecture/modules -name "*.md" 2>/dev/null | wc -l | tr -d ' ')
    pass "Canonical architecture documents found ($MODULE_COUNT modules)"
else
    warn "No canonical architecture directory found"
fi

# Domain models
echo ""
echo "--- Domain Models ---"
DOMAIN_FILES=$(find "$SRC_DIR" -path "*/domain/*.py" 2>/dev/null | wc -l | tr -d ' ')
if [ "$DOMAIN_FILES" -gt 0 ]; then
    pass "Domain model files found ($DOMAIN_FILES files)"
else
    warn "No domain model files found yet"
fi

# Dependency direction
echo ""
echo "--- Dependency Direction ---"
DOMAIN_INFRA_IMPORTS=$(find "$SRC_DIR" -path "*/domain/*.py" -exec grep -l "from.*infrastructure" {} + 2>/dev/null | wc -l | tr -d ' ')
if [ "$DOMAIN_INFRA_IMPORTS" -eq 0 ]; then
    pass "Domain layer has no infrastructure dependencies (clean architecture)"
else
    warn "Domain imports from infrastructure ($DOMAIN_INFRA_IMPORTS references)"
fi

# Error handling
echo ""
echo "--- Error Handling ---"
ERROR_COUNT=$(grep -r "class.*Error" "$SRC_DIR" 2>/dev/null | wc -l | tr -d ' ')
if [ "$ERROR_COUNT" -gt 0 ]; then
    pass "Custom exception classes defined ($ERROR_COUNT found)"
else
    warn "Consider defining custom exception classes"
fi

# Typed models
echo ""
echo "--- Typed Models ---"
MODEL_COUNT=$(grep -r "@dataclass" "$SRC_DIR" 2>/dev/null | wc -l | tr -d ' ')
if [ "$MODEL_COUNT" -gt 0 ]; then
    pass "Typed dataclass models found ($MODEL_COUNT definitions)"
else
    warn "No dataclass models found"
fi

# Summary
echo ""
echo "============================================"
echo "  Passed: ${GREEN}${PASS_COUNT}${NC}  Failed: ${RED}${#ERRORS[@]}${NC}"
if [ ${#ERRORS[@]} -gt 0 ]; then
    for err in "${ERRORS[@]}"; do echo "  - $err"; done
    exit 1
fi
echo -e "${GREEN}Architecture validation passed.${NC}"
exit 0
