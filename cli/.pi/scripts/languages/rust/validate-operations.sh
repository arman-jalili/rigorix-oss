#!/usr/bin/env bash
# ============================================================================
# validate-operations.sh — Rust
# ============================================================================
set -euo pipefail

PASS_COUNT=0
ERRORS=()
WARNINGS=()

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; WARNINGS+=("$1"); }

echo "============================================"
echo "  Operations Validation (Rust)"
echo "============================================"
echo ""

if [ ! -f "Cargo.toml" ]; then
    warn "No Cargo.toml found (skipping Rust operations validation)"
    echo ""
    echo "============================================"
    echo "  Summary"
    echo "============================================"
    echo -e "  Passed:   ${GREEN}${PASS_COUNT}${NC}"
    echo -e "  Failed:   ${RED}${#ERRORS[@]}${NC}"
    echo ""
    echo -e "${GREEN}No Rust project detected, nothing to validate.${NC}"
    exit 0
fi

# ---------------------------------------------------------------------------
# Structured logging
# ---------------------------------------------------------------------------
echo "--- Structured Logging ---"
HAS_LOGGING=0
if grep -qE '^\s*(tracing|slog)\s*=' Cargo.toml 2>/dev/null; then
    HAS_LOGGING=1
    pass "Structured logging detected"
elif grep -qE '^\s*log\s*=' Cargo.toml 2>/dev/null; then
    HAS_LOGGING=1
    warn "Basic log crate detected (consider upgrading to tracing or slog for structured logging)"
fi
if [ "$HAS_LOGGING" -eq 0 ]; then
    warn "No logging crate found in Cargo.toml (tracing, slog, or log)"
fi

# ---------------------------------------------------------------------------
# Health checks
# ---------------------------------------------------------------------------
echo ""
echo "--- Health Checks ---"
HEALTH_FOUND=0
if grep -rqE '"/health"|"/healthz"' --include="*.rs" src/ 2>/dev/null; then
    HEALTH_FOUND=1
    pass "Health check endpoints detected (/health or /healthz)"
fi
if [ "$HEALTH_FOUND" -eq 0 ]; then
    warn "No health check endpoints found (consider adding /health or /healthz)"
fi

# ---------------------------------------------------------------------------
# Graceful shutdown
# ---------------------------------------------------------------------------
echo ""
echo "--- Graceful Shutdown ---"
SHUTDOWN_FOUND=0
if grep -rqE 'tokio::signal|ctrlc|impl\s+Drop' --include="*.rs" src/ 2>/dev/null; then
    SHUTDOWN_FOUND=1
    pass "Graceful shutdown mechanism detected"
fi
if [ "$SHUTDOWN_FOUND" -eq 0 ]; then
    warn "No graceful shutdown mechanism found (tokio::signal, ctrlc, or Drop)"
fi

# ---------------------------------------------------------------------------
# Metrics
# ---------------------------------------------------------------------------
echo ""
echo "--- Metrics ---"
if grep -qE '^\s*(prometheus|metrics|opentelemetry)\s*=' Cargo.toml 2>/dev/null; then
    pass "Metrics library detected"
else
    warn "No metrics library found in Cargo.toml (prometheus, metrics, or opentelemetry)"
fi

# ---------------------------------------------------------------------------
# Error handling
# ---------------------------------------------------------------------------
echo ""
echo "--- Error Handling ---"
if grep -qE '^\s*(thiserror|anyhow|eyre)\s*=' Cargo.toml 2>/dev/null; then
    pass "Error handling library detected"
else
    warn "No error handling library found in Cargo.toml (thiserror, anyhow, or eyre)"
fi

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------
echo ""
echo "--- Configuration ---"
CONFIG_FOUND=0
if grep -qE '^\s*(config|dotenvy)\s*=' Cargo.toml 2>/dev/null; then
    CONFIG_FOUND=1
    pass "Configuration library detected"
fi
if [ "$CONFIG_FOUND" -eq 0 ]; then
    # Check for std::env usage with defaults
    if grep -rqE 'std::env::var' --include="*.rs" src/ 2>/dev/null; then
        warn "Environment variables used without config library (consider config or dotenvy)"
    else
        warn "No configuration management detected"
    fi
fi

# ---------------------------------------------------------------------------
# Async runtime
# ---------------------------------------------------------------------------
echo ""
echo "--- Async Runtime ---"
if grep -qE '^\s*(tokio|async-std)\s*=' Cargo.toml 2>/dev/null; then
    pass "Async runtime detected"
else
    warn "No async runtime found in Cargo.toml (tokio or async-std)"
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

echo -e "${GREEN}Operations validation completed.${NC}"
exit 0
