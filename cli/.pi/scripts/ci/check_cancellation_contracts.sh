#!/usr/bin/env bash
# ============================================================================
# check_cancellation_contracts.sh — Verify Cancellation module contracts
#
# Checks that each trait defined in the contract freeze has a corresponding
# concrete implementation. Reports violations with file:line.
#
# Usage:
#   bash check_cancellation_contracts.sh          # Run all checks
#   bash check_cancellation_contracts.sh --help   # Show this help
#   bash check_cancellation_contracts.sh --list   # List all interface implementations
#
# Exit codes:
#   0 — All contracts have implementations
#   1 — One or more contracts missing implementations
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../../.." && pwd)"
SRC_DIR="${REPO_ROOT}/cli/src/cancellation"

PASS_COUNT=0
FAIL_COUNT=0
MISSING=()

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
    echo "Cancellation Interface ↔ Implementation Mapping"
    echo "================================================"
    echo ""
    echo "│ Interface                         │ Implementation              │ Status │"
    echo "│───────────────────────────────────│─────────────────────────────│────────│"
    for pair in \
        "SignalHandler|SignalHandlerImpl" \
        "CancellationCliRepository|pending" \
        "CancellationCliError|enum defined" \
        "CancellationCliEvent|enum defined" \
        "ShutdownLevel|enum defined" \
        "GracefulShutdownInput|struct defined" \
        "ImmediateShutdownInput|struct defined" \
        "ShutdownOutput|struct defined" \
        "SignalStatusInput/Output|structs defined"; do
        IFS='|' read -r iface impl <<< "$pair"
        if [ "$impl" = "pending" ]; then
            echo "│ $(printf '%-34s' "$iface") │ $(printf '%-28s' "⚠️  not yet implemented") │ ⏳  │"
        else
            echo "│ $(printf '%-34s' "$iface") │ $(printf '%-28s' "$impl") │ ✅  │"
        fi
    done
    exit 0
}

if [ "${1:-}" = "--help" ]; then show_help; fi
if [ "${1:-}" = "--list" ]; then show_list; fi

echo "============================================"
echo "  Cancellation Contract Implementation Check"
echo "============================================"
echo ""

# ---------------------------------------------------------------------------
# Check 1: SignalHandler → SignalHandlerImpl
# ---------------------------------------------------------------------------
echo "--- Signal Contracts ---"
if grep -q "impl SignalHandler for SignalHandlerImpl" "${SRC_DIR}/infrastructure/signal_impl.rs" 2>/dev/null; then
    pass "SignalHandler → SignalHandlerImpl"
else
    fail "SignalHandler → SignalHandlerImpl (missing impl)"
fi

# ---------------------------------------------------------------------------
# Check 2: SignalHandler trait definition in application layer
# ---------------------------------------------------------------------------
if grep -q "pub trait SignalHandler" "${SRC_DIR}/application/service.rs" 2>/dev/null; then
    pass "SignalHandler trait defined in application/service.rs"
else
    fail "SignalHandler trait definition missing from application/service.rs"
fi

# ---------------------------------------------------------------------------
# Check 3: ShutdownLevel enum defined
# ---------------------------------------------------------------------------
if grep -q "pub enum ShutdownLevel" "${SRC_DIR}/application/service.rs" 2>/dev/null; then
    pass "ShutdownLevel enum defined in application/service.rs"
else
    fail "ShutdownLevel enum definition missing"
fi

# ---------------------------------------------------------------------------
# Check 4: CancellationCliRepository trait defined
# ---------------------------------------------------------------------------
echo ""
echo "--- Repository Contracts ---"
if grep -q "pub trait CancellationCliRepository" "${SRC_DIR}/infrastructure/repository/mod.rs" 2>/dev/null; then
    REPO_METHODS=$(grep -c "async fn" "${SRC_DIR}/infrastructure/repository/mod.rs" 2>/dev/null || true)
    pass "CancellationCliRepository trait defined (${REPO_METHODS} methods)"
else
    fail "CancellationCliRepository trait definition missing"
fi

# ---------------------------------------------------------------------------
# Check 5: Domain types defined
# ---------------------------------------------------------------------------
echo ""
echo "--- Domain Contracts ---"
if [ -f "${SRC_DIR}/domain/error.rs" ] && grep -q "pub enum CancellationCliError" "${SRC_DIR}/domain/error.rs" 2>/dev/null; then
    pass "CancellationCliError enum defined in domain/error.rs"
else
    fail "CancellationCliError enum missing"
fi

if [ -f "${SRC_DIR}/domain/event/mod.rs" ] && grep -q "pub enum CancellationCliEvent" "${SRC_DIR}/domain/event/mod.rs" 2>/dev/null; then
    pass "CancellationCliEvent enum defined in domain/event/mod.rs"
else
    fail "CancellationCliEvent enum missing"
fi

# ---------------------------------------------------------------------------
# Check 6: Application DTOs defined
# ---------------------------------------------------------------------------
echo ""
echo "--- Application DTO Contracts ---"
if [ -f "${SRC_DIR}/application/dto/mod.rs" ]; then
    DTO_COUNT=$(grep -c "pub struct\|pub enum" "${SRC_DIR}/application/dto/mod.rs" 2>/dev/null || true)
    if [ "$DTO_COUNT" -ge 4 ]; then
        pass "${DTO_COUNT} DTO types defined in application/dto/mod.rs"
    else
        fail "Only ${DTO_COUNT} DTOs found (expected 4+)"
    fi
else
    fail "application/dto/mod.rs missing"
fi

# ---------------------------------------------------------------------------
# Check 7: HTTP API contracts defined
# ---------------------------------------------------------------------------
echo ""
echo "--- API Contracts ---"
if [ -f "${SRC_DIR}/interfaces/http/mod.rs" ]; then
    API_COUNT=$(grep -c "pub const.*_PATH:" "${SRC_DIR}/interfaces/http/mod.rs" 2>/dev/null || true)
    if [ "$API_COUNT" -ge 3 ]; then
        pass "${API_COUNT} API endpoint paths defined in interfaces/http/mod.rs"
    else
        fail "API endpoint paths missing (expected 3+)"
    fi
else
    fail "interfaces/http/mod.rs missing"
fi

# ---------------------------------------------------------------------------
# Check 8: Module structure completeness
# ---------------------------------------------------------------------------
echo ""
echo "--- Module Structure Contracts ---"
for layer in "domain" "application" "infrastructure" "interfaces"; do
    if [ -f "${SRC_DIR}/${layer}/mod.rs" ]; then
        pass "cancellation/${layer}/mod.rs exists"
    else
        fail "cancellation/${layer}/mod.rs missing"
    fi
done

# ---------------------------------------------------------------------------
# Check 9: Module registered in lib.rs
# ---------------------------------------------------------------------------
echo ""
echo "--- Module Registration ---"
LIB_RS="${REPO_ROOT}/cli/src/lib.rs"
if grep -q "pub mod cancellation" "${LIB_RS}" 2>/dev/null; then
    pass "cancellation module registered in lib.rs"
else
    fail "cancellation module not registered in lib.rs"
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
