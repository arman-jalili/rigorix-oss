#!/usr/bin/env bash
# ============================================================================
# validate-tests.sh — Java (Maven/Gradle)
# ============================================================================
set -euo pipefail

PASS_COUNT=0; ERRORS=(); WARNINGS=()
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; WARNINGS+=("$1"); }

echo "============================================"
echo "  Test Validation (Java)"
echo "============================================"
echo ""

# Detect build tool
BUILD_TOOL=""
if [ -f "pom.xml" ]; then
    BUILD_TOOL="maven"
elif [ -f "build.gradle" ] || [ -f "build.gradle.kts" ]; then
    BUILD_TOOL="gradle"
fi

# ---------------------------------------------------------------------------
# Unit Tests
# ---------------------------------------------------------------------------
echo "--- Unit Tests ---"
if [ -d "src/test" ] || [ -d "src/test/java" ]; then
    if [ -z "$BUILD_TOOL" ]; then
        warn "No build tool detected, skipping unit tests"
    elif [ "$BUILD_TOOL" = "maven" ] && command -v mvn &>/dev/null; then
        if mvn test -q -Dtest="*Test" 2>/dev/null; then
            pass "Unit tests passed (mvn test)"
        else
            fail "Unit tests failed (mvn test)"
        fi
    elif [ "$BUILD_TOOL" = "gradle" ] && command -v gradle &>/dev/null; then
        if gradle test --tests "*Test" -q 2>/dev/null; then
            pass "Unit tests passed (gradle test)"
        else
            fail "Unit tests failed (gradle test)"
        fi
    elif [ "$BUILD_TOOL" = "maven" ] && [ -f "mvnw" ]; then
        if ./mvnw test -q -Dtest="*Test" 2>/dev/null; then
            pass "Unit tests passed (./mvnw test)"
        else
            fail "Unit tests failed (./mvnw test)"
        fi
    elif [ "$BUILD_TOOL" = "gradle" ] && [ -f "gradlew" ]; then
        if ./gradlew test --tests "*Test" -q 2>/dev/null; then
            pass "Unit tests passed (./gradlew test)"
        else
            fail "Unit tests failed (./gradlew test)"
        fi
    else
        warn "No build tool executable available for unit tests"
    fi
else
    pass "No test directory (src/test) found, skipping tests"
fi

# ---------------------------------------------------------------------------
# Integration Tests
# ---------------------------------------------------------------------------
echo ""
echo "--- Integration Tests ---"
if [ -d "src/test" ]; then
    if [ -z "$BUILD_TOOL" ]; then
        pass "No build tool detected, skipping integration tests"
    elif [ "$BUILD_TOOL" = "maven" ] && command -v mvn &>/dev/null; then
        if mvn test -q -Dtest="*IT" 2>/dev/null; then
            pass "Integration tests passed (mvn test -Dtest=*IT)"
        else
            fail "Integration tests failed"
        fi
    elif [ "$BUILD_TOOL" = "gradle" ] && command -v gradle &>/dev/null; then
        if gradle test --tests "*IT" -q 2>/dev/null; then
            pass "Integration tests passed (gradle test --tests *IT)"
        else
            fail "Integration tests failed"
        fi
    elif [ "$BUILD_TOOL" = "maven" ] && [ -f "mvnw" ]; then
        if ./mvnw test -q -Dtest="*IT" 2>/dev/null; then
            pass "Integration tests passed (./mvnw test)"
        else
            fail "Integration tests failed"
        fi
    elif [ "$BUILD_TOOL" = "gradle" ] && [ -f "gradlew" ]; then
        if ./gradlew test --tests "*IT" -q 2>/dev/null; then
            pass "Integration tests passed (./gradlew test)"
        else
            fail "Integration tests failed"
        fi
    else
        warn "No build tool executable available for integration tests"
    fi
else
    pass "No src/test directory, skipping integration tests"
fi

# ---------------------------------------------------------------------------
# Coverage
# ---------------------------------------------------------------------------
echo ""
echo "--- Coverage ---"
if [ -n "$BUILD_TOOL" ]; then
    if [ -f "pom.xml" ] && grep -q "jacoco" pom.xml 2>/dev/null; then
        if command -v mvn &>/dev/null; then
            COVERAGE_OUTPUT=$(mvn jacoco:report -q 2>&1 || true)
            if [ -f "target/site/jacoco/index.html" ]; then
                COVERAGE_PCT=$(grep -oP 'Total.*?([0-9]+)%' target/site/jacoco/index.html 2>/dev/null | grep -oP '[0-9]+' | head -1 || echo "")
                if [ -n "$COVERAGE_PCT" ] && [ "$COVERAGE_PCT" -ge 80 ]; then
                    pass "Coverage: ${COVERAGE_PCT}% (≥ 80%)"
                elif [ -n "$COVERAGE_PCT" ]; then
                    fail "Coverage: ${COVERAGE_PCT}% (< 80%)"
                else
                    warn "Coverage report generated but percentage not detected"
                fi
            else
                warn "JaCoCo report not found at target/site/jacoco/"
            fi
        fi
    elif [ -f "build.gradle" ] || [ -f "build.gradle.kts" ]; then
        if grep -q "jacoco" build.gradle build.gradle.kts 2>/dev/null; then
            if command -v gradle &>/dev/null; then
                COVERAGE_OUTPUT=$(gradle jacocoTestReport -q 2>&1 || true)
                pass "Coverage report generated (check build/reports/jacoco/)"
            fi
        fi
    else
        warn "JaCoCo not configured in build file, skipping coverage"
    fi
else
    warn "No build tool detected, skipping coverage"
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

echo -e "${GREEN}Test validation passed.${NC}"
exit 0
