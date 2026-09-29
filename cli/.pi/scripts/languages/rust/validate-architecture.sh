#!/usr/bin/env bash
# ============================================================================
# validate-architecture.sh — Rust
#
# CWD-robust and workspace-aware. Handles two shapes in one pass:
#   * a single crate      — layers at src/<layer>/                (flat)
#   * a Cargo workspace   — members with bounded contexts:
#                           <crate>/src/<context>/<layer>/        (nested)
#
# The Rigorix OSS repo is a workspace (engine/mcp/cli/actions/server) whose
# members use bounded contexts, and the Guardian runner invokes this script
# from the workspace root (no src/ there). So: resolve the crate set from the
# workspace manifest when needed, never assume a crate-local src/.
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

# ---------------------------------------------------------------------------
# Resolve crate roots: a single crate (cwd has src/) or the workspace members.
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
    warn "No Cargo.toml + src/ found (skipping Rust architecture validation)"
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

# Count layer dirs across crates: flat src/<layer> AND nested src/<context>/<layer>.
layer_count() {
    local layer="$1" n=0 nested
    for c in "${CRATE_ROOTS[@]}"; do
        [ -d "$c/src/$layer" ] && n=$((n + 1))
        nested=$(find "$c/src" -mindepth 2 -maxdepth 2 -type d -name "$layer" 2>/dev/null | wc -l | tr -d ' ')
        n=$((n + nested))
    done
    echo "$n"
}

# ---------------------------------------------------------------------------
# Layer structure
# ---------------------------------------------------------------------------
echo "--- Layer Structure ---"
LAYERS_FOUND=0
for layer in domain application infrastructure; do
    LAYERS_FOUND=$((LAYERS_FOUND + $(layer_count "$layer")))
done
if [ "$LAYERS_FOUND" -ge 2 ]; then
    pass "Clean architecture layers detected ($LAYERS_FOUND across ${#CRATE_ROOTS[@]} crate(s))"
elif [ "$LAYERS_FOUND" -eq 1 ]; then
    warn "Partial layer structure found (1 layer)"
else
    RS_FILES=$(find "${SRC_DIRS[@]}" -name '*.rs' 2>/dev/null | wc -l | tr -d ' ')
    if [ "$RS_FILES" -gt 0 ]; then
        pass "Source files found ($RS_FILES files — flat/thin crate accepted)"
    else
        fail "No architectural layers found (no src/<layer>/ or src/<context>/<layer>/)"
    fi
fi

# ---------------------------------------------------------------------------
# Canonical references (module files)
# ---------------------------------------------------------------------------
echo ""
echo "--- Canonical References ---"
if [ -d ".pi/architecture/modules" ]; then
    MODULE_COUNT=$(find .pi/architecture/modules -name "*.md" 2>/dev/null | wc -l | tr -d ' ') || true
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
for c in "${CRATE_ROOTS[@]}"; do
    for dir in "$c/src/domain" "$c/src/models"; do
        [ -d "$dir" ] || continue
        m=$(grep -rlE '^[[:space:]]*(pub[[:space:]]+)?struct[[:space:]]' "$dir" 2>/dev/null | wc -l | tr -d ' ') || true
        DOMAIN_MODELS=$((DOMAIN_MODELS + m))
    done
    # Bounded contexts: <crate>/src/<context>/domain/
    DOMAIN_MODELS=$((DOMAIN_MODELS + $(find "$c/src" -mindepth 2 -maxdepth 2 -type d -name domain -exec grep -rlE '^[[:space:]]*(pub[[:space:]]+)?struct[[:space:]]' {} + 2>/dev/null | wc -l | tr -d ' ')))
done
if [ "$DOMAIN_MODELS" -gt 0 ]; then
    pass "Domain models found ($DOMAIN_MODELS files with struct definitions)"
else
    ALL_MODELS=$(grep -rlE '^[[:space:]]*(pub[[:space:]]+)?struct[[:space:]]' "${SRC_DIRS[@]}" 2>/dev/null | wc -l | tr -d ' ') || true
    if [ "$ALL_MODELS" -gt 0 ]; then
        pass "Struct definitions found in source ($ALL_MODELS files)"
    else
        warn "No struct definitions found"
    fi
fi

# ---------------------------------------------------------------------------
# Dependency direction (domain must not depend on infrastructure)
# ---------------------------------------------------------------------------
echo ""
echo "--- Dependency Direction ---"
DOMAIN_DIRS=()
for c in "${CRATE_ROOTS[@]}"; do
    [ -d "$c/src/domain" ] && DOMAIN_DIRS+=("$c/src/domain")
    while IFS= read -r d; do
        [ -n "$d" ] && DOMAIN_DIRS+=("$d")
    done < <(find "$c/src" -mindepth 2 -maxdepth 2 -type d -name domain 2>/dev/null)
done
if [ ${#DOMAIN_DIRS[@]} -gt 0 ]; then
    DOMAIN_DEPS=$(grep -rE 'use[[:space:]]+crate::([a-z_]+::)*infrastructure' "${DOMAIN_DIRS[@]}" 2>/dev/null | wc -l | tr -d ' ') || true
    if [ "$DOMAIN_DEPS" -eq 0 ]; then
        pass "Domain layer does not depend on infrastructure (${#DOMAIN_DIRS[@]} domain dirs)"
    else
        fail "Domain layer depends on infrastructure ($DOMAIN_DEPS violations)"
    fi
else
    warn "No domain/ directory (cannot check dependency direction)"
fi

# ---------------------------------------------------------------------------
# Error handling
# ---------------------------------------------------------------------------
echo ""
echo "--- Error Handling ---"
HAS_ERROR_CRATE=0
for c in "${CRATE_ROOTS[@]}"; do
    if [ -f "$c/Cargo.toml" ] && grep -qE '(thiserror|eyre|anyhow)' "$c/Cargo.toml" 2>/dev/null; then
        HAS_ERROR_CRATE=1
    fi
done
CUSTOM_ERRORS=$(grep -rE 'enum[[:space:]]+\w*Error' "${SRC_DIRS[@]}" 2>/dev/null | wc -l | tr -d ' ') || true
if [ "$HAS_ERROR_CRATE" -eq 1 ] || [ "$CUSTOM_ERRORS" -gt 0 ]; then
    pass "Custom error handling detected"
else
    warn "No custom error types found (consider thiserror, eyre, or anyhow)"
fi

# ---------------------------------------------------------------------------
# Trait definitions
# ---------------------------------------------------------------------------
echo ""
echo "--- Trait Definitions ---"
TRAITS=$(grep -rE '^[[:space:]]*(pub[[:space:]]+)?trait[[:space:]]+' "${SRC_DIRS[@]}" 2>/dev/null | wc -l | tr -d ' ') || true
if [ "$TRAITS" -gt 0 ]; then
    pass "Trait definitions found ($TRAITS interfaces)"
else
    warn "No trait definitions found (consider using traits for interfaces)"
fi

# ---------------------------------------------------------------------------
# Crate structure (lib.rs / main.rs)
# ---------------------------------------------------------------------------
echo ""
echo "--- Crate Structure ---"
WITH_ENTRY=0
for c in "${CRATE_ROOTS[@]}"; do
    if [ -f "$c/src/lib.rs" ] || [ -f "$c/src/main.rs" ]; then
        WITH_ENTRY=$((WITH_ENTRY + 1))
    fi
done
if [ "$WITH_ENTRY" -gt 0 ]; then
    pass "$WITH_ENTRY/${#CRATE_ROOTS[@]} crate(s) expose lib.rs or main.rs"
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
