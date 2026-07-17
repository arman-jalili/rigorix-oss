#!/usr/bin/env bash
# ============================================================================
# validate-operations.sh — Java (Spring Boot Operations)
# ============================================================================
set -euo pipefail

PASS_COUNT=0; ERRORS=(); WARNINGS=()
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; WARNINGS+=("$1"); }

echo "============================================"
echo "  Operations Validation (Java)"
echo "============================================"
echo ""

SRC_DIR="${1:-src/main/java}"

# ---------------------------------------------------------------------------
# Actuator / Health Endpoints
# ---------------------------------------------------------------------------
echo "--- Actuator / Health Endpoints ---"
if [ -f "pom.xml" ] && grep -q "actuator" pom.xml 2>/dev/null; then
    pass "Spring Boot Actuator dependency found in pom.xml"
elif [ -f "build.gradle" ] && grep -q "actuator" build.gradle 2>/dev/null; then
    pass "Spring Boot Actuator dependency found in build.gradle"
elif [ -f "build.gradle.kts" ] && grep -q "actuator" build.gradle.kts 2>/dev/null; then
    pass "Spring Boot Actuator dependency found in build.gradle.kts"
else
    warn "Spring Boot Actuator not found — add spring-boot-starter-actuator for health checks and metrics"
fi

HEALTH_ENDPOINT=$(find "$SRC_DIR" -name "*HealthIndicator*.java" -o -name "*HealthCheck*.java" 2>/dev/null | head -5)
if [ -n "$HEALTH_ENDPOINT" ]; then
    pass "Custom health indicators found ($(echo "$HEALTH_ENDPOINT" | wc -l | tr -d ' ') files)"
else
    warn "No custom health indicators found (consider implementing HealthIndicator for readiness checks)"
fi

# ---------------------------------------------------------------------------
# @Scheduled Tasks
# ---------------------------------------------------------------------------
echo ""
echo "--- @Scheduled Tasks ---"
if [ -d "$SRC_DIR" ]; then
    SCHEDULED_COUNT=$(grep -r "@Scheduled" "$SRC_DIR" --include="*.java" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$SCHEDULED_COUNT" -gt 0 ]; then
        # Check if @EnableScheduling is present
        ENABLED_SCHEDULING=$(grep -r "@EnableScheduling" "$SRC_DIR" --include="*.java" 2>/dev/null | wc -l | tr -d ' ')
        if [ "$ENABLED_SCHEDULING" -gt 0 ]; then
            pass "$SCHEDULED_COUNT @Scheduled tasks found with @EnableScheduling"
        else
            warn "$SCHEDULED_COUNT @Scheduled tasks found but @EnableScheduling not found on any configuration"
        fi
    else
        pass "No @Scheduled tasks found"
    fi
fi

# ---------------------------------------------------------------------------
# @Async Tasks
# ---------------------------------------------------------------------------
echo ""
echo "--- @Async Tasks ---"
if [ -d "$SRC_DIR" ]; then
    ASYNC_COUNT=$(grep -r "@Async" "$SRC_DIR" --include="*.java" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$ASYNC_COUNT" -gt 0 ]; then
        ENABLED_ASYNC=$(grep -r "@EnableAsync" "$SRC_DIR" --include="*.java" 2>/dev/null | wc -l | tr -d ' ')
        if [ "$ENABLED_ASYNC" -gt 0 ]; then
            pass "$ASYNC_COUNT @Async methods found with @EnableAsync"
        else
            warn "$ASYNC_COUNT @Async methods found but @EnableAsync not found"
        fi
    else
        pass "No @Async methods found"
    fi
fi

# ---------------------------------------------------------------------------
# Observability (Micrometer / Tracing)
# ---------------------------------------------------------------------------
echo ""
echo "--- Observability ---"
if [ -f "pom.xml" ] && grep -q "micrometer\|micrometer-tracing" pom.xml 2>/dev/null; then
    pass "Micrometer observability found in pom.xml"
elif [ -f "build.gradle" ] && grep -q "micrometer\|micrometer-tracing" build.gradle 2>/dev/null; then
    pass "Micrometer observability found in build.gradle"
elif [ -f "build.gradle.kts" ] && grep -q "micrometer\|micrometer-tracing" build.gradle.kts 2>/dev/null; then
    pass "Micrometer observability found in build.gradle.kts"
else
    warn "Micrometer not found — consider adding micrometer-tracing for observability"
fi

# ---------------------------------------------------------------------------
# Logging Configuration
# ---------------------------------------------------------------------------
echo ""
echo "--- Logging Configuration ---"
LOG_CONFIG=$(find . -name "logback.xml" -o -name "logback-spring.xml" -o -name "log4j2.xml" -o -name "log4j2-spring.xml" 2>/dev/null | head -5)
if [ -n "$LOG_CONFIG" ]; then
    pass "Logging configuration found ($(echo "$LOG_CONFIG" | wc -l | tr -d ' ') files)"
else
    warn "No logging configuration file found (consider adding logback-spring.xml)"
fi

# ---------------------------------------------------------------------------
# Profile-Specific Configuration
# ---------------------------------------------------------------------------
echo ""
echo "--- Profile-Specific Configuration ---"
PROFILE_FILES=$(find . -name "application-*.yml" -o -name "application-*.yaml" -o -name "application-*.properties" 2>/dev/null | head -10)
if [ -n "$PROFILE_FILES" ]; then
    pass "Profile-specific configuration found ($(echo "$PROFILE_FILES" | wc -l | tr -d ' ') files)"
    for f in $PROFILE_FILES; do
        echo "    - $f"
    done
else
    warn "No profile-specific configuration files found (consider application-dev.yml, application-prod.yml)"
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

echo -e "${GREEN}Operations validation passed.${NC}"
exit 0
