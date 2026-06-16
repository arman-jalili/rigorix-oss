#!/usr/bin/env bash
# ============================================================================
# check_cli_contracts.sh — Verify CLI interface contracts
#
# Checks that the CLI has the expected structure. The CLI is a thin binary
# with a single cli_boundary module calling rigorix-engine directly.
#
# Usage:
#   bash check_cli_contracts.sh          # Run all checks
#   bash check_cli_contracts.sh --help   # Show this help
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

if [ "${1:-}" = "--help" ]; then show_help; fi

echo "============================================"
echo "  CLI Contract Implementation Check"
echo "============================================"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Binary entry point
# ---------------------------------------------------------------------------
echo "--- Entry Point ---"
if [ -f "${SRC_DIR}/main.rs" ] && grep -q "fn main()" "${SRC_DIR}/main.rs" 2>/dev/null; then
    pass "main.rs entry point"
else
    fail "main.rs entry point missing"
fi

# ---------------------------------------------------------------------------
# Check 2: lib.rs declares cli_boundary
# ---------------------------------------------------------------------------
echo ""
echo "--- Module Structure ---"
if [ -f "${SRC_DIR}/lib.rs" ] && grep -q "pub mod cli_boundary" "${SRC_DIR}/lib.rs" 2>/dev/null; then
    pass "lib.rs declares cli_boundary module"
else
    fail "lib.rs missing cli_boundary declaration"
fi

# ---------------------------------------------------------------------------
# Check 3: No mirror modules (execution_engine, planning, etc.)
# ---------------------------------------------------------------------------
echo ""
echo "--- No Engine Mirror Modules ---"
MIRROR_COUNT=0
for mirror in execution_engine planning event_system state_persistence template_generation templates; do
    if [ -d "${SRC_DIR}/${mirror}" ]; then
        fail "Mirror module ${mirror}/ still exists"
        MIRROR_COUNT=$((MIRROR_COUNT + 1))
    fi
done
if [ "$MIRROR_COUNT" -eq 0 ]; then
    pass "No engine mirror modules"
fi

# ---------------------------------------------------------------------------
# Check 4: Clap command definitions
# ---------------------------------------------------------------------------
echo ""
echo "--- Command Definitions ---"
if grep -q "CliCommand" "${SRC_DIR}/cli_boundary/cli.rs" 2>/dev/null; then
    pass "CliCommand enum defined"
else
    fail "CliCommand missing from cli_boundary/cli.rs"
fi

# ---------------------------------------------------------------------------
# Check 5: Config loader
# ---------------------------------------------------------------------------
echo ""
echo "--- Config Loading ---"
if grep -q "pub fn load_config\|pub async fn load_config" "${SRC_DIR}/cli_boundary/config.rs" 2>/dev/null; then
    pass "Config loader function"
elif grep -q "pub fn load_config\|pub async fn load_config" "${SRC_DIR}/cli_boundary/config_impl.rs" 2>/dev/null; then
    pass "Config loader function (config_impl.rs)"
else
    fail "load_config function not found"
fi

# ---------------------------------------------------------------------------
# Check 6: Signal handler
# ---------------------------------------------------------------------------
echo ""
echo "--- Signal Handling ---"
if grep -q "Ctrl\|signal\|SIGINT\|SIGTERM" "${SRC_DIR}/cli_boundary/signal.rs" 2>/dev/null; then
    pass "Signal handling defined"
else
    fail "Signal handler missing"
fi

# ---------------------------------------------------------------------------
# Check 7: Tracing initialization
# ---------------------------------------------------------------------------
echo ""
echo "--- Tracing ---"
if grep -q "pub fn init_tracing\|pub fn init_logging" "${SRC_DIR}/cli_boundary/tracing.rs" 2>/dev/null; then
    pass "Tracing initialization"
else
    fail "Tracing init missing"
fi

# ---------------------------------------------------------------------------
# Check 8: Output formatter
# ---------------------------------------------------------------------------
echo ""
echo "--- Output Formatting ---"
if [ -f "${SRC_DIR}/cli_boundary/output.rs" ] || [ -f "${SRC_DIR}/cli_boundary/output_impl.rs" ]; then
    pass "Output formatter files exist"
else
    fail "Output formatter missing"
fi

# ---------------------------------------------------------------------------
# Check 9: Error type
# ---------------------------------------------------------------------------
echo ""
echo "--- Error Handling ---"
if grep -q "pub enum CliError\|CliError" "${SRC_DIR}/cli_boundary/error.rs" 2>/dev/null; then
    pass "CliError defined"
else
    fail "CliError missing"
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
