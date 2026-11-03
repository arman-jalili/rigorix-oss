#!/usr/bin/env bash
# ============================================================================
# validate-security.sh — Java (Spring Boot Security)
# ============================================================================
set -euo pipefail

PASS_COUNT=0; ERRORS=(); WARNINGS=()
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; WARNINGS+=("$1"); }

echo "============================================"
echo "  Security Validation (Java)"
echo "============================================"
echo ""

SRC_DIR="${1:-src/main/java}"

# ---------------------------------------------------------------------------
# @PreAuthorize Coverage
# ---------------------------------------------------------------------------
echo "--- @PreAuthorize Coverage ---"
if [ -d "$SRC_DIR" ]; then
    CONTROLLER_FILES=$(find "$SRC_DIR" -name "*Controller.java" 2>/dev/null)
    if [ -n "$CONTROLLER_FILES" ]; then
        TOTAL_CONTROLLERS=$(echo "$CONTROLLER_FILES" | wc -l | tr -d ' ')
        PROTECTED_COUNT=0
        for f in $CONTROLLER_FILES; do
            if grep -q "@PreAuthorize\|@Secured\|@RolesAllowed" "$f" 2>/dev/null; then
                PROTECTED_COUNT=$((PROTECTED_COUNT + 1))
            fi
        done
        if [ "$PROTECTED_COUNT" -eq "$TOTAL_CONTROLLERS" ]; then
            pass "All $TOTAL_CONTROLLERS controllers have method-level security"
        elif [ "$PROTECTED_COUNT" -gt 0 ]; then
            warn "$PROTECTED_COUNT of $TOTAL_CONTROLLERS controllers have method-level security ($((TOTAL_CONTROLLERS - PROTECTED_COUNT)) unprotected)"
        else
            warn "No controllers have @PreAuthorize/@Secured annotations"
        fi
    else
        pass "No controllers found, skipping @PreAuthorize check"
    fi
else
    warn "No source directory found"
fi

# ---------------------------------------------------------------------------
# CSRF Configuration
# ---------------------------------------------------------------------------
echo ""
echo "--- CSRF Configuration ---"
SECURITY_CONFIG=$(find . -name "*SecurityConfig*" -o -name "*WebSecurityConfig*" -o -name "*SecurityConfiguration*" 2>/dev/null | head -5)
if [ -n "$SECURITY_CONFIG" ]; then
    CSRF_DISABLED=0
    for f in $SECURITY_CONFIG; do
        if grep -qi "csrf.*disable\|\.csrf()" "$f" 2>/dev/null; then
            CSRF_DISABLED=$((CSRF_DISABLED + 1))
        fi
    done
    if [ "$CSRF_DISABLED" -gt 0 ]; then
        warn "CSRF protection disabled in $CSRF_DISABLED config(s) — verify this is intentional (common for REST APIs)"
    else
        pass "CSRF protection not explicitly disabled (default enabled)"
    fi
else
    warn "No security configuration class found"
fi

# ---------------------------------------------------------------------------
# CORS Configuration
# ---------------------------------------------------------------------------
echo ""
echo "--- CORS Configuration ---"
if [ -n "$SECURITY_CONFIG" ]; then
    CORS_CONFIGURED=0
    for f in $SECURITY_CONFIG; do
        if grep -qi "cors\|\.cors()" "$f" 2>/dev/null; then
            CORS_CONFIGURED=$((CORS_CONFIGURED + 1))
        fi
    done
    if [ "$CORS_CONFIGURED" -gt 0 ]; then
        pass "CORS is explicitly configured in security config"
    else
        warn "CORS not explicitly configured (may use defaults)"
    fi
fi

# ---------------------------------------------------------------------------
# Dependency Vulnerabilities (Maven/Gradle)
# ---------------------------------------------------------------------------
echo ""
echo "--- Dependency Vulnerabilities ---"
if [ -f "pom.xml" ]; then
    if command -v mvn &>/dev/null; then
        pass "Maven project — run 'mvn dependency-check:check' for full audit"
    elif [ -f "mvnw" ]; then
        pass "Maven Wrapper project — run './mvnw dependency-check:check' for full audit"
    else
        warn "Maven project but neither mvn nor mvnw found"
    fi
elif [ -f "build.gradle" ] || [ -f "build.gradle.kts" ]; then
    if command -v gradle &>/dev/null; then
        pass "Gradle project — run 'gradle dependencyCheck' for full audit"
    elif [ -f "gradlew" ]; then
        pass "Gradle Wrapper project — run './gradlew dependencyCheck' for full audit"
    else
        warn "Gradle project but neither gradle nor gradlew found"
    fi
fi

# ---------------------------------------------------------------------------
# Sensitive Configuration
# ---------------------------------------------------------------------------
echo ""
echo "--- Sensitive Configuration ---"
SECRETS_FOUND=0
for pattern in "password" "secret" "api.key" "token" "jwt.secret"; do
    MATCHES=$(grep -rn "$pattern" application.yml application.yaml application.properties 2>/dev/null | grep -v "example\|placeholder\|#" || true)
    if [ -n "$MATCHES" ]; then
        SECRETS_FOUND=$((SECRETS_FOUND + 1))
    fi
done
if [ "$SECRETS_FOUND" -gt 0 ]; then
    warn "Sensitive configuration keys found (password/secret/token) — verify they use external secrets"
else
    pass "No hardcoded secrets detected in configuration files"
fi

# ---------------------------------------------------------------------------
# @Query Injection
# ---------------------------------------------------------------------------
echo ""
echo "--- @Query Safety ---"
if [ -d "$SRC_DIR" ]; then
    UNSAFE_QUERIES=0
    for f in $(find "$SRC_DIR" -name "*.java" 2>/dev/null); do
        if grep -qE "@Query\s*\(\s*\"SELECT" "$f" 2>/dev/null; then
            if grep -qE "#\{\s*.*param\}|:\s*.*Param|@Param" "$f" 2>/dev/null; then
                : # uses parameterized queries
            else
                warn "Potential unsafe query in $f (consider using @Param for parameterized queries)"
                UNSAFE_QUERIES=$((UNSAFE_QUERIES + 1))
            fi
        fi
    done
    if [ "$UNSAFE_QUERIES" -eq 0 ]; then
        pass "No obviously unsafe @Query patterns found"
    fi
else
    pass "No source directory found, skipping @Query check"
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

echo -e "${GREEN}Security validation passed.${NC}"
exit 0
