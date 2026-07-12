#!/usr/bin/env bash
# ============================================================================
# validate-architecture.sh — Rust
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

echo "============================================"
echo "  Architecture Validation (Rust)"
echo "============================================"
echo ""

if [ ! -f "Cargo.toml" ]; then
    warn "No Cargo.toml found (skipping Rust architecture validation)"
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

# ---------------------------------------------------------------------------
# Layer structure (clean architecture)
# ---------------------------------------------------------------------------
echo "--- Layer Structure ---"
LAYERS_FOUND=0
for layer in src/domain src/application src/infrastructure; do
    if [ -d "$layer" ]; then
        LAYERS_FOUND=$((LAYERS_FOUND + 1))
    fi
done
if [ "$LAYERS_FOUND" -ge 2 ]; then
    pass "Clean architecture layers detected ($LAYERS_FOUND/3)"
elif [ "$LAYERS_FOUND" -eq 1 ]; then
    warn "Partial layer structure found (1/3 layers)"
else
    # Check for alternative common structures
    if [ -d "src/models" ] || [ -d "src/handlers" ] || [ -d "src/services" ]; then
        warn "Alternative project structure detected (no clean architecture layers)"
    else
        fail "No architectural layers found (no src/domain/, src/application/, or src/infrastructure/)"
    fi
fi

# ---------------------------------------------------------------------------
# Canonical references
# ---------------------------------------------------------------------------
echo ""
echo "--- Canonical References ---"
if [ -d ".pi/architecture/modules" ]; then
    MODULE_COUNT=$(find .pi/architecture/modules -name "*.md" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$MODULE_COUNT" -gt 0 ]; then
        pass "Architecture modules defined ($MODULE_COUNT module files)"
    else
        warn "No architecture module files found in .pi/architecture/modules/"
    fi
else
    warn "No .pi/architecture/modules/ directory (no canonical module references)"
fi

# ---------------------------------------------------------------------------
# Domain models
# ---------------------------------------------------------------------------
echo ""
echo "--- Domain Models ---"
DOMAIN_MODELS=0
for dir in src/domain src/models; do
    if [ -d "$dir" ]; then
        MODELS=$(grep -rlE '^\s*(pub\s+)?struct\s' "$dir" 2>/dev/null | wc -l | tr -d ' ')
        DOMAIN_MODELS=$((DOMAIN_MODELS + MODELS))
    fi
done
if [ "$DOMAIN_MODELS" -gt 0 ]; then
    pass "Domain models found ($DOMAIN_MODELS files with struct definitions)"
else
    # Check all of src as fallback
    ALL_MODELS=$(grep -rlE '^\s*(pub\s+)?struct\s' src/ 2>/dev/null | wc -l | tr -d ' ')
    if [ "$ALL_MODELS" -gt 0 ]; then
        pass "Struct definitions found in src/ ($ALL_MODELS files)"
    else
        warn "No struct definitions found"
    fi
fi

# ---------------------------------------------------------------------------
# Dependency direction
# ---------------------------------------------------------------------------
echo ""
echo "--- Dependency Direction ---"
if [ -d "src/domain" ]; then
    DOMAIN_DEPS=$(grep -rE 'use\s+crate::infrastructure' src/domain/ 2>/dev/null | wc -l | tr -d ' ')
    if [ "$DOMAIN_DEPS" -eq 0 ]; then
        pass "Domain layer does not depend on infrastructure"
    else
        fail "Domain layer depends on infrastructure ($DOMAIN_DEPS violations)"
    fi
else
    warn "No src/domain/ directory (cannot check dependency direction)"
fi

# ---------------------------------------------------------------------------
# Error handling
# ---------------------------------------------------------------------------
echo ""
echo "--- Error Handling ---"
HAS_ERROR_TYPES=0
if command -v cargo &>/dev/null; then
    # Check for thiserror, eyre, or anyhow in Cargo.toml
    if grep -qE '(thiserror|eyre|anyhow)' Cargo.toml 2>/dev/null; then
        HAS_ERROR_TYPES=1
    fi
fi
# Also check for custom error enum definitions
CUSTOM_ERRORS=$(grep -rE 'enum\s+\w*Error' src/ 2>/dev/null | wc -l | tr -d ' ')
if [ "$HAS_ERROR_TYPES" -eq 1 ] || [ "$CUSTOM_ERRORS" -gt 0 ]; then
    pass "Custom error handling detected"
else
    warn "No custom error types found (consider thiserror, eyre, or anyhow)"
fi

# ---------------------------------------------------------------------------
# Trait definitions
# ---------------------------------------------------------------------------
echo ""
echo "--- Trait Definitions ---"
TRAITS=$(grep -rE '^\s*(pub\s+)?trait\s+' src/ 2>/dev/null | wc -l | tr -d ' ')
if [ "$TRAITS" -gt 0 ]; then
    pass "Trait definitions found ($TRAITS interfaces)"
else
    warn "No trait definitions found (consider using traits for interfaces)"
fi

# ---------------------------------------------------------------------------
# Crate structure
# ---------------------------------------------------------------------------
echo ""
echo "--- Crate Structure ---"
HAS_LIB=0
HAS_MAIN=0
[ -f "src/lib.rs" ] && HAS_LIB=1
[ -f "src/main.rs" ] && HAS_MAIN=1
if [ "$HAS_LIB" -eq 1 ] && [ "$HAS_MAIN" -eq 1 ]; then
    pass "Both lib.rs and main.rs present (library + binary)"
elif [ "$HAS_LIB" -eq 1 ]; then
    pass "Library crate detected (lib.rs)"
elif [ "$HAS_MAIN" -eq 1 ]; then
    pass "Binary crate detected (main.rs)"
else
    warn "No lib.rs or main.rs found"
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

echo -e "${GREEN}Architecture validation completed.${NC}"
exit 0
