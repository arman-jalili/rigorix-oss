#!/usr/bin/env bash
# ============================================================================
# validate-canonical.sh — Go
# ============================================================================
set -euo pipefail

PASS_COUNT=0; ERRORS=(); WARNINGS=()
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; WARNINGS+=("$1"); }

echo "============================================"
echo "  Canonical Reference Validation (Go)"
echo "============================================"
echo ""

# ---------------------------------------------------------------------------
# Architecture Reference Tracing
# ---------------------------------------------------------------------------
echo "--- Architecture Reference Tracing ---"
if [ -f "go.mod" ]; then
    GO_FILES=$(find . -name "*.go" -not -path "./vendor/*" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$GO_FILES" -gt 0 ]; then
        CANONICAL_REFS=$(grep -rlE '//\s*(Canonical:|@canonical|Reference:|Canonical\s+ref)' . --include="*.go" 2>/dev/null | wc -l | tr -d ' ')
        if [ "$CANONICAL_REFS" -gt 0 ]; then
            PCT=$((CANONICAL_REFS * 100 / GO_FILES))
            pass "Canonical references found in ${CANONICAL_REFS}/${GO_FILES} Go files (${PCT}%)"
        else
            fail "No canonical reference comments (// Canonical: or // @canonical) found"
        fi
    else
        pass "No Go source files to check"
    fi
else
    warn "Not in a Go module, skipping reference tracing"
fi

# ---------------------------------------------------------------------------
# Module-to-Implementation Mapping
# ---------------------------------------------------------------------------
echo ""
echo "--- Module-to-Implementation Mapping ---"
if [ -d ".pi/architecture/modules" ]; then
    MAPPED=0
    TOTAL=0
    for md in .pi/architecture/modules/*.md; do
        [ -f "$md" ] || continue
        TOTAL=$((TOTAL + 1))
        # Extract module name from filename (strip .md and path)
        MODULE_NAME=$(basename "$md" .md)
        # Check if any Go file path contains the module name component
        if find . -name "*.go" -not -path "./vendor/*" 2>/dev/null | grep -qi "/${MODULE_NAME}"; then
            MAPPED=$((MAPPED + 1))
        fi
    done
    if [ "$TOTAL" -gt 0 ]; then
        PCT=$((MAPPED * 100 / TOTAL))
        if [ "$MAPPED" -eq "$TOTAL" ]; then
            pass "All $TOTAL modules have matching Go implementations"
        else
            warn "$MAPPED/$TOTAL modules mapped to Go files (${PCT}%)"
        fi
    else
        pass "No architecture modules to map"
    fi
else
    warn "No canonical architecture modules directory found"
fi

# ---------------------------------------------------------------------------
# Package Documentation
# ---------------------------------------------------------------------------
echo ""
echo "--- Package Documentation ---"
if [ -f "go.mod" ]; then
    PKG_FILES_WITH_DOC=$(grep -rlE '//\s*Package\s+\w+\s+(implements|provides|defines|is|contains)' . --include="*.go" 2>/dev/null | wc -l | tr -d ' ')
    GO_PACKAGES=$(go list ./... 2>/dev/null | wc -l | tr -d ' ')
    if [ "$GO_PACKAGES" -gt 0 ] && [ "$PKG_FILES_WITH_DOC" -gt 0 ]; then
        pass "Package documentation found in $PKG_FILES_WITH_DOC files"
    else
        warn "No package-level doc comments found (// Package X implements...)"
    fi
else
    warn "Not in a Go module, skipping package doc check"
fi

# ---------------------------------------------------------------------------
# ADR Linkage
# ---------------------------------------------------------------------------
echo ""
echo "--- ADR Linkage ---"
if [ -d ".pi/architecture/decisions" ]; then
    ADR_COUNT=$(find .pi/architecture/decisions -name "*.md" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$ADR_COUNT" -gt 0 ]; then
        ADR_REFS=$(grep -rlE '//\s*ADR[-_]?\d+' . --include="*.go" 2>/dev/null | wc -l | tr -d ' ')
        if [ "$ADR_REFS" -gt 0 ]; then
            pass "ADR references found in $ADR_REFS Go files"
        else
            warn "No ADR references in code ($ADR_COUNT decisions exist)"
        fi
    else
        pass "No ADRs to link to"
    fi
else
    warn "No ADR directory (.pi/architecture/decisions) found"
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
