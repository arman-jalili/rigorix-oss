#!/usr/bin/env bash
# ============================================================================
# validate-ci.sh — Java (Maven/Gradle)
# ============================================================================
set -euo pipefail

PASS_COUNT=0; ERRORS=(); WARNINGS=()
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; WARNINGS+=("$1"); }

echo "============================================"
echo "  CI/MR Validation (Java)"
echo "============================================"
echo ""

# Detect build tool
BUILD_TOOL=""
if [ -f "pom.xml" ]; then
    BUILD_TOOL="maven"
elif [ -f "build.gradle" ] || [ -f "build.gradle.kts" ]; then
    BUILD_TOOL="gradle"
fi

if [ -z "$BUILD_TOOL" ]; then
    warn "No build file detected (pom.xml or build.gradle). Is this a Java project?"
fi

# Check Java availability
if ! command -v java &>/dev/null; then
    warn "Java runtime not found (install JDK 17+ for Spring Boot projects)"
fi

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------
echo "--- Build ---"
if [ -z "$BUILD_TOOL" ]; then
    pass "No build tool detected, skipping build"
elif [ "$BUILD_TOOL" = "maven" ] && command -v mvn &>/dev/null; then
    if mvn clean compile -q 2>/dev/null; then
        pass "Maven build succeeded (mvn clean compile)"
    else
        fail "Maven build failed"
    fi
elif [ "$BUILD_TOOL" = "gradle" ] && command -v gradle &>/dev/null; then
    if gradle build -q 2>/dev/null; then
        pass "Gradle build succeeded (gradle build)"
    else
        fail "Gradle build failed"
    fi
elif [ "$BUILD_TOOL" = "maven" ]; then
    if [ -f "mvnw" ]; then
        if ./mvnw clean compile -q 2>/dev/null; then
            pass "Maven Wrapper build succeeded (./mvnw clean compile)"
        else
            fail "Maven Wrapper build failed"
        fi
    else
        warn "Maven not found and no mvnw wrapper available, skipping build"
    fi
elif [ "$BUILD_TOOL" = "gradle" ]; then
    if [ -f "gradlew" ]; then
        if ./gradlew build -q 2>/dev/null; then
            pass "Gradle Wrapper build succeeded (./gradlew build)"
        else
            fail "Gradle Wrapper build failed"
        fi
    else
        warn "Gradle not found and no gradlew wrapper available, skipping build"
    fi
fi

# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------
echo ""
echo "--- Tests ---"
if [ -z "$BUILD_TOOL" ]; then
    pass "No build tool detected, skipping tests"
elif [ "$BUILD_TOOL" = "maven" ] && command -v mvn &>/dev/null; then
    if mvn test -q 2>/dev/null; then
        pass "All tests passed (mvn test)"
    else
        fail "Tests failed (mvn test)"
    fi
elif [ "$BUILD_TOOL" = "gradle" ] && command -v gradle &>/dev/null; then
    if gradle test -q 2>/dev/null; then
        pass "All tests passed (gradle test)"
    else
        fail "Tests failed (gradle test)"
    fi
elif [ "$BUILD_TOOL" = "maven" ] && [ -f "mvnw" ]; then
    if ./mvnw test -q 2>/dev/null; then
        pass "All tests passed (./mvnw test)"
    else
        fail "Tests failed (./mvnw test)"
    fi
elif [ "$BUILD_TOOL" = "gradle" ] && [ -f "gradlew" ]; then
    if ./gradlew test -q 2>/dev/null; then
        pass "All tests passed (./gradlew test)"
    else
        fail "Tests failed (./gradlew test)"
    fi
else
    warn "No build tool available to run tests"
fi

# ---------------------------------------------------------------------------
# Lint
# ---------------------------------------------------------------------------
echo ""
echo "--- Lint ---"
if [ -z "$BUILD_TOOL" ]; then
    pass "No build tool detected, skipping lint"
elif [ "$BUILD_TOOL" = "maven" ] && [ -f "pom.xml" ]; then
    if grep -q "checkstyle" pom.xml 2>/dev/null; then
        if command -v mvn &>/dev/null; then
            if mvn checkstyle:check -q 2>/dev/null; then
                pass "Checkstyle passed (mvn checkstyle:check)"
            else
                fail "Checkstyle failed"
            fi
        elif [ -f "mvnw" ]; then
            if ./mvnw checkstyle:check -q 2>/dev/null; then
                pass "Checkstyle passed (./mvnw checkstyle:check)"
            else
                fail "Checkstyle failed"
            fi
        fi
    else
        warn "No checkstyle plugin found in pom.xml"
    fi
