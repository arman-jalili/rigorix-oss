#!/usr/bin/env bash
# ============================================================================
# validate-operations.sh — Go
# ============================================================================
set -euo pipefail

PASS_COUNT=0; ERRORS=(); WARNINGS=()
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; WARNINGS+=("$1"); }

echo "============================================"
echo "  Operations Validation (Go)"
echo "============================================"
echo ""

# ---------------------------------------------------------------------------
# Structured Logging
# ---------------------------------------------------------------------------
echo "--- Structured Logging ---"
if [ -f "go.mod" ]; then
    LOG_FRAMEWORKS=""
    grep -qE '"log/slog"' . -r --include="*.go" 2>/dev/null && LOG_FRAMEWORKS="${LOG_FRAMEWORKS:+$LOG_FRAMEWORKS, }slog"
    grep -qE '"go.uber.org/zap"' . -r --include="*.go" 2>/dev/null && LOG_FRAMEWORKS="${LOG_FRAMEWORKS:+$LOG_FRAMEWORKS, }zap"
    grep -qE '"github.com/sirupsen/logrus"' . -r --include="*.go" 2>/dev/null && LOG_FRAMEWORKS="${LOG_FRAMEWORKS:+$LOG_FRAMEWORKS, }logrus"
    grep -qE '"github.com/rs/zerolog"' . -r --include="*.go" 2>/dev/null && LOG_FRAMEWORKS="${LOG_FRAMEWORKS:+$LOG_FRAMEWORKS, }zerolog"
    if [ -n "$LOG_FRAMEWORKS" ]; then
        pass "Structured logging found ($LOG_FRAMEWORKS)"
    else
        warn "No structured logging framework detected (consider slog, zap, logrus, or zerolog)"
    fi
else
    warn "Not in a Go module, skipping logging check"
fi

# ---------------------------------------------------------------------------
# Health Checks
# ---------------------------------------------------------------------------
echo ""
echo "--- Health Checks ---"
if [ -f "go.mod" ]; then
    HEALTH_ENDPOINTS=$(grep -rE '("/health|/healthz|/ready|/readiness)' . --include="*.go" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$HEALTH_ENDPOINTS" -gt 0 ]; then
        pass "Health/readiness endpoint handlers found ($HEALTH_ENDPOINTS references)"
    else
        warn "No health check endpoints found (/health, /healthz, /ready)"
    fi
else
    pass "No Go source files to check"
fi

# ---------------------------------------------------------------------------
# Graceful Shutdown
# ---------------------------------------------------------------------------
echo ""
echo "--- Graceful Shutdown ---"
if [ -f "go.mod" ]; then
    SHUTDOWN_PATTERNS=0
    grep -qE 'signal\.Notify' . -r --include="*.go" 2>/dev/null && SHUTDOWN_PATTERNS=$((SHUTDOWN_PATTERNS + 1))
    grep -qE 'context\.WithCancel' . -r --include="*.go" 2>/dev/null && SHUTDOWN_PATTERNS=$((SHUTDOWN_PATTERNS + 1))
    grep -qE 'defer\s+.*cancel' . -r --include="*.go" 2>/dev/null && SHUTDOWN_PATTERNS=$((SHUTDOWN_PATTERNS + 1))
    if [ "$SHUTDOWN_PATTERNS" -gt 0 ]; then
        pass "Graceful shutdown patterns found ($SHUTDOWN_PATTERNS patterns detected)"
    else
        warn "No graceful shutdown patterns found (signal.Notify, context.WithCancel, defer cleanup)"
    fi
else
    pass "No Go source files to check"
fi

# ---------------------------------------------------------------------------
# Metrics
# ---------------------------------------------------------------------------
echo ""
echo "--- Metrics ---"
if [ -f "go.mod" ]; then
    METRICS_LIBS=""
    grep -qE '"github.com/prometheus' . -r --include="*.go" 2>/dev/null && METRICS_LIBS="${METRICS_LIBS:+$METRICS_LIBS, }prometheus"
    grep -qE '"go.opentelemetry.io' . -r --include="*.go" 2>/dev/null && METRICS_LIBS="${METRICS_LIBS:+$METRICS_LIBS, }opentelemetry"
    grep -qE '"expvar"' . -r --include="*.go" 2>/dev/null && METRICS_LIBS="${METRICS_LIBS:+$METRICS_LIBS, }expvar"
    if [ -n "$METRICS_LIBS" ]; then
        pass "Metrics/telemetry libraries found ($METRICS_LIBS)"
    else
        warn "No metrics libraries detected (consider prometheus, opentelemetry, or expvar)"
    fi
else
    warn "Not in a Go module, skipping metrics check"
fi

# ---------------------------------------------------------------------------
# Error Wrapping
# ---------------------------------------------------------------------------
echo ""
echo "--- Error Wrapping ---"
if [ -f "go.mod" ]; then
    WRAP_COUNT=$(grep -rE 'fmt\.Errorf\(.*%w' . --include="*.go" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$WRAP_COUNT" -gt 0 ]; then
        pass "Error wrapping with %w found ($WRAP_COUNT usages)"
    else
        warn "No error wrapping patterns found (use fmt.Errorf(\"... %w\", err))"
    fi
else
    pass "No Go source files to check"
fi

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------
echo ""
echo "--- Configuration ---"
if [ -f "go.mod" ]; then
    CONFIG_PATTERNS=0
    grep -qE '"github.com/spf13/viper"' . -r --include="*.go" 2>/dev/null && CONFIG_PATTERNS=$((CONFIG_PATTERNS + 1))
    grep -qE '"github.com/kelseyhightower/envconfig"' . -r --include="*.go" 2>/dev/null && CONFIG_PATTERNS=$((CONFIG_PATTERNS + 1))
    grep -qE 'os\.Getenv' . -r --include="*.go" 2>/dev/null && CONFIG_PATTERNS=$((CONFIG_PATTERNS + 1))
    if [ "$CONFIG_PATTERNS" -gt 0 ]; then
        pass "Configuration management patterns found ($CONFIG_PATTERNS approaches detected)"
    else
        warn "No configuration management patterns found (consider viper, envconfig, or os.Getenv with defaults)"
    fi
else
    warn "Not in a Go module, skipping configuration check"
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
