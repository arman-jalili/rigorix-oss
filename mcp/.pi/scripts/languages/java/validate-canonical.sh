#!/usr/bin/env bash
# ============================================================================
# validate-canonical.sh — Java (language-agnostic, same pattern as others)
# ============================================================================
set -euo pipefail

PASS_COUNT=0; ERRORS=(); WARNINGS=()
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; WARNINGS+=("$1"); }

echo "============================================"
echo "  Canonical Reference Validation (Java)"
echo "============================================"
echo ""

SRC_DIR="${1:-src/main/java}"

# ---------------------------------------------------------------------------
# Architecture Module References
# ---------------------------------------------------------------------------
echo "--- Architecture Module References ---"
if [ -d ".pi/architecture/modules" ]; then
    MODULE_FILES=$(find .pi/architecture/modules -name "*.md" -not -name "module-template.md" 2>/dev/null)
    MODULE_COUNT=$(echo "$MODULE_FILES" | wc -l | tr -d ' ')
    if [ "$MODULE_COUNT" -gt 0 ]; then
        pass "Architecture module docs found ($MODULE_COUNT modules)"
    else
        warn "No architecture module docs found"
    fi
else
    warn "No .pi/architecture/modules directory found"
fi

# ---------------------------------------------------------------------------
# Implementation File References
# ---------------------------------------------------------------------------
echo ""
echo "--- Implementation File References ---"
if [ -d "$SRC_DIR" ]; then
    JAVA_FILES=$(find "$SRC_DIR" -name "*.java" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$JAVA_FILES" -gt 0 ]; then
        # Check for canonical reference headers in Java files
        WITH_REFS=0
        for f in $(find "$SRC_DIR" -name "*.java" 2>/dev/null); do
            if grep -q "Canonical Reference\|Canonical Reference:" "$f" 2>/dev/null; then
                WITH_REFS=$((WITH_REFS + 1))
            fi
        done
        if [ "$WITH_REFS" -gt 0 ]; then
            pass "$WITH_REFS of $JAVA_FILES Java files have canonical reference headers"
        else
            fail "No canonical reference headers found in Java source files"
        fi
    else
        pass "No Java source files found"
    fi
else
    warn "No source directory found, skipping implementation reference check"
fi

# ---------------------------------------------------------------------------
# README References
# ---------------------------------------------------------------------------
echo ""
echo "--- README / Documentation References ---"
if [ -f "README.md" ]; then
    if grep -qi "architecture\|canonical\|reference" README.md 2>/dev/null; then
        pass "README.md references architecture documentation"
    else
        warn "README.md does not reference architecture documentation"
    fi
else
    warn "No README.md found"
fi

# ---------------------------------------------------------------------------
# ADR References
# ---------------------------------------------------------------------------
echo ""
echo "--- ADR References ---"
if [ -d ".pi/architecture/decisions" ]; then
    ADR_COUNT=$(find .pi/architecture/decisions -name "*.md" -not -name "ADR-template.md" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$ADR_COUNT" -gt 0 ]; then
        pass "Architecture Decision Records found ($ADR_COUNT ADRs)"
    else
        warn "No ADRs found in .pi/architecture/decisions/"
    fi
else
    warn "No .pi/architecture/decisions/ directory found"
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

echo -e "${GREEN}Canonical reference validation passed.${NC}"
exit 0
