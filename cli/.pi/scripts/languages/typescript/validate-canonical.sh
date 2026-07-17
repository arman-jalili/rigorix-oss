#!/usr/bin/env bash
# ============================================================================
# validate-canonical.sh — TypeScript
# ============================================================================
set -euo pipefail

PASS_COUNT=0; ERRORS=(); WARNINGS=()
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; WARNINGS+=("$1"); }

echo "============================================"
echo "  Canonical Validation (TypeScript)"
echo "============================================"
echo ""

SRC_DIR="${1:-src}"

# ---------------------------------------------------------------------------
# Architecture Reference Tracing
# ---------------------------------------------------------------------------
echo "--- Architecture Reference Tracing ---"
CANONICAL_REFS=0
TOTAL_TS_FILES=0
if [ -d "$SRC_DIR" ]; then
    TOTAL_TS_FILES=$(find "$SRC_DIR" -name "*.ts" -o -name "*.tsx" 2>/dev/null | wc -l | tr -d ' ')
    CANONICAL_REFS=$(find "$SRC_DIR" -type f \( -name "*.ts" -o -name "*.tsx" \) -exec grep -lE '(// Canonical:|@canonical).*\.pi/architecture/' {} + 2>/dev/null | wc -l | tr -d ' ')
    if [ "$TOTAL_TS_FILES" -gt 0 ] && [ "$CANONICAL_REFS" -gt 0 ]; then
        PCT=$((CANONICAL_REFS * 100 / TOTAL_TS_FILES))
        pass "Canonical references found: $CANONICAL_REFS / $TOTAL_TS_FILES files ($PCT%)"
    elif [ "$TOTAL_TS_FILES" -gt 0 ]; then
        fail "No canonical references found in $TOTAL_TS_FILES TypeScript files"
    else
        warn "No TypeScript source files found"
    fi
else
    warn "No source directory found, skipping canonical reference check"
fi

# ---------------------------------------------------------------------------
# Module-to-Implementation Mapping
# ---------------------------------------------------------------------------
echo ""
echo "--- Module-to-Implementation Mapping ---"
if [ -d ".pi/architecture/modules" ]; then
    MAPPED=0
    UNMAPPED=0
    for module_doc in .pi/architecture/modules/*.md; do
        [ -f "$module_doc" ] || continue
        MODULE_NAME=$(basename "$module_doc" .md)
        # Look for matching TS files (by convention: module name maps to src/<module>/ or similar)
        if find "$SRC_DIR" -type f \( -name "*.ts" -o -name "*.tsx" \) 2>/dev/null | grep -qi "$MODULE_NAME" 2>/dev/null; then
            MAPPED=$((MAPPED + 1))
        else
            UNMAPPED=$((UNMAPPED + 1))
        fi
    done
    TOTAL_MODULES=$((MAPPED + UNMAPPED))
    if [ "$UNMAPPED" -eq 0 ] && [ "$MAPPED" -gt 0 ]; then
        pass "All $TOTAL_MODULES architecture modules have implementation files"
    elif [ "$MAPPED" -gt 0 ]; then
        pass "Mapped: $MAPPED/$TOTAL_MODULES modules (check $UNMAPPED unmapped)"
    else
        fail "No architecture modules mapped to implementation"
    fi
else
    warn "No .pi/architecture/modules directory found"
fi

# ---------------------------------------------------------------------------
# Module Documentation
# ---------------------------------------------------------------------------
echo ""
echo "--- Module Documentation ---"
if [ -d "$SRC_DIR" ]; then
    EXPORT_WITH_DOCS=0
    EXPORTS_TOTAL=0
    # Check for documented exports: JSDoc comment (/** ... */) immediately before export
    EXPORTS_TOTAL=$(grep -rE "^\s*(export\s+(const|function|class|interface|type|default))" "$SRC_DIR" --include="*.ts" --include="*.tsx" 2>/dev/null | wc -l | tr -d ' ')
    EXPORT_WITH_DOCS=$(grep -B1 -rE "^\s*(export\s+(const|function|class|interface|type|default))" "$SRC_DIR" --include="*.ts" --include="*.tsx" 2>/dev/null | grep -c '\*/' 2>/dev/null || echo "0")
    if [ "$EXPORTS_TOTAL" -gt 0 ]; then
        if [ "$EXPORT_WITH_DOCS" -gt 0 ]; then
            PCT=$((EXPORT_WITH_DOCS * 100 / EXPORTS_TOTAL))
            pass "Documented exports: $EXPORT_WITH_DOCS / $EXPORTS_TOTAL ($PCT%)"
        else
            warn "No exports have JSDoc documentation"
        fi
    else
        pass "No public exports found, nothing to document"
    fi
else
    warn "No source directory found, skipping documentation check"
fi

# ---------------------------------------------------------------------------
# ADR Linkage
# ---------------------------------------------------------------------------
echo ""
echo "--- ADR Linkage ---"
if [ -d ".pi/architecture/decisions" ]; then
    ADR_FILES=$(find .pi/architecture/decisions -name "*.md" 2>/dev/null | wc -l | tr -d ' ')
    ADR_REFS=$(find "$SRC_DIR" -type f \( -name "*.ts" -o -name "*.tsx" \) -exec grep -rlE '(@adr|ADR-)' {} + 2>/dev/null | wc -l | tr -d ' ')
    if [ "$ADR_FILES" -gt 0 ] && [ "$ADR_REFS" -gt 0 ]; then
        pass "ADR references found: $ADR_REFS files link to $ADR_FILES decisions"
    elif [ "$ADR_FILES" -gt 0 ]; then
        warn "No ADR references in code ($ADR_FILES decisions exist)"
    else
        warn "No ADR directory found"
    fi
else
    warn "No .pi/architecture/decisions directory found"
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

echo -e "${GREEN}Canonical validation passed.${NC}"
exit 0
