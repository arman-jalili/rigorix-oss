#!/usr/bin/env bash
# ============================================================================
# validate-canonical.sh — Rust
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
echo "  Canonical Reference Validation (Rust)"
echo "============================================"
echo ""

if [ ! -f "Cargo.toml" ]; then
    warn "No Cargo.toml found (skipping Rust canonical validation)"
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
# Architecture reference tracing
# ---------------------------------------------------------------------------
echo "--- Architecture Reference Tracing ---"
CANONICAL_REFS=0
TOTAL_RS=0

# Count all Rust source files
TOTAL_RS=$(find src -name "*.rs" 2>/dev/null | wc -l | tr -d ' ')
if [ "$TOTAL_RS" -gt 0 ]; then
    # Look for canonical reference patterns in doc comments
    CANONICAL_REFS=$(grep -rE '(///\s*Canonical:|//!\s*@canonical|///\s*Reference:)' src/ 2>/dev/null | wc -l | tr -d ' ')
    if [ "$CANONICAL_REFS" -gt 0 ]; then
        PCT=$((CANONICAL_REFS * 100 / TOTAL_RS))
        pass "Canonical references found: ${CANONICAL_REFS} files (${PCT}% of ${TOTAL_RS} Rust files)"
    else
        fail "No canonical references found in Rust doc comments"
    fi
else
    warn "No Rust source files in src/"
fi

# ---------------------------------------------------------------------------
# Module-to-implementation mapping
# ---------------------------------------------------------------------------
echo ""
echo "--- Module-to-Implementation Mapping ---"
if [ -d ".pi/architecture/modules" ]; then
    MODULE_FILES=$(find .pi/architecture/modules -name "*.md" 2>/dev/null)
    MAPPED=0
    TOTAL_MODULES=0
    for mf in $MODULE_FILES; do
        TOTAL_MODULES=$((TOTAL_MODULES + 1))
        MODULE_NAME=$(basename "$mf" .md)
        # Check if a matching Rust file exists (exact match or containing module name)
        if find src -name "*${MODULE_NAME}*" -name "*.rs" 2>/dev/null | grep -q .; then
            MAPPED=$((MAPPED + 1))
        fi
    done
    if [ "$TOTAL_MODULES" -gt 0 ] && [ "$MAPPED" -eq "$TOTAL_MODULES" ]; then
        pass "All $TOTAL_MODULES architecture modules mapped to implementation"
    elif [ "$MAPPED" -gt 0 ]; then
        pass "$MAPPED/$TOTAL_MODULES architecture modules mapped to implementation"
    else
        fail "No architecture modules mapped to Rust implementation files"
    fi
else
    warn "No .pi/architecture/modules/ directory (no module mapping to validate)"
fi

# ---------------------------------------------------------------------------
# Module documentation
# ---------------------------------------------------------------------------
echo ""
echo "--- Module Documentation ---"
if [ "$TOTAL_RS" -gt 0 ]; then
    MOD_DOCS=$(grep -rlE '^//!\s' src/ 2>/dev/null | wc -l | tr -d ' ')
    if [ "$MOD_DOCS" -gt 0 ]; then
        PCT=$((MOD_DOCS * 100 / TOTAL_RS))
        pass "Module documentation found in ${MOD_DOCS}/${TOTAL_RS} files (${PCT}%)"
    else
        warn "No module-level doc comments found (use //! for module docs)"
    fi
else
    warn "No Rust source files to check for documentation"
fi

# ---------------------------------------------------------------------------
# ADR linkage
# ---------------------------------------------------------------------------
echo ""
echo "--- ADR Linkage ---"
if [ -d ".pi/architecture/decisions" ]; then
    ADR_FILES=$(find .pi/architecture/decisions -name "*.md" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$ADR_FILES" -gt 0 ]; then
        ADR_REFS=$(grep -rE '///\s*ADR-' src/ 2>/dev/null | wc -l | tr -d ' ')
        if [ "$ADR_REFS" -gt 0 ]; then
            pass "ADR references found in code ($ADR_REFS references)"
        else
            warn "No ADR references in code (consider adding /// ADR-NNN comments)"
        fi
    else
        warn "No ADR files found in .pi/architecture/decisions/"
    fi
else
    warn "No .pi/architecture/decisions/ directory (no ADRs to link)"
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

echo -e "${GREEN}Canonical reference validation completed.${NC}"
exit 0
