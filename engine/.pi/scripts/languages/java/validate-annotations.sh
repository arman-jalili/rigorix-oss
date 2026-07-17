#!/usr/bin/env bash
# ============================================================================
# validate-annotations.sh — Spring Boot Annotation Enforcement
# ============================================================================
# Checks:
#   1. @Transactional on @Service public methods
#   2. @PostConstruct allowed only in service/config/component packages
#   3. Field injection (@Autowired on fields) — constructor injection required
#   4. Layering: web/ must not import repository/ directly
#   5. Package naming: enforce com.{project}.{layer}.{component}
# ============================================================================
set -euo pipefail

PASS_COUNT=0; ERRORS=(); WARNINGS=()
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; WARNINGS+=("$1"); }

echo "============================================"
echo "  Spring Annotation Validation"
echo "============================================"
echo ""

SRC_DIR="${1:-src/main/java}"

# ============================================================================
# Check 1: @Transactional on @Service public methods
# ============================================================================
echo "--- @Transactional Coverage ---"
if [ -d "$SRC_DIR" ]; then
    SERVICE_FILES=$(find "$SRC_DIR" -name "*.java" -exec grep -l "@Service" {} \; 2>/dev/null)
    if [ -n "$SERVICE_FILES" ]; then
        MISSING_TRANSACTIONAL=0
        for f in $SERVICE_FILES; do
            # Find public methods in @Service classes
            PUBLIC_METHODS=$(grep -c "public\s" "$f" 2>/dev/null || echo "0")
            TRANSACTIONAL_METHODS=$(grep -c "@Transactional" "$f" 2>/dev/null || echo "0")
            if [ "$PUBLIC_METHODS" -gt 0 ] && [ "$TRANSACTIONAL_METHODS" -eq 0 ]; then
                warn "Service class without @Transactional: $f ($PUBLIC_METHODS public methods, 0 @Transactional)"
                MISSING_TRANSACTIONAL=$((MISSING_TRANSACTIONAL + 1))
            fi
        done
        if [ "$MISSING_TRANSACTIONAL" -eq 0 ]; then
            pass "All @Service classes have @Transactional annotations"
        else
            fail "$MISSING_TRANSACTIONAL @Service classes missing @Transactional"
        fi
    else
        pass "No @Service classes found, skipping @Transactional check"
    fi
else
    pass "No source directory found, skipping @Transactional check"
fi

# ============================================================================
# Check 2: @PostConstruct placement
# ============================================================================
echo ""
echo "--- @PostConstruct Placement ---"
if [ -d "$SRC_DIR" ]; then
    PC_IN_CONTROLLER=$(grep -r "@PostConstruct" "$SRC_DIR" --include="*.java" -l 2>/dev/null | xargs grep -l "@RestController\|@Controller" 2>/dev/null || true)
    if [ -n "$PC_IN_CONTROLLER" ]; then
        for f in $PC_IN_CONTROLLER; do
            fail "@PostConstruct found in controller: $f (only allowed in service/config/component)"
        done
    else
        pass "No @PostConstruct found in controller classes"
    fi

    # Also check @PostConstruct is in valid packages
    PC_FILES=$(grep -r "@PostConstruct" "$SRC_DIR" --include="*.java" -l 2>/dev/null || true)
    if [ -n "$PC_FILES" ]; then
        for f in $PC_FILES; do
            if echo "$f" | grep -qE "(/service/|/config/|/component/)"; then
                : # Valid location
            else
                warn "@PostConstruct in unexpected package: $f (expected service/config/component)"
            fi
        done
    fi
else
    pass "No source directory found, skipping @PostConstruct check"
fi

# ============================================================================
# Check 3: Field injection (@Autowired on fields)
# ============================================================================
echo ""
echo "--- Field Injection Detection ---"
if [ -d "$SRC_DIR" ]; then
    FIELD_AUTOWIRED=$(grep -rn "@Autowired" "$SRC_DIR" --include="*.java" 2>/dev/null || true)
    CONSTRUCTOR_AUTOWIRED=$(grep -rn "@Autowired" "$SRC_DIR" --include="*.java" -B1 2>/dev/null | grep -E "\s*(public|protected|private)\s+\w+\(" || true)

    if [ -n "$FIELD_AUTOWIRED" ]; then
        # Count field-level @Autowired (not constructor)
        FIELD_COUNT=$(echo "$FIELD_AUTOWIRED" | wc -l | tr -d ' ')
        CONSTRUCTOR_COUNT=$(grep -rn "@Autowired\s*$" "$SRC_DIR" --include="*.java" 2>/dev/null | grep -v "//\|/\*" | wc -l | tr -d ' ')

        # Heuristic: more field injections than constructor injections
        if [ "$FIELD_COUNT" -gt "$CONSTRUCTOR_COUNT" ]; then
            fail "Field-level @Autowired found ($FIELD_COUNT occurrences) — prefer constructor injection"
        else
            warn "Field-level @Autowired found ($FIELD_COUNT occurrences) — review for constructor injection migration"
        fi
    else
        pass "No field-level @Autowired found (constructor injection used)"
    fi
else
    pass "No source directory found, skipping field injection check"
fi

# ============================================================================
# Check 4: Layering — web/ must not import repository/
# ============================================================================
echo ""
echo "--- Layering Violations ---"
WEB_DIRS=$(find "$SRC_DIR" -type d -name "web" -o -type d -name "controller" -o -type d -name "interfaces" 2>/dev/null)
if [ -n "$WEB_DIRS" ]; then
    VIOLATIONS=0
    for webdir in $WEB_DIRS; do
        REPO_IMPORTS=$(grep -r "import.*\.repository\.\|import.*\.infrastructure\." "$webdir" --include="*.java" 2>/dev/null || true)
        if [ -n "$REPO_IMPORTS" ]; then
            fail "Web layer imports repository/infrastructure directly:"
            echo "$REPO_IMPORTS"
            VIOLATIONS=$((VIOLATIONS + 1))
        fi
    done
    if [ "$VIOLATIONS" -eq 0 ]; then
        pass "Web layer has no direct repository imports (correct layering)"
    fi
else
    pass "No web/controller directory found, skipping layering check"
fi

# ============================================================================
# Check 5: Package naming convention
# ============================================================================
echo ""
echo "--- Package Naming Convention ---"
if [ -d "$SRC_DIR" ]; then
    BAD_PACKAGE_FILES=0
    for f in $(find "$SRC_DIR" -name "*.java" 2>/dev/null); do
        # Check if package declaration follows com.{project}.{layer}.{component}
        PACKAGE_DECL=$(grep "^package " "$f" 2>/dev/null || true)
        if [ -n "$PACKAGE_DECL" ]; then
            # Must start with a domain that has at least 3 parts (com.example.layer)
            if echo "$PACKAGE_DECL" | grep -qE "^package\s+[a-z][a-z0-9]*(\.[a-z][a-z0-9]*){2,};$"; then
                : # Good package structure: com.example.layer
            elif echo "$PACKAGE_DECL" | grep -qE "^package\s+org\.[a-z]|[a-z]+\.example"; then
                : # Common patterns: org.example, com.example
            else
                warn "Unusual package naming in $f: $PACKAGE_DECL"
                BAD_PACKAGE_FILES=$((BAD_PACKAGE_FILES + 1))
            fi
        fi
    done
    if [ "$BAD_PACKAGE_FILES" -eq 0 ]; then
        pass "Package naming conventions look correct"
    fi
else
    pass "No source directory found, skipping package naming check"
fi

# ============================================================================
# Summary
# ============================================================================
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

echo -e "${GREEN}Spring annotation validation passed.${NC}"
exit 0
