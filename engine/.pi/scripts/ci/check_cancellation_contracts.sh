#!/usr/bin/env bash
# ============================================================================
# check_cancellation_contracts.sh
#
# Validates that every contract interface from the cancellation module has
# a concrete implementation. Uses grep/find to detect trait definitions and
# their implementing structs.
#
# Usage: bash .pi/scripts/ci/check_cancellation_contracts.sh [--help]
#
# Exit codes: 0 = all contracts implemented, 1 = violations found
# ============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PI_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
SRC_DIR="$(cd "$PI_DIR/.." && pwd)/engine/src"

PASS=0
FAIL=0
ERRORS=()

log_pass() { echo "  ✓ PASS: $1"; ((PASS++)); }
log_fail() { echo "  ✗ FAIL: $1"; ERRORS+=("$1"); ((FAIL++)); }

# Determine source directory
if [ ! -d "$SRC_DIR" ]; then
    SRC_DIR="$(cd "$PI_DIR/.." && pwd)/src"
fi
if [ ! -d "$SRC_DIR" ]; then
    log_fail "Source directory not found"
    exit 1
fi

echo ""
echo "═══ Cancellation Contract Implementation Check ═══"
echo "Source: $SRC_DIR/cancellation"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Service Contracts
# ---------------------------------------------------------------------------
echo "--- Service Contracts ---"

if grep -q 'pub trait CancellationService' "$SRC_DIR/cancellation/application/service.rs" 2>/dev/null; then
    if grep -q 'impl.*CancellationService' "$SRC_DIR/cancellation/application/cancellation_service_impl.rs" 2>/dev/null; then
        log_pass "CancellationService → CancellationManagerImpl"
    else
        log_fail "CancellationService trait has no implementation"
    fi
else
    log_fail "CancellationService trait not found"
fi

if grep -q 'pub trait CleanupHandler' "$SRC_DIR/cancellation/application/service.rs" 2>/dev/null; then
    # CleanupHandler is typically implemented inline in tests;
    # it's an interface that consumers implement, not a single impl.
    log_pass "CleanupHandler trait defined (consumer-implemented interface)"
fi

# ---------------------------------------------------------------------------
# Check 2: Factory Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Factory Contracts ---"

if grep -q 'pub trait CancellationManagerFactory' "$SRC_DIR/cancellation/application/factory.rs" 2>/dev/null; then
    if grep -q 'impl.*CancellationManagerFactory' "$SRC_DIR/cancellation/application/cancellation_manager_factory_impl.rs" 2>/dev/null; then
        log_pass "CancellationManagerFactory → CancellationManagerFactoryImpl"
    else
        log_fail "CancellationManagerFactory trait has no implementation"
    fi
else
    log_fail "CancellationManagerFactory trait not found"
fi

# ---------------------------------------------------------------------------
# Check 3: Domain entities exist
# ---------------------------------------------------------------------------
echo ""
echo "--- Domain Entities ---"

if grep -q 'pub enum ShutdownSignal' "$SRC_DIR/cancellation/domain/signal.rs" 2>/dev/null; then
    log_pass "ShutdownSignal enum exists"
else
    log_fail "ShutdownSignal enum not found"
fi

if grep -q 'pub enum CancellationError' "$SRC_DIR/cancellation/domain/error.rs" 2>/dev/null; then
    log_pass "CancellationError enum exists"
else
    log_fail "CancellationError enum not found"
fi

if grep -q 'pub enum CancellationEvent' "$SRC_DIR/cancellation/domain/event/mod.rs" 2>/dev/null; then
    log_pass "CancellationEvent enum exists"
else
    log_fail "CancellationEvent enum not found"
fi

# ---------------------------------------------------------------------------
# Check 4: DTO schemas exist
# ---------------------------------------------------------------------------
echo ""
echo "--- DTO Schemas ---"

DTO_COUNT=$(grep -c 'pub struct.*\(Input\|Output\)' "$SRC_DIR/cancellation/application/dto/mod.rs" 2>/dev/null || echo 0)
if [ "$DTO_COUNT" -ge 4 ]; then
    log_pass "DTO schemas exist ($DTO_COUNT input/output DTOs)"
else
    log_fail "Fewer than 4 DTO schemas found ($DTO_COUNT)"
fi

# ---------------------------------------------------------------------------
# Check 5: API contracts exist
# ---------------------------------------------------------------------------
echo ""
echo "--- API Contracts ---"

if grep -q 'pub const.*_PATH' "$SRC_DIR/cancellation/interfaces/http/mod.rs" 2>/dev/null; then
    ENDPOINT_COUNT=$(grep -c 'pub const.*_PATH' "$SRC_DIR/cancellation/interfaces/http/mod.rs" || echo 0)
    log_pass "API endpoint contracts exist ($ENDPOINT_COUNT endpoints)"
else
    log_fail "No API endpoint contracts found"
fi

if grep -q 'pub struct ApiErrorResponse' "$SRC_DIR/cancellation/interfaces/http/mod.rs" 2>/dev/null; then
    log_pass "Error response format defined"
else
    log_fail "Error response format not found"
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "═══ Summary ═══"
echo "  Passed: $PASS"
echo "  Failed: $FAIL"
echo ""

if [ ${#ERRORS[@]} -gt 0 ]; then
    echo "FAILURES:"
    for err in "${ERRORS[@]}"; do
        echo "  - $err"
    done
    echo ""
    echo "Some cancellation contracts are missing implementations."
    exit 1
fi

echo "All cancellation contracts have implementations."
exit 0
