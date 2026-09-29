#!/usr/bin/env bash
# ============================================================================
# validate-canonical.sh — Rust
#
# CWD-robust and workspace-aware. Same crate resolution as
# validate-architecture.sh: a single crate (src/ at cwd) or the Cargo
# workspace members. Canonical docs are per-crate (<crate>/.pi/architecture/
# {modules,decisions}), so module mapping is checked against each crate's own
# src/, and the code markers (@canonical / ADR-NNN / module docs) are scanned
# across every crate.
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

# ---------------------------------------------------------------------------
# Resolve crate roots (single crate OR workspace members)
# ---------------------------------------------------------------------------
CRATE_ROOTS=()
if [ -d "src" ]; then
    CRATE_ROOTS=(".")
elif [ -f "Cargo.toml" ] && grep -qE '^[[:space:]]*\[workspace\]' Cargo.toml; then
    while IFS= read -r m; do
        [ -n "$m" ] && [ -d "$m/src" ] && CRATE_ROOTS+=("$m")
    done < <(awk '/^[[:space:]]*\[workspace\]/{f=1;next} /^[[:space:]]*\[/{f=0} f' Cargo.toml \
                | grep -oE '"[^"]+"' | tr -d '"' || true)
fi

if [ ${#CRATE_ROOTS[@]} -eq 0 ]; then
    warn "No Cargo.toml + src/ found (skipping Rust canonical validation)"
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

SRC_DIRS=()
for c in "${CRATE_ROOTS[@]}"; do
    SRC_DIRS+=("$c/src")
done

# ---------------------------------------------------------------------------
# Architecture reference tracing (code markers across all crates)
# ---------------------------------------------------------------------------
echo "--- Architecture Reference Tracing ---"
TOTAL_RS=$(find "${SRC_DIRS[@]}" -name '*.rs' 2>/dev/null | wc -l | tr -d ' ') || true
if [ "$TOTAL_RS" -gt 0 ]; then
    CANONICAL_REFS=$(grep -rE '(///[[:space:]]*Canonical:|//![[:space:]]*@canonical|///[[:space:]]*Reference:)' "${SRC_DIRS[@]}" 2>/dev/null | wc -l | tr -d ' ') || true
    if [ "$CANONICAL_REFS" -gt 0 ]; then
        PCT=$((CANONICAL_REFS * 100 / TOTAL_RS))
        pass "Canonical references found: ${CANONICAL_REFS} markers (${PCT}% of ${TOTAL_RS} Rust files)"
    else
        fail "No canonical references found in Rust doc comments"
    fi
else
    warn "No Rust source files in src/"
fi

# ---------------------------------------------------------------------------
# Module-to-implementation mapping — per architecture scope (root + crates)
# ---------------------------------------------------------------------------
echo ""
echo "--- Module-to-Implementation Mapping ---"
MAPPED_TOTAL=0
MODULES_TOTAL=0
SCOPES_FOUND=0
for scope in "." "${CRATE_ROOTS[@]}"; do
    mods_dir="$scope/.pi/architecture/modules"
    [ -d "$mods_dir" ] || continue
    if [ "$scope" = "." ]; then
        search_dirs=("${SRC_DIRS[@]}")
    else
        search_dirs=("$scope/src")
    fi
    scoped_modules=0
    scoped_mapped=0
    for mf in "$mods_dir"/*.md; do
        [ -e "$mf" ] || continue
        scoped_modules=$((scoped_modules + 1))
        MODULES_TOTAL=$((MODULES_TOTAL + 1))
        name=$(basename "$mf" .md)
        if find "${search_dirs[@]}" -name "*${name}*" -name '*.rs' 2>/dev/null | grep -q .; then
            scoped_mapped=$((scoped_mapped + 1))
            MAPPED_TOTAL=$((MAPPED_TOTAL + 1))
        fi
    done
    [ "$scoped_modules" -gt 0 ] && SCOPES_FOUND=$((SCOPES_FOUND + 1))
done
if [ "$MODULES_TOTAL" -gt 0 ] && [ "$MAPPED_TOTAL" -eq "$MODULES_TOTAL" ]; then
    pass "All $MODULES_TOTAL architecture modules mapped to implementation ($SCOPES_FOUND scope(s))"
elif [ "$MAPPED_TOTAL" -gt 0 ]; then
    pass "$MAPPED_TOTAL/$MODULES_TOTAL architecture modules mapped to implementation"
else
    fail "No architecture modules mapped to Rust implementation files"
fi

# ---------------------------------------------------------------------------
# Module documentation
# ---------------------------------------------------------------------------
echo ""
echo "--- Module Documentation ---"
if [ "$TOTAL_RS" -gt 0 ]; then
    MOD_DOCS=$(grep -rlE '^//!' "${SRC_DIRS[@]}" 2>/dev/null | wc -l | tr -d ' ') || true
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
# ADR linkage (root + per-crate decisions; code refs across all crates)
# ---------------------------------------------------------------------------
echo ""
echo "--- ADR Linkage ---"
ADR_FILES=0
for scope in "." "${CRATE_ROOTS[@]}"; do
    d="$scope/.pi/architecture/decisions"
    if [ -d "$d" ]; then
        n=$(find "$d" -name '*.md' 2>/dev/null | wc -l | tr -d ' ')
        ADR_FILES=$((ADR_FILES + n))
    fi
done
if [ "$ADR_FILES" -gt 0 ]; then
    ADR_REFS=$(grep -rE '///[[:space:]]*ADR-' "${SRC_DIRS[@]}" 2>/dev/null | wc -l | tr -d ' ') || true
    if [ "$ADR_REFS" -gt 0 ]; then
        pass "ADR references found in code ($ADR_REFS references across $ADR_FILES ADRs)"
    else
        warn "No ADR references in code (consider adding /// ADR-NNN comments)"
    fi
else
    warn "No ADR files found in .pi/architecture/decisions/ (root or crates)"
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
