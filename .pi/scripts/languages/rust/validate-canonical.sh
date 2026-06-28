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

# Count all Rust source files (workspace: check src/, engine/src/, cli/src/, actions/src/)
SRC_DIRS=""
for d in src engine/src cli/src actions/src; do
    if [ -d "$d" ]; then
        SRC_DIRS="$SRC_DIRS $d"
    fi
done

TOTAL_RS=0
for d in $SRC_DIRS; do
    count=$(find "$d" -name "*.rs" 2>/dev/null | wc -l | tr -d ' ')
    TOTAL_RS=$((TOTAL_RS + count))
done

if [ "$TOTAL_RS" -gt 0 ]; then
    # Look for canonical reference patterns in doc comments
    CANONICAL_REFS=0
    for d in $SRC_DIRS; do
        count=$(grep -rE '(///\s*Canonical:|//!\s*@canonical|///\s*Reference:)' "$d" 2>/dev/null | wc -l | tr -d ' ')
        CANONICAL_REFS=$((CANONICAL_REFS + count))
    done
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
MAPPED=0
TOTAL_MODULES=0
# Check module dirs at root and per-crate
MODULE_DIRS=""
for d in .pi engine/.pi cli/.pi actions/.pi; do
    if [ -d "$d/architecture/modules" ]; then
        MODULE_DIRS="$MODULE_DIRS $d/architecture/modules"
    fi
done

if [ -n "$MODULE_DIRS" ]; then
    for mdir in $MODULE_DIRS; do
        MODULE_FILES=$(find "$mdir" -name "*.md" -not -name "*template*" 2>/dev/null)
        for mf in $MODULE_FILES; do
            TOTAL_MODULES=$((TOTAL_MODULES + 1))
            MODULE_NAME=$(basename "$mf" .md)
            # Check in all source directories
            MODULE_FOUND=false
            for d in $SRC_DIRS; do
                if find "$d" -name "*${MODULE_NAME}*" -name "*.rs" 2>/dev/null | grep -q .; then
                    MODULE_FOUND=true
                    break
                fi
            done
            if [ "$MODULE_FOUND" = true ]; then
                MAPPED=$((MAPPED + 1))
            fi
        done
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
    MOD_DOCS=0
    for d in $SRC_DIRS; do
        count=$(grep -rlE '^//!\s' "$d" 2>/dev/null | wc -l | tr -d ' ')
        MOD_DOCS=$((MOD_DOCS + count))
    done
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
# Check ADR dirs at root and per-crate
ADR_DIRS=""
for d in .pi engine/.pi cli/.pi actions/.pi; do
    if [ -d "$d/architecture/decisions" ]; then
        ADR_DIRS="$ADR_DIRS $d/architecture/decisions"
    fi
done

if [ -n "$ADR_DIRS" ]; then
    ADR_FILES=0
    for adir in $ADR_DIRS; do
        count=$(find "$adir" -name "*.md" 2>/dev/null | wc -l | tr -d ' ')
        ADR_FILES=$((ADR_FILES + count))
    done
    if [ "$ADR_FILES" -gt 0 ]; then
        ADR_REFS=0
        for d in $SRC_DIRS; do
            count=$(grep -rE '///\s*ADR-' "$d" 2>/dev/null | wc -l | tr -d ' ')
            ADR_REFS=$((ADR_REFS + count))
        done
        if [ "$ADR_REFS" -gt 0 ]; then
            pass "ADR references found in code ($ADR_REFS references across $ADR_FILES ADRs)"
        else
            warn "No ADR references in code (consider adding /// ADR-NNN comments)"
        fi
    else
        warn "No ADR files found"
    fi
else
    warn "No architecture/decisions/ directories found"
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
