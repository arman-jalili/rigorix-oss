#!/usr/bin/env bash
# ============================================================================
# validate-operations.sh — Python
# ============================================================================
set -euo pipefail

if [ -f "poetry.lock" ] && command -v poetry &>/dev/null; then
    POETRY_ENV=$(poetry env info --path 2>/dev/null || true)
    if [ -n "$POETRY_ENV" ] && [ -f "$POETRY_ENV/bin/activate" ]; then
        source "$POETRY_ENV/bin/activate"
    fi
fi

PASS_COUNT=0; ERRORS=(); WARNINGS=()
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; WARNINGS+=("$1"); }

echo "============================================"
echo "  Operations Validation (Python)"
echo "============================================"
echo ""

SRC_DIR="${1:-src}"

# ── Structured Logging ──
echo "--- Structured Logging ---"
if grep -rqE "(structlog|import logging|loguru|log\.info|log\.error|logger\.info|logger\.error)" "$SRC_DIR" --include="*.py" 2>/dev/null; then
    pass "Structured logging detected"
else
    warn "No structured logging framework detected"
fi

# ── Health Checks ──
echo ""
echo "--- Health Checks ---"
if grep -rqE "(/health|/healthz|healthcheck)" "$SRC_DIR" --include="*.py" 2>/dev/null; then
    pass "Health check endpoint(s) detected"
else
    warn "No health check endpoints found"
fi

# ── Graceful Shutdown ──
echo ""
echo "--- Graceful Shutdown ---"
if grep -rqE "signal\.(signal|SIGTERM|SIGINT)|atexit\.register|contextmanager" "$SRC_DIR" --include="*.py" 2>/dev/null; then
    pass "Graceful shutdown patterns detected"
else
    warn "No graceful shutdown patterns found"
fi

# ── Metrics ──
echo ""
echo "--- Metrics & Observability ---"
if grep -rqE "(prometheus_client|opentelemetry|datadog|statsd|metrics|counter|histogram|gauge)" "$SRC_DIR" --include="*.py" 2>/dev/null; then
    pass "Metrics framework detected"
else
    warn "No metrics framework detected"
fi

# ── Distributed Tracing ──
echo ""
echo "--- Distributed Tracing ---"
if grep -rqE "(opentelemetry|jaeger|zipkin|sentry_sdk|ddtrace)" "$SRC_DIR" --include="*.py" 2>/dev/null; then
    pass "Distributed tracing detected"
else
    warn "No distributed tracing detected"
fi

# ── Configuration Management ──
echo ""
echo "--- Configuration Management ---"
if grep -rqE "(pydantic\.BaseSettings|pydantic_settings|environ|os\.environ|dynaconf|python-dotenv)" "$SRC_DIR" --include="*.py" 2>/dev/null; then
    pass "Configuration management framework detected"
else
    warn "No configuration management detected (consider pydantic-settings or environs)"
fi

# ── Retry & Resilience ──
echo ""
echo "--- Retry & Resilience ---"
if grep -rqE "(tenacity|@retry|backoff|circuitbreaker)" "$SRC_DIR" --include="*.py" 2>/dev/null; then
    pass "Retry/resilience patterns detected"
else
    warn "No retry or circuit breaker patterns found"
fi

# ── Summary ──
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
