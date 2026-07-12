#!/usr/bin/env bash
# ============================================================================
# validate-spring-architecture.sh — Spring Boot Package Ring Enforcement
# ============================================================================
#
# Package Ring Definitions (shared with validate-annotations.sh):
#   domain/        → Zero external dependencies (no imports outside domain)
#   application/   → Only depends on domain/
#   infrastructure/ → Depends on application/ and domain/ only
#   interfaces/    → Web layer, depends on application/ only
#
# Convention: com.{project}.{layer}.{component}
#   Example: com.myapp.domain.model, com.myapp.application.usecase
#
# Architecture mode: reads archMode from guardian-manifest.json
#   strict     — domain/ must have zero external deps (default)
#   simplified — domain/ may contain concrete providers (downgraded to warnings)
# ============================================================================
set -euo pipefail

PASS_COUNT=0; ERRORS=(); WARNINGS=()
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; WARNINGS+=("$1"); }

echo "============================================"
echo "  Spring Architecture Ring Validation"
echo "============================================"
echo ""

ARCH_MODE="strict"
if [ -f "guardian-manifest.json" ]; then
	ARCH_MODE=$(jq -r '.archMode // "strict"' guardian-manifest.json 2>/dev/null || echo "strict")
fi

echo "Architecture mode: ${ARCH_MODE}"
echo ""

SRC_DIR="${1:-src/main/java}"

if [ ! -d "$SRC_DIR" ]; then
    warn "No source directory found at $SRC_DIR"
    echo ""
    echo "============================================"
    echo "  Summary"
    echo "============================================"
    echo -e "  Passed:   ${GREEN}0${NC}"
    echo -e "  Failed:   ${RED}0${NC}"
    echo ""
    exit 0
fi

# ============================================================================
# Step 1: Detect package rings
# ============================================================================
echo "--- Layer Discovery ---"
DOMAIN_DIR=$(find "$SRC_DIR" -type d -name "domain" 2>/dev/null | head -1)
APP_DIR=$(find "$SRC_DIR" -type d -name "application" 2>/dev/null | head -1)
INFRA_DIR=$(find "$SRC_DIR" -type d -name "infrastructure" 2>/dev/null | head -1)
WEB_DIR=$(find "$SRC_DIR" -type d \( -name "web" -o -name "interfaces" -o -name "controller" \) 2>/dev/null | head -1)

LAYER_COUNT=0
[ -n "$DOMAIN_DIR" ] && LAYER_COUNT=$((LAYER_COUNT + 1)) && echo "  [✓] domain/"
[ -n "$APP_DIR" ] && LAYER_COUNT=$((LAYER_COUNT + 1)) && echo "  [✓] application/"
[ -n "$INFRA_DIR" ] && LAYER_COUNT=$((LAYER_COUNT + 1)) && echo "  [✓] infrastructure/"
[ -n "$WEB_DIR" ] && LAYER_COUNT=$((LAYER_COUNT + 1)) && echo "  [✓] interfaces/web/"

if [ "$LAYER_COUNT" -ge 3 ]; then
    pass "Clean Architecture layers detected ($LAYER_COUNT of 4)"
elif [ "$LAYER_COUNT" -ge 1 ]; then
    pass "Partial layers detected ($LAYER_COUNT of 4)"
else
    warn "No Clean Architecture layers detected"
fi

# ============================================================================
# Step 2: Domain ring — must have zero external imports
# ============================================================================
echo ""
echo "--- Domain Ring: No External Dependencies ---"
if [ -n "$DOMAIN_DIR" ]; then
    BAD_IMPORTS=0
    for f in $(find "$DOMAIN_DIR" -name "*.java" 2>/dev/null); do
        # Check for imports from outside domain
        while IFS= read -r line; do
            if echo "$line" | grep -qE "^(import\s+)([a-zA-Z0-9_.]+);$" 2>/dev/null; then
                # Extract the package root (first 2-3 segments)
                IMPORT_PKG=$(echo "$line" | sed 's/import \(.*\);/\1/' | cut -d'.' -f1-3 2>/dev/null)
                FILE_PKG=$(grep "^package " "$f" 2>/dev/null | sed 's/package \(.*\);/\1/' | cut -d'.' -f1-3 2>/dev/null)
                if [ -n "$IMPORT_PKG" ] && [ -n "$FILE_PKG" ] && [ "$IMPORT_PKG" != "$FILE_PKG" ]; then
                    # Check if import is from a different layer
                    IMPORT_LAYER=$(echo "$IMPORT_PKG" | rev | cut -d'.' -f1 | rev)
                    if [ "$IMPORT_LAYER" != "domain" ] && [ "$IMPORT_LAYER" != "model" ] && [ "$IMPORT_LAYER" != "validation" ]; then
                        # Skip Java/Spring standard library imports
                        if echo "$IMPORT_PKG" | grep -qE "^(java\.|javax\.|org\.springframework\.|org\.slf4j\.|com\.fasterxml\.)"; then
                            continue
                        fi
                        if [ "$ARCH_MODE" = "strict" ]; then
                            fail "Domain layer import from '$IMPORT_LAYER': ${f#./}"
                        else
                            warn "Domain layer import from '$IMPORT_LAYER': ${f#./} (allowed in simplified mode)"
                        fi
                        BAD_IMPORTS=$((BAD_IMPORTS + 1))
                    fi
                fi
            fi
        done < "$f"
    done
    if [ "$BAD_IMPORTS" -eq 0 ]; then
        pass "Domain ring has no external dependencies (clean)"
    fi
