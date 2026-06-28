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

# Crate directories to check (workspace: check all crate roots)
CRATE_DIRS=()
if [ -d "src" ]; then
    CRATE_DIRS+=(".")
fi
for crate in engine cli actions; do
    if [ -d "$crate/src" ]; then
        CRATE_DIRS+=("$crate")
    fi
done

if [ ${#CRATE_DIRS[@]} -eq 0 ]; then
    warn "No Rust source directories found (checked ., engine/, cli/, actions/)"
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
TOTAL_CRATES=0
for dir in "${CRATE_DIRS[@]}"; do
    TOTAL_CRATES=$((TOTAL_CRATES + 1))
    for layer in src/domain src/application src/infrastructure; do
        if [ -d "$dir/$layer" ]; then
            LAYERS_FOUND=$((LAYERS_FOUND + 1))
        fi
    done
    # Check per-module layers (e.g. src/{module}/domain/)
    for layer_pattern in domain application infrastructure; do
        count=$(find "$dir/src" -maxdepth 2 -type d -name "$layer_pattern" 2>/dev/null | wc -l | tr -d ' ' || true)
        LAYERS_FOUND=$((LAYERS_FOUND + count))
    done
done

TOTAL_POSSIBLE=$((TOTAL_CRATES * 3))
if [ "$TOTAL_CRATES" -gt 0 ]; then
    if [ "$LAYERS_FOUND" -ge "$((TOTAL_CRATES * 2))" ]; then
        pass "Clean architecture layers detected ($LAYERS_FOUND layers across $TOTAL_CRATES crates)"
    elif [ "$LAYERS_FOUND" -gt 0 ]; then
        pass "Partial layer structure ($LAYERS_FOUND layers across $TOTAL_CRATES crates — includes module-level layers)"
    else
        pass "Flat structure ($TOTAL_CRATES crate(s) — no standard layer dirs, acceptable for thin crates)"
    fi
else
    fail "No architectural layers found (no src/domain/, src/application/, or src/infrastructure/)"
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
for dir in "${CRATE_DIRS[@]}"; do
    for sub in src/domain src/models; do
        if [ -d "$dir/$sub" ]; then
            MODELS=$(grep -rlE '^\s*(pub\s+)?struct\s' "$dir/$sub" 2>/dev/null | wc -l | tr -d ' ')
            DOMAIN_MODELS=$((DOMAIN_MODELS + MODELS))
        fi
    done
done
if [ "$DOMAIN_MODELS" -gt 0 ]; then
    pass "Domain models found ($DOMAIN_MODELS files with struct definitions across $TOTAL_CRATES crates)"
else
    ALL_MODELS=0
    for dir in "${CRATE_DIRS[@]}"; do
        count=$(grep -rlE '^\s*(pub\s+)?struct\s' "$dir/src/" 2>/dev/null | wc -l | tr -d ' ')
        ALL_MODELS=$((ALL_MODELS + count))
    done
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
VIOLATIONS=0
for dir in "${CRATE_DIRS[@]}"; do
    if [ -d "$dir/src/domain" ]; then
        c=$(grep -rE 'use\s+crate::infrastructure' "$dir/src/domain/" 2>/dev/null | wc -l | tr -d ' ')
        VIOLATIONS=$((VIOLATIONS + c))
    fi
done
if [ "$VIOLATIONS" -eq 0 ]; then
    pass "Domain layer does not depend on infrastructure (across ${#CRATE_DIRS[@]} crate(s))"
else
    fail "Domain layer depends on infrastructure ($VIOLATIONS violations)"
fi

# ---------------------------------------------------------------------------
# Error handling
# ---------------------------------------------------------------------------
echo ""
echo "--- Error Handling ---"
HAS_ERROR_TYPES=0
for dir in "${CRATE_DIRS[@]}"; do
    if [ -f "$dir/Cargo.toml" ] && grep -qE '(thiserror|eyre|anyhow)' "$dir/Cargo.toml" 2>/dev/null; then
        HAS_ERROR_TYPES=1
    fi
done

CUSTOM_ERRORS=0
for dir in "${CRATE_DIRS[@]}"; do
    if [ -d "$dir/src" ]; then
        c=$(grep -rE 'enum\s+\w*Error' "$dir/src/" 2>/dev/null | wc -l | tr -d ' ')
        CUSTOM_ERRORS=$((CUSTOM_ERRORS + c))
    fi
done

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
TRAITS=0
for dir in "${CRATE_DIRS[@]}"; do
    if [ -d "$dir/src" ]; then
        c=$(grep -rE '^\s*(pub\s+)?trait\s+' "$dir/src/" 2>/dev/null | wc -l | tr -d ' ')
        TRAITS=$((TRAITS + c))
    fi
done
if [ "$TRAITS" -gt 0 ]; then
    pass "Trait definitions found ($TRAITS interfaces across ${#CRATE_DIRS[@]} crate(s))"
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
for dir in "${CRATE_DIRS[@]}"; do
    if [ -f "$dir/src/lib.rs" ]; then HAS_LIB=$((HAS_LIB + 1)); fi
    if [ -f "$dir/src/main.rs" ]; then HAS_MAIN=$((HAS_MAIN + 1)); fi
done
if [ "$HAS_LIB" -gt 0 ] && [ "$HAS_MAIN" -gt 0 ]; then
    pass "Both lib.rs and main.rs present ($HAS_LIB lib + $HAS_MAIN bin across ${#CRATE_DIRS[@]} crates)"
elif [ "$HAS_LIB" -gt 0 ]; then
    pass "Library crate(s) detected ($HAS_LIB lib.rs files)"
elif [ "$HAS_MAIN" -gt 0 ]; then
    pass "Binary crate(s) detected ($HAS_MAIN main.rs files)"
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
