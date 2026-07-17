#!/usr/bin/env bash
# ============================================================================
# validate-operations.sh — TypeScript
# ============================================================================
set -euo pipefail

PASS_COUNT=0; ERRORS=(); WARNINGS=()
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; WARNINGS+=("$1"); }

echo "============================================"
echo "  Operations Validation (TypeScript)"
echo "============================================"
echo ""

SRC_DIR="${1:-src}"

# ---------------------------------------------------------------------------
# Structured Logging
# ---------------------------------------------------------------------------
echo "--- Structured Logging ---"
LOGGING_LIBS=0
if [ -d "$SRC_DIR" ] || [ -d "." ]; then
    SEARCH_DIRS=""
    if [ -d "$SRC_DIR" ]; then
        SEARCH_DIRS="$SRC_DIR"
    fi
    SEARCH_DIRS="${SEARCH_DIRS} ${SEARCH_DIRS:+.}"

    for dir in $SEARCH_DIRS; do
        [ -d "$dir" ] || continue
        FOUND=$(find "$dir" -type f \( -name "*.ts" -o -name "*.tsx" \) -not -path "*/node_modules/*" \
            -exec grep -lE "(winston|pino|bunyan|console\.(log|info|warn|error))" {} + 2>/dev/null | wc -l | tr -d ' ')
        LOGGING_LIBS=$((LOGGING_LIBS + FOUND))
    done
    if [ "$LOGGING_LIBS" -gt 0 ]; then
        pass "Structured logging detected ($LOGGING_LIBS files)"
    else
        warn "No structured logging or console.log patterns found"
    fi
else
    warn "No source directory found, skipping logging check"
fi

# ---------------------------------------------------------------------------
# Health Checks
# ---------------------------------------------------------------------------
echo ""
echo "--- Health Checks ---"
HEALTH_ROUTES=0
if [ -d "$SRC_DIR" ]; then
    HEALTH_ROUTES=$(find "$SRC_DIR" -type f \( -name "*.ts" -o -name "*.tsx" \) -not -path "*/node_modules/*" \
        -exec grep -lE "(/health[^/]?|/healthz|/ready|/readiness)" {} + 2>/dev/null | wc -l | tr -d ' ')
    if [ "$HEALTH_ROUTES" -gt 0 ]; then
        pass "Health check endpoints found ($HEALTH_ROUTES files)"
    else
        warn "No health check endpoints detected (/health, /healthz, /ready)"
    fi
else
    warn "No source directory found, skipping health check check"
fi

# ---------------------------------------------------------------------------
# Graceful Shutdown
# ---------------------------------------------------------------------------
echo ""
echo "--- Graceful Shutdown ---"
SHUTDOWN_HANDLERS=0
if [ -d "$SRC_DIR" ]; then
    SHUTDOWN_HANDLERS=$(find "$SRC_DIR" -type f \( -name "*.ts" -o -name "*.tsx" \) -not -path "*/node_modules/*" \
        -exec grep -lE "process\.on\(['\"]SIG(TERM|INT)['\"]" {} + 2>/dev/null | wc -l | tr -d ' ')
    if [ "$SHUTDOWN_HANDLERS" -gt 0 ]; then
        pass "Graceful shutdown handlers found ($SHUTDOWN_HANDLERS files)"
    else
        warn "No SIGTERM/SIGINT handlers detected"
    fi
else
    warn "No source directory found, skipping shutdown check"
fi

# ---------------------------------------------------------------------------
# Metrics
# ---------------------------------------------------------------------------
echo ""
echo "--- Metrics ---"
METRICS_LIBS=0
if [ -d "$SRC_DIR" ]; then
    METRICS_LIBS=$(find "$SRC_DIR" -type f \( -name "*.ts" -o -name "*.tsx" \) -not -path "*/node_modules/*" \
        -exec grep -lE "(prom-client|@opentelemetry|statsd|hot-shots|dd-trace)" {} + 2>/dev/null | wc -l | tr -d ' ')
    if [ "$METRICS_LIBS" -gt 0 ]; then
        pass "Metrics/observability libraries detected ($METRICS_LIBS files)"
    else
        warn "No metrics libraries (prom-client, OpenTelemetry, statsd) detected"
    fi
else
    warn "No source directory found, skipping metrics check"
fi

# ---------------------------------------------------------------------------
# Error Handling
# ---------------------------------------------------------------------------
echo ""
echo "--- Error Handling ---"
ERROR_PATTERNS=0
if [ -d "$SRC_DIR" ]; then
    # Check for try/catch blocks
    TRY_CATCH=$(find "$SRC_DIR" -type f \( -name "*.ts" -o -name "*.tsx" \) -not -path "*/node_modules/*" \
        -exec grep -lE "try\s*\{" {} + 2>/dev/null | wc -l | tr -d ' ')
    # Check for error middleware (Express-style)
    ERROR_MIDDLEWARE=$(find "$SRC_DIR" -type f \( -name "*.ts" -o -name "*.tsx" \) -not -path "*/node_modules/*" \
        -exec grep -lE "error.*:.*Error.*req.*res" {} + 2>/dev/null | wc -l | tr -d ' ')
    ERROR_PATTERNS=$((TRY_CATCH + ERROR_MIDDLEWARE))
    if [ "$ERROR_PATTERNS" -gt 0 ]; then
        pass "Error handling patterns found ($ERROR_PATTERNS files with try/catch or error middleware)"
    else
        warn "No error handling patterns detected"
    fi
else
    warn "No source directory found, skipping error handling check"
fi

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------
echo ""
echo "--- Configuration ---"
CONFIG_FOUND=0
if [ -d "$SRC_DIR" ]; then
    CONFIG_FOUND=$(find "$SRC_DIR" -type f \( -name "*.ts" -o -name "*.tsx" \) -not -path "*/node_modules/*" \
        -exec grep -lE "(dotenv|require\(['\"]config['\"]\)|process\.env)" {} + 2>/dev/null | wc -l | tr -d ' ')
    if [ "$CONFIG_FOUND" -gt 0 ]; then
        pass "Configuration management detected ($CONFIG_FOUND files with dotenv/config/process.env)"
    else
        warn "No configuration management patterns detected (dotenv, config, process.env)"
    fi
else
    warn "No source directory found, skipping configuration check"
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