else
    pass "No domain ring detected, skipping"
fi

# ============================================================================
# Step 3: Application ring — only depends on domain
# ============================================================================
echo ""
echo "--- Application Ring: Only Depends on Domain ---"
if [ -n "$APP_DIR" ]; then
    BAD_IMPORTS=0
    for f in $(find "$APP_DIR" -name "*.java" 2>/dev/null); do
        while IFS= read -r line; do
            if echo "$line" | grep -qE "import\s+.*\.(infrastructure|web|interfaces|controller|repository)\." 2>/dev/null; then
                # Skip Java/Spring standard imports
                if echo "$line" | grep -qE "import\s+(java\.|javax\.|org\.springframework\.|org\.slf4j\.)"; then
                    continue
                fi
                fail "Application layer imports from non-domain layer: ${f#./}"
                echo "     $line"
                BAD_IMPORTS=$((BAD_IMPORTS + 1))
            fi
        done < "$f"
    done
    if [ "$BAD_IMPORTS" -eq 0 ]; then
        pass "Application ring only depends on domain (clean)"
    fi
else
    pass "No application ring detected, skipping"
fi

# ============================================================================
# Step 4: Infrastructure ring — depends on domain + application only
# ============================================================================
echo ""
echo "--- Infrastructure Ring: Only Depends on Domain + Application ---"
if [ -n "$INFRA_DIR" ]; then
    BAD_IMPORTS=0
    for f in $(find "$INFRA_DIR" -name "*.java" 2>/dev/null); do
        while IFS= read -r line; do
            if echo "$line" | grep -qE "import\s+.*\.(web|interfaces|controller)\." 2>/dev/null; then
                if echo "$line" | grep -qE "import\s+(java\.|javax\.|org\.springframework\.|org\.slf4j\.)"; then
                    continue
                fi
                fail "Infrastructure imports from web layer: ${f#./}"
                echo "     $line"
                BAD_IMPORTS=$((BAD_IMPORTS + 1))
            fi
        done < "$f"
    done
    if [ "$BAD_IMPORTS" -eq 0 ]; then
        pass "Infrastructure ring has no web layer imports (clean)"
    fi
else
    pass "No infrastructure ring detected, skipping"
fi

# ============================================================================
# Step 5: Web/Interfaces ring — only depends on application
# ============================================================================
echo ""
echo "--- Web Ring: Only Depends on Application ---"
if [ -n "$WEB_DIR" ]; then
    BAD_IMPORTS=0
    for f in $(find "$WEB_DIR" -name "*.java" 2>/dev/null); do
        while IFS= read -r line; do
            if echo "$line" | grep -qE "import\s+.*\.(infrastructure|repository)\." 2>/dev/null; then
                if echo "$line" | grep -qE "import\s+(java\.|javax\.|org\.springframework\.|org\.slf4j\.)"; then
                    continue
                fi
                fail "Web layer imports from infrastructure/repository: ${f#./}"
                echo "     $line"
                BAD_IMPORTS=$((BAD_IMPORTS + 1))
            fi
        done < "$f"
    done
    if [ "$BAD_IMPORTS" -eq 0 ]; then
        pass "Web ring has no infrastructure/repository imports (clean)"
    fi
else
    pass "No web ring detected, skipping"
fi

# ============================================================================
# Step 6: Dependency direction summary
# ============================================================================
echo ""
echo "--- Dependency Direction Summary ---"
echo ""
echo "  Domain/  → nothing outside domain"
echo "  Application/ → domain/ only"
echo "  Infrastructure/ → domain/ + application/"
echo "  Web/ → application/ only"
echo ""

# ============================================================================
# Summary
# ============================================================================
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

echo -e "${GREEN}Spring architecture validation passed.${NC}"
exit 0
