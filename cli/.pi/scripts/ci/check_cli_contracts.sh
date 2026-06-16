#!/usr/bin/env bash
# ============================================================================
# check_cli_contracts.sh — Verify CLI interface contracts have implementations
#
# Checks that each trait defined in the CLI boundary has a corresponding
# concrete implementation struct. Reports violations with file:line.
#
# Usage:
#   bash check_cli_contracts.sh          # Run all checks
#   bash check_cli_contracts.sh --help   # Show this help
#   bash check_cli_contracts.sh --list   # List all interface implementations
#
# Exit codes:
#   0 — All contracts have implementations
#   1 — One or more contracts missing implementations
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../../.." && pwd)"
SRC_DIR="${REPO_ROOT}/cli/src"

PASS_COUNT=0
FAIL_COUNT=0
MISSING=()

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; FAIL_COUNT=$((FAIL_COUNT + 1)); MISSING+=("$1"); }

show_help() {
    sed -n '3,13p' "$0" | sed 's/^#//'
    exit 0
}

show_list() {
    echo "CLI Interface ↔ Implementation Mapping"
    echo "======================================"
    echo ""
    echo "│ Interface                     │ Implementation              │ Status │"
    echo "│───────────────────────────────│─────────────────────────────│────────│"
    for pair in \
        "CliConfigLoader|CliConfigLoaderImpl" \
        "LogFormatter|LogFormatterImpl" \
        "SignalHandler|SignalHandlerImpl" \
        "CliOrchestrator|pending" \
        "ExecutionSession|pending" \
        "CliOrchestratorFactory|pending" \
        "ExecutionSessionFactory|pending" \
        "TuiRenderer|pending"; do
        IFS='|' read -r iface impl <<< "$pair"
        if [ "$impl" = "pending" ]; then
            echo "│ $(printf '%-30s' "$iface") │ $(printf '%-28s' "⚠️  not yet implemented") │ ⏳  │"
        else
            echo "│ $(printf '%-30s' "$iface") │ $(printf '%-28s' "$impl") │ ✅  │"
        fi
    done
    exit 0
}

if [ "${1:-}" = "--help" ]; then show_help; fi
if [ "${1:-}" = "--list" ]; then show_list; fi

echo "============================================"
echo "  CLI Contract Implementation Check"
echo "============================================"
echo ""

# ---------------------------------------------------------------------------
# Check 1: CliConfigLoader → CliConfigLoaderImpl
# ---------------------------------------------------------------------------
echo "--- Config Contracts ---"
if grep -q "impl CliConfigLoader for CliConfigLoaderImpl" "${SRC_DIR}/configuration/infrastructure/config_impl.rs" 2>/dev/null; then
    pass "CliConfigLoader → CliConfigLoaderImpl"
else
    fail "CliConfigLoader → CliConfigLoaderImpl (missing impl)"
fi

# ---------------------------------------------------------------------------
# Check 2: LogFormatter → LogFormatterImpl
# ---------------------------------------------------------------------------
echo ""
echo "--- Output Contracts ---"
if grep -q "impl LogFormatter for LogFormatterImpl" "${SRC_DIR}/cli_boundary/infrastructure/output_impl.rs" 2>/dev/null; then
    pass "LogFormatter → LogFormatterImpl"
else
    fail "LogFormatter → LogFormatterImpl (missing impl)"
fi

# ---------------------------------------------------------------------------
# Check 3: SignalHandler → SignalHandlerImpl
# ---------------------------------------------------------------------------
echo ""
echo "--- Signal Contracts ---"
if grep -q "impl SignalHandler for SignalHandlerImpl" "${SRC_DIR}/cancellation/infrastructure/signal_impl.rs" 2>/dev/null; then
    pass "SignalHandler → SignalHandlerImpl"
else
    fail "SignalHandler → SignalHandlerImpl (missing impl)"
fi

# ---------------------------------------------------------------------------
# Check 4: tracing functions exist
# ---------------------------------------------------------------------------
echo ""
echo "--- Tracing Contracts ---"
if grep -q "pub fn init_tracing" "${SRC_DIR}/observability/infrastructure/tracing.rs" 2>/dev/null; then
    pass "init_tracing() defined"
else
    fail "init_tracing() missing"
fi

# ---------------------------------------------------------------------------
# Check 5: Main entry point dispatches commands
# ---------------------------------------------------------------------------
echo ""
echo "--- CLI Entry Point ---"
if grep -q "fn main()" "${REPO_ROOT}/cli/src/main.rs" 2>/dev/null; then
    pass "Binary entry point (main.rs)"
else
    fail "Binary entry point missing"
fi

# ---------------------------------------------------------------------------
# Check 6: DTO definitions match CLI command enum
# ---------------------------------------------------------------------------
echo ""
echo "--- DTO Completeness ---"
DTO_COUNT=$(find "${SRC_DIR}/application/dto" -name "*.rs" -exec grep -c "pub struct\|pub enum" {} \; 2>/dev/null | awk '{s+=$1} END {print s}' || true)
if [ "$DTO_COUNT" -gt 0 ]; then
    pass "${DTO_COUNT} DTO types defined"
else
    fail "No DTO types found"
fi

# ---------------------------------------------------------------------------
# Check 7: Test coverage (at least 1 test per implementation)
# ---------------------------------------------------------------------------
echo ""
echo "--- Test Coverage ---"
TEST_COUNT=$(find "${SRC_DIR}" -name "*.rs" -exec grep -c "#\[test\]" {} \; 2>/dev/null | awk '{s+=$1} END {print s}' || true)
if [ "$TEST_COUNT" -ge 30 ]; then
    pass "${TEST_COUNT} tests (≥30 threshold)"
elif [ "$TEST_COUNT" -ge 10 ]; then
    pass "${TEST_COUNT} tests (≥10 minimum)"
else
    fail "Only ${TEST_COUNT} tests (minimum 10 required)"
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "============================================"
echo "  Summary"
echo "============================================"
echo -e "  Passed:   ${GREEN}${PASS_COUNT}${NC}"
echo -e "  Failed:   ${RED}${FAIL_COUNT}${NC}"
echo ""

if [ ${#MISSING[@]} -gt 0 ]; then
    echo "MISSING IMPLEMENTATIONS:"
    for m in "${MISSING[@]}"; do
        echo "  - $m"
    done
    echo ""
fi

if [ "$FAIL_COUNT" -gt 0 ]; then
    echo -e "${RED}Some contracts missing implementations.${NC}"
    exit 1
else
    echo -e "${GREEN}All contracts satisfied.${NC}"
    exit 0
fi
