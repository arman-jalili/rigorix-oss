#!/usr/bin/env bash
# ============================================================================
# validate-architecture.sh — Java (Spring Boot Clean Architecture)
# ============================================================================
set -euo pipefail

PASS_COUNT=0; ERRORS=(); WARNINGS=()
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; WARNINGS+=("$1"); }

echo "============================================"
echo "  Architecture Validation (Java)"
echo "============================================"
echo ""

SRC_DIR="${1:-src/main/java}"

ARCH_MODE="strict"
if [ -f "guardian-manifest.json" ]; then
	ARCH_MODE=$(jq -r '.archMode // "strict"' guardian-manifest.json 2>/dev/null || echo "strict")
fi

# ---------------------------------------------------------------------------
# Layer Structure
# ---------------------------------------------------------------------------
echo "--- Layer Structure ---"
if [ -d "$SRC_DIR" ]; then
    # Check for Clean Architecture layers
    HAS_DOMAIN=$(find "$SRC_DIR" -type d -name "domain" 2>/dev/null | head -1)
    HAS_APPLICATION=$(find "$SRC_DIR" -type d -name "application" 2>/dev/null | head -1)
    HAS_INFRASTRUCTURE=$(find "$SRC_DIR" -type d -name "infrastructure" 2>/dev/null | head -1)
    HAS_WEB=$(find "$SRC_DIR" -type d -name "web" -o -type d -name "interfaces" 2>/dev/null | head -1)

    LAYER_COUNT=0
    [ -n "$HAS_DOMAIN" ] && LAYER_COUNT=$((LAYER_COUNT + 1))
    [ -n "$HAS_APPLICATION" ] && LAYER_COUNT=$((LAYER_COUNT + 1))
    [ -n "$HAS_INFRASTRUCTURE" ] && LAYER_COUNT=$((LAYER_COUNT + 1))

    if [ "$LAYER_COUNT" -ge 3 ]; then
        pass "Clean Architecture layers present ($LAYER_COUNT of 3)"
    elif [ "$LAYER_COUNT" -ge 1 ]; then
        pass "Some architecture layers present ($LAYER_COUNT of 3)"
    else
        warn "No Clean Architecture layers found in $SRC_DIR"
    fi
else
    warn "No source directory found at $SRC_DIR"
fi

# ---------------------------------------------------------------------------
# Package Naming Convention
# ---------------------------------------------------------------------------
echo ""
echo "--- Package Naming ---"
if [ -d "$SRC_DIR" ]; then
    PACKAGE_DIRS=$(find "$SRC_DIR" -type d -mindepth 3 2>/dev/null | head -5)
    if [ -n "$PACKAGE_DIRS" ]; then
        BAD_NAMES=0
        for dir in $(find "$SRC_DIR" -type d 2>/dev/null); do
            BASENAME=$(basename "$dir")
            if echo "$BASENAME" | grep -qE '^[a-z]+$'; then
                : # valid package name
            elif [ "$BASENAME" = "java" ] || [ "$BASENAME" = "main" ] || [ "$BASENAME" = "src" ]; then
                : # standard directory names
            else
                BAD_NAMES=$((BAD_NAMES + 1))
            fi
        done
        if [ "$BAD_NAMES" -eq 0 ]; then
            pass "Package naming convention looks correct"
        else
            warn "Some directory names may not follow Java package conventions"
        fi
    else
        pass "No deep package structure to validate"
    fi
else
    warn "No source directory found"
fi

# ---------------------------------------------------------------------------
# Canonical References
# ---------------------------------------------------------------------------
echo ""
echo "--- Canonical References ---"
if [ -d ".pi/architecture/modules" ]; then
    MODULE_COUNT=$(find .pi/architecture/modules -name "*.md" 2>/dev/null | wc -l | tr -d ' ')
    pass "Canonical architecture documents found ($MODULE_COUNT modules)"
else
    warn "No .pi/architecture/modules directory found"
fi

# ---------------------------------------------------------------------------
# Dependency Direction
# ---------------------------------------------------------------------------
echo ""
echo "--- Dependency Direction ---"
if [ -n "$HAS_DOMAIN" ]; then
    # Domain must NOT import infrastructure or web
    DOMAIN_BAD_IMPORTS=0
    for f in $(find "$HAS_DOMAIN" -name "*.java" 2>/dev/null); do
        if grep -qE "import\s+.*\.infrastructure\." "$f" 2>/dev/null || grep -qE "import\s+.*\.web\." "$f" 2>/dev/null; then
            if [ "$ARCH_MODE" = "strict" ]; then
                fail "Domain layer imports from infrastructure/web: $f"
            else
                warn "Domain layer imports from infrastructure/web: $f (allowed in simplified mode)"
            fi
            DOMAIN_BAD_IMPORTS=$((DOMAIN_BAD_IMPORTS + 1))
        fi
    done
    if [ "$DOMAIN_BAD_IMPORTS" -eq 0 ]; then
        pass "Domain layer has no infrastructure or web imports"
    fi
else
    pass "No domain directory, skipping dependency direction check"
fi

# ---------------------------------------------------------------------------
# Spring Boot Specific
# ---------------------------------------------------------------------------
echo ""
echo "--- Spring Boot Structure ---"
SPRING_BOOT_APP=$(find "$SRC_DIR" -name "*Application.java" 2>/dev/null | head -1)
if [ -n "$SPRING_BOOT_APP" ]; then
    pass "Spring Boot main application class found: $(basename "$SPRING_BOOT_APP")"
else
    warn "No *Application.java Spring Boot entry point found"
fi

CONFIG_FILES=$(find . -name "application.yml" -o -name "application.yaml" -o -name "application.properties" 2>/dev/null | head -5)
if [ -n "$CONFIG_FILES" ]; then
    pass "Spring Boot configuration files found ($(echo "$CONFIG_FILES" | wc -l | tr -d ' ') files)"
else
    warn "No Spring Boot configuration files found (application.yml/properties)"
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

echo -e "${GREEN}Architecture validation passed.${NC}"
exit 0
