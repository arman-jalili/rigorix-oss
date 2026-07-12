#!/usr/bin/env bash
# ============================================================================
# validate-integration.sh — Java (Spring Boot Integration)
# ============================================================================
set -euo pipefail

PASS_COUNT=0; ERRORS=(); WARNINGS=()
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; WARNINGS+=("$1"); }

echo "============================================"
echo "  Integration Validation (Java)"
echo "============================================"
echo ""

SRC_DIR="${1:-src/main/java}"
TEST_DIR="${2:-src/test/java}"

# ---------------------------------------------------------------------------
# Test Slice Correctness
# ---------------------------------------------------------------------------
echo "--- Test Slice Correctness ---"
if [ -d "$TEST_DIR" ]; then
    # Check for @SpringBootTest usage (no slice annotations)
    FULL_CONTEXT_TESTS=$(grep -r "@SpringBootTest" "$TEST_DIR" --include="*.java" 2>/dev/null | wc -l | tr -d ' ')
    # Check for slice test annotations
    WEB_MVC_TESTS=$(grep -r "@WebMvcTest" "$TEST_DIR" --include="*.java" 2>/dev/null | wc -l | tr -d ' ')
    DATA_JPA_TESTS=$(grep -r "@DataJpaTest" "$TEST_DIR" --include="*.java" 2>/dev/null | wc -l | tr -d ' ')
    JSON_TESTS=$(grep -r "@JsonTest" "$TEST_DIR" --include="*.java" 2>/dev/null | wc -l | tr -d ' ')
    REST_CLIENT_TESTS=$(grep -r "@RestClientTest" "$TEST_DIR" --include="*.java" 2>/dev/null | wc -l | tr -d ' ')

    TOTAL_SLICE_TESTS=$((WEB_MVC_TESTS + DATA_JPA_TESTS + JSON_TESTS + REST_CLIENT_TESTS))

    if [ "$FULL_CONTEXT_TESTS" -gt 0 ] || [ "$TOTAL_SLICE_TESTS" -gt 0 ]; then
        if [ "$TOTAL_SLICE_TESTS" -ge "$FULL_CONTEXT_TESTS" ]; then
            pass "Good test slice usage: $TOTAL_SLICE_TESTS slice tests, $FULL_CONTEXT_TESTS full context tests"
        else
            warn "Consider using slice tests (@WebMvcTest, @DataJpaTest) instead of @SpringBootTest where possible. $FULL_CONTEXT_TESTS full / $TOTAL_SLICE_TESTS slice."
        fi
    else
        warn "No Spring test annotations found in test directory"
    fi
else
    pass "No test directory found, skipping test slice check"
fi

# ---------------------------------------------------------------------------
# Spring Context Caching
# ---------------------------------------------------------------------------
echo ""
echo "--- Context Caching ---"
if [ -d "$TEST_DIR" ]; then
    DIRTY_CONTEXT=$(grep -r "@DirtiesContext" "$TEST_DIR" --include="*.java" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$DIRTY_CONTEXT" -gt 0 ]; then
        warn "$DIRTY_CONTEXT @DirtiesContext annotations found — these disable context caching and slow tests"
    else
        pass "No @DirtiesContext found (context caching enabled)"
    fi
else
    pass "No test directory found, skipping context caching check"
fi

# ---------------------------------------------------------------------------
# Database State Management
# ---------------------------------------------------------------------------
echo ""
echo "--- Database State Management ---"
if [ -d "$TEST_DIR" ]; then
    DATA_JPA_CLASSES=$(grep -rl "@DataJpaTest" "$TEST_DIR" --include="*.java" 2>/dev/null || true)
    TRANSACTIONAL_TESTS=0
    if [ -n "$DATA_JPA_CLASSES" ]; then
        TRANSACTIONAL_TESTS=$(echo "$DATA_JPA_CLASSES" | wc -l | tr -d ' ')
    fi

    ROLLBACK_COUNT=$(grep -r "@Rollback" "$TEST_DIR" --include="*.java" 2>/dev/null | wc -l | tr -d ' ')

    if [ "$TRANSACTIONAL_TESTS" -gt 0 ] || [ "$ROLLBACK_COUNT" -gt 0 ]; then
        pass "Database state management found (@DataJpaTest: $TRANSACTIONAL_TESTS, @Rollback: $ROLLBACK_COUNT)"
    else
        warn "No explicit database state management found in tests (consider @DataJpaTest or @Rollback)"
    fi
fi

# ---------------------------------------------------------------------------
# Testcontainers (if applicable)
# ---------------------------------------------------------------------------
echo ""
echo "--- Testcontainers ---"
if [ -d "$TEST_DIR" ]; then
    TESTCONTAINERS=$(grep -r "testcontainers\|Testcontainers\|@Testcontainers\|@Container" "$TEST_DIR" --include="*.java" 2>/dev/null | head -10)
    if [ -n "$TESTCONTAINERS" ]; then
        pass "Testcontainers detected for integration testing"
    else
        warn "Testcontainers not detected — consider for database/external-service integration tests"
    fi
fi

# ---------------------------------------------------------------------------
# Mockito Usage
# ---------------------------------------------------------------------------
echo ""
echo "--- Mockito Usage ---"
if [ -d "$TEST_DIR" ]; then
    MOCKITO_ANNOTATIONS=$(grep -r "@Mock\|@InjectMocks\|@MockBean\|@SpyBean" "$TEST_DIR" --include="*.java" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$MOCKITO_ANNOTATIONS" -gt 0 ]; then
        pass "Mockito usage detected ($MOCKITO_ANNOTATIONS annotations)"
    else
        warn "No Mockito annotations found (@Mock, @InjectMocks, @MockBean)"
    fi
fi

# ---------------------------------------------------------------------------
# AssertJ / Assertions
# ---------------------------------------------------------------------------
echo ""
echo "--- Assertions ---"
if [ -d "$TEST_DIR" ]; then
    ASSERTJ=$(grep -r "assertThat\|Assertions.assertThat" "$TEST_DIR" --include="*.java" 2>/dev/null | wc -l | tr -d ' ')
    JUNIT_ASSERT=$(grep -r "assertEquals\|assertTrue\|assertNotNull\|assertThrows" "$TEST_DIR" --include="*.java" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$ASSERTJ" -gt 0 ] || [ "$JUNIT_ASSERT" -gt 0 ]; then
        pass "Assertions found (AssertJ: $ASSERTJ, JUnit: $JUNIT_ASSERT)"
    else
        warn "No assertion calls found in test directory"
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

echo -e "${GREEN}Integration validation passed.${NC}"
exit 0
