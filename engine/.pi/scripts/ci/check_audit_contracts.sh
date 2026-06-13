#!/usr/bin/env bash
# ============================================================================
# check_audit_contracts.sh
#
# Validates that every contract interface from the audit module has
# a concrete implementation. Uses grep/find to detect trait definitions and
# their implementing structs.
#
# Usage: bash .pi/scripts/ci/check_audit_contracts.sh [--help]
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
echo "═══ Audit Contract Implementation Check ═══"
echo "Source: $SRC_DIR/audit"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Service Contracts
# ---------------------------------------------------------------------------
echo "--- Service Contracts ---"

if grep -q 'pub trait AuditService' "$SRC_DIR/audit/application/service.rs" 2>/dev/null; then
    if grep -q 'impl.*AuditService' "$SRC_DIR/audit/application/audit_service_impl.rs" 2>/dev/null; then
        log_pass "AuditService → AuditServiceImpl"
    else
        log_fail "AuditService trait has no implementation"
    fi
else
    log_fail "AuditService trait not found"
fi

if grep -q 'pub trait AuditSender' "$SRC_DIR/audit/application/service.rs" 2>/dev/null; then
    if grep -q 'impl.*AuditSender' "$SRC_DIR/audit/application/audit_sender_impl.rs" 2>/dev/null; then
        log_pass "AuditSender → AuditSenderImpl"
    else
        log_fail "AuditSender trait has no implementation"
    fi
else
    log_fail "AuditSender trait not found"
fi

if grep -q 'pub trait AuditQueue' "$SRC_DIR/audit/application/service.rs" 2>/dev/null; then
    if grep -q 'impl.*AuditQueue' "$SRC_DIR/audit/application/audit_queue_impl.rs" 2>/dev/null; then
        log_pass "AuditQueue → AuditQueueImpl"
    else
        log_fail "AuditQueue trait has no implementation"
    fi
else
    log_fail "AuditQueue trait not found"
fi

if grep -q 'pub trait CircuitBreaker' "$SRC_DIR/audit/application/service.rs" 2>/dev/null; then
    if grep -q 'impl.*CircuitBreaker' "$SRC_DIR/audit/application/circuit_breaker_impl.rs" 2>/dev/null; then
        log_pass "CircuitBreaker → CircuitBreakerImpl"
    else
        log_fail "CircuitBreaker trait has no implementation"
    fi
else
    log_fail "CircuitBreaker trait not found"
fi

# ---------------------------------------------------------------------------
# Check 2: Factory Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Factory Contracts ---"

if grep -q 'pub trait AuditEnvelopeFactory' "$SRC_DIR/audit/application/factory.rs" 2>/dev/null; then
    if grep -q 'impl.*AuditEnvelopeFactory' "$SRC_DIR/audit/application/envelope_factory_impl.rs" 2>/dev/null; then
        log_pass "AuditEnvelopeFactory → AuditEnvelopeFactoryImpl"
    else
        log_fail "AuditEnvelopeFactory trait has no implementation"
    fi
else
    log_fail "AuditEnvelopeFactory trait not found"
fi

if grep -q 'pub trait CircuitBreakerFactory' "$SRC_DIR/audit/application/factory.rs" 2>/dev/null; then
    if grep -q 'impl.*CircuitBreakerFactory' "$SRC_DIR/audit/application/circuit_breaker_factory_impl.rs" 2>/dev/null; then
        log_pass "CircuitBreakerFactory → CircuitBreakerFactoryImpl"
    else
        log_fail "CircuitBreakerFactory trait has no implementation"
    fi
else
    log_fail "CircuitBreakerFactory trait not found"
fi

# ---------------------------------------------------------------------------
# Check 3: Repository Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Repository Contracts ---"

if grep -q 'pub trait AuditEnvelopeRepository' "$SRC_DIR/audit/infrastructure/repository/mod.rs" 2>/dev/null; then
    if grep -q 'impl.*AuditEnvelopeRepository' "$SRC_DIR/audit/infrastructure/local_audit_repository.rs" 2>/dev/null; then
        log_pass "AuditEnvelopeRepository → LocalAuditEnvelopeRepository"
    else
        log_fail "AuditEnvelopeRepository trait has no implementation"
    fi
else
    log_fail "AuditEnvelopeRepository trait not found"
fi

# ---------------------------------------------------------------------------
# Check 4: Domain entities exist
# ---------------------------------------------------------------------------
echo ""
echo "--- Domain Entities ---"

if grep -q 'pub struct AuditEnvelope' "$SRC_DIR/audit/domain/envelope.rs" 2>/dev/null; then
    log_pass "AuditEnvelope struct exists"
else
    log_fail "AuditEnvelope struct not found"
fi

if grep -q 'pub enum AuditError' "$SRC_DIR/audit/domain/error.rs" 2>/dev/null; then
    log_pass "AuditError enum exists"
else
    log_fail "AuditError enum not found"
fi

if grep -q 'pub enum AuditEvent' "$SRC_DIR/audit/domain/event/mod.rs" 2>/dev/null; then
    log_pass "AuditEvent enum exists"
else
    log_fail "AuditEvent enum not found"
fi

if grep -q 'pub enum CircuitBreakerState' "$SRC_DIR/audit/domain/envelope.rs" 2>/dev/null; then
    log_pass "CircuitBreakerState enum exists"
else
    log_fail "CircuitBreakerState enum not found"
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
    echo "Some audit contracts are missing implementations."
    exit 1
fi

echo "All audit contracts have implementations."
exit 0
