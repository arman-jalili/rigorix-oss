#!/usr/bin/env bash
# ============================================================================
# validate-canonical.sh — Python
# ============================================================================
set -euo pipefail

if [ -f "poetry.lock" ] && command -v poetry &>/dev/null; then
    POETRY_ENV=$(poetry env info --path 2>/dev/null || true)
    if [ -n "$POETRY_ENV" ] && [ -f "$POETRY_ENV/bin/activate" ]; then
        source "$POETRY_ENV/bin/activate"
    fi
fi

PASS_COUNT=0; ERRORS=(); WARNINGS=()
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; WARNINGS+=("$1"); }

echo "============================================"
echo "  Canonical Validation (Python)"
echo "============================================"
echo ""

SRC_DIR="${1:-src}"

# ── Architecture reference tracing ──
echo "--- Architecture Reference Tracing ---"
TOTAL_PY_FILES=$(find "$SRC_DIR" -name "*.py" 2>/dev/null | wc -l | tr -d ' ')
if [ "$TOTAL_PY_FILES" -gt 0 ]; then
    FILES_WITH_REFS=$(grep -rlE "(Canonical:|@canonical|Reference:.*\.pi/architecture)" "$SRC_DIR" --include="*.py" 2>/dev/null | wc -l | tr -d ' ')
    PCT=$((FILES_WITH_REFS * 100 / TOTAL_PY_FILES))
    if [ "$FILES_WITH_REFS" -gt 0 ]; then
        pass "Canonical references detected in $FILES_WITH_REFS/$TOTAL_PY_FILES files (${PCT}%)"
    else
        fail "No canonical reference comments found in source files"
    fi
else
    warn "No source files found to check"
fi

# ── Module-to-implementation mapping ──
echo ""
echo "--- Module-to-Implementation Mapping ---"
if [ -d ".pi/architecture/modules" ]; then
    MISSING=0
    for module_file in .pi/architecture/modules/*.md; do
        [ -f "$module_file" ] || continue
        # Extract component file paths from the module doc
        while IFS= read -r impl_path; do
            impl_path=$(echo "$impl_path" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')
            [ -z "$impl_path" ] && continue
            # Strip backticks and quotes
            impl_path=$(echo "$impl_path" | tr -d '`"' | sed "s/'//g")
            if [ -n "$impl_path" ]; then
                if [ ! -f "$impl_path" ]; then
                    warn "Architecture references $impl_path but file does not exist"
                    ((MISSING++))
                fi
            fi
        done < <(grep -oE '`[a-z_/]+\.py`' "$module_file" 2>/dev/null || true)
    done
    if [ "$MISSING" -eq 0 ]; then
        pass "All architecture-referenced implementation files exist"
    else
        warn "$MISSING referenced implementation file(s) missing"
    fi
else
    warn "No architecture modules directory found"
fi

# ── Package documentation ──
echo ""
echo "--- Package Documentation ---"
INIT_DOCS=0
INIT_WITHOUT_DOCS=0
for init_file in $(find "$SRC_DIR" -name "__init__.py" 2>/dev/null | head -20); do
    if grep -qE '"""|"""' "$init_file" 2>/dev/null; then
        ((INIT_DOCS++))
    else
        ((INIT_WITHOUT_DOCS++))
    fi
done
if [ "$INIT_DOCS" -gt 0 ]; then
    pass "Package docstrings found in $INIT_DOCS __init__.py files"
fi
if [ "$INIT_WITHOUT_DOCS" -gt 0 ]; then
    warn "$INIT_WITHOUT_DOCS __init__.py file(s) lack docstrings"
fi

# ── ADR linkage ──
echo ""
echo "--- ADR Linkage ---"
if [ -d ".pi/architecture/decisions" ]; then
    ADR_COUNT=$(find .pi/architecture/decisions -name "ADR-*.md" 2>/dev/null | wc -l | tr -d ' ')
    ADR_REFS=$(grep -rlE "ADR-[0-9]+" "$SRC_DIR" --include="*.py" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$ADR_REFS" -gt 0 ]; then
        pass "ADR references found in $ADR_REFS source files ($ADR_COUNT ADRs exist)"
    else
        warn "No ADR references found in source files (consider documenting architectural decisions)"
    fi
else
    warn "No ADR directory found at .pi/architecture/decisions/"
fi

# ── Summary ──
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