elif [ "$BUILD_TOOL" = "gradle" ] && [ -f "build.gradle" ] || [ -f "build.gradle.kts" ]; then
    if command -v gradle &>/dev/null; then
        if gradle checkstyleMain -q 2>/dev/null; then
            pass "Checkstyle passed (gradle checkstyleMain)"
        else
            fail "Checkstyle failed"
        fi
    elif [ -f "gradlew" ]; then
        if ./gradlew checkstyleMain -q 2>/dev/null; then
            pass "Checkstyle passed (./gradlew checkstyleMain)"
        else
            fail "Checkstyle failed"
        fi
    else
        warn "Checkstyle not configured, skipping lint"
    fi
else
    warn "No linter configuration detected, skipping lint"
fi

# ---------------------------------------------------------------------------
# Format
# ---------------------------------------------------------------------------
echo ""
echo "--- Format ---"
if [ -z "$BUILD_TOOL" ]; then
    pass "No build tool detected, skipping format check"
elif [ "$BUILD_TOOL" = "maven" ]; then
    if command -v mvn &>/dev/null; then
        if mvn spotless:check -q 2>/dev/null; then
            pass "Format check passed (mvn spotless:check)"
        else
            fail "Format check failed (mvn spotless:check)"
        fi
    elif [ -f "mvnw" ]; then
        if ./mvnw spotless:check -q 2>/dev/null; then
            pass "Format check passed (./mvnw spotless:check)"
        else
            fail "Format check failed (./mvnw spotless:check)"
        fi
    else
        warn "Maven not available for format check, skipping"
    fi
elif [ "$BUILD_TOOL" = "gradle" ]; then
    if command -v gradle &>/dev/null; then
        if gradle spotlessCheck -q 2>/dev/null; then
            pass "Format check passed (gradle spotlessCheck)"
        else
            fail "Format check failed (gradle spotlessCheck)"
        fi
    elif [ -f "gradlew" ]; then
        if ./gradlew spotlessCheck -q 2>/dev/null; then
            pass "Format check passed (./gradlew spotlessCheck)"
        else
            fail "Format check failed (./gradlew spotlessCheck)"
        fi
    else
        warn "Gradle not available for format check, skipping"
    fi
fi

# ---------------------------------------------------------------------------
# Security Audit
# ---------------------------------------------------------------------------
echo ""
echo "--- Security Audit ---"
if [ -z "$BUILD_TOOL" ]; then
    pass "No build tool detected, skipping security audit"
elif [ "$BUILD_TOOL" = "maven" ]; then
    if command -v mvn &>/dev/null; then
        AUDIT_OUT=$(mvn dependency-check:check 2>&1 || true)
        if echo "$AUDIT_OUT" | grep -qi "no vulnerabilities found"; then
            pass "No vulnerabilities found"
        elif echo "$AUDIT_OUT" | grep -qiE "(critical|high)"; then
            fail "Security audit found critical/high vulnerabilities"
        else
            warn "Security audit reported findings (review manually)"
        fi
    elif [ -f "mvnw" ]; then
        AUDIT_OUT=$(./mvnw dependency-check:check 2>&1 || true)
        if echo "$AUDIT_OUT" | grep -qi "no vulnerabilities found"; then
            pass "No vulnerabilities found"
        elif echo "$AUDIT_OUT" | grep -qiE "(critical|high)"; then
            fail "Security audit found critical/high vulnerabilities"
        else
            warn "Security audit reported findings (review manually)"
        fi
    else
        warn "Maven not available for security audit"
    fi
elif [ "$BUILD_TOOL" = "gradle" ]; then
    if command -v gradle &>/dev/null; then
        AUDIT_OUT=$(gradle dependencyCheck 2>&1 || true)
        if echo "$AUDIT_OUT" | grep -qi "no vulnerabilities found"; then
            pass "No vulnerabilities found"
        elif echo "$AUDIT_OUT" | grep -qiE "(critical|high)"; then
            fail "Security audit found critical/high vulnerabilities"
        else
            warn "Security audit reported findings (review manually)"
        fi
    elif [ -f "gradlew" ]; then
        AUDIT_OUT=$(./gradlew dependencyCheck 2>&1 || true)
        if echo "$AUDIT_OUT" | grep -qi "no vulnerabilities found"; then
            pass "No vulnerabilities found"
        elif echo "$AUDIT_OUT" | grep -qiE "(critical|high)"; then
            fail "Security audit found critical/high vulnerabilities"
        else
            warn "Security audit reported findings (review manually)"
        fi
    else
        warn "Gradle not available for security audit"
    fi
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

echo -e "${GREEN}All CI checks passed.${NC}"
exit 0
