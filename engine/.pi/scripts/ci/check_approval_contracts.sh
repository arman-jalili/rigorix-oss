#!/usr/bin/env bash
# ============================================================================
# check_approval_contracts.sh
#
# Validates that every contract interface from the approval module has a
# concrete implementation. Uses grep/find to detect trait definitions and
# their implementing structs.
#
# Usage: bash .pi/scripts/ci/check_approval_contracts.sh [--help]
#
# Exit codes: 0 = all contracts implemented, 1 = violations found
# ============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ENGINE_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
SRC_DIR="$ENGINE_ROOT/src"
APPROVAL_DIR="$SRC_DIR/approval"

PASS=0
FAIL=0
ERRORS=()

log_pass() { echo "  ✓ PASS: $1"; PASS=$((PASS + 1)); }
log_fail() { echo "  ✗ FAIL: $1"; ERRORS+=("$1"); FAIL=$((FAIL + 1)); }

if [ ! -d "$APPROVAL_DIR" ]; then
    echo "Approval module not found at $APPROVAL_DIR" >&2
    exit 1
fi

echo ""
echo "═══ Approval Contract Implementation Check ═══"
echo "Source: $APPROVAL_DIR"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Module registration
# ---------------------------------------------------------------------------
echo "--- Module Registration ---"

if grep -q 'pub mod approval;' "$SRC_DIR/lib.rs" 2>/dev/null; then
    log_pass "approval module registered in lib.rs"
else
    log_fail "pub mod approval; missing from src/lib.rs"
fi

# ---------------------------------------------------------------------------
# Check 2: Service Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Service Contracts ---"

if grep -q 'pub trait ApprovalService' "$APPROVAL_DIR/application/service.rs" 2>/dev/null; then
    if grep -q 'impl ApprovalService for ApprovalServiceImpl' "$APPROVAL_DIR/application/service_impl.rs" 2>/dev/null; then
        log_pass "ApprovalService → ApprovalServiceImpl"
    else
        log_fail "ApprovalService trait has no implementation in service_impl.rs"
    fi
else
    log_fail "ApprovalService trait not found in application/service.rs"
fi

# Implementation-level support traits (wired by execution_engine / audit).
if grep -q 'pub trait NodeIntentResolver' "$APPROVAL_DIR/application/service.rs" 2>/dev/null; then
    log_pass "NodeIntentResolver support trait defined"
else
    log_fail "NodeIntentResolver support trait not found"
fi

if grep -q 'pub trait ScopeViolationSink' "$APPROVAL_DIR/application/service.rs" 2>/dev/null; then
    log_pass "ScopeViolationSink support trait defined"
else
    log_fail "ScopeViolationSink support trait not found"
fi

# ---------------------------------------------------------------------------
# Check 3: Repository Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Repository Contracts ---"

if grep -q 'pub trait ApprovalRepository' "$APPROVAL_DIR/infrastructure/repository/approval_repository.rs" 2>/dev/null; then
    if grep -A1 '^impl.*ApprovalRepository' "$APPROVAL_DIR/infrastructure/repository/approval_repository_impl.rs" 2>/dev/null | grep -q 'for InMemoryApprovalRepository'; then
        log_pass "ApprovalRepository → InMemoryApprovalRepository"
    else
        log_fail "InMemoryApprovalRepository does not implement ApprovalRepository"
    fi
    if grep -A1 '^impl.*ApprovalRepository' "$APPROVAL_DIR/infrastructure/repository/approval_repository_impl.rs" 2>/dev/null | grep -q 'for FileBackedApprovalRepository'; then
        log_pass "ApprovalRepository → FileBackedApprovalRepository"
    else
        log_fail "FileBackedApprovalRepository does not implement ApprovalRepository"
    fi
else
    log_fail "ApprovalRepository trait not found"
fi

# ---------------------------------------------------------------------------
# Check 4: Domain entities exist
# ---------------------------------------------------------------------------
echo ""
echo "--- Domain Entities ---"

if grep -q 'pub struct ExecutionIntent' "$APPROVAL_DIR/domain/intent.rs" 2>/dev/null; then
    log_pass "ExecutionIntent struct exists"
else
    log_fail "ExecutionIntent struct not found"
fi

if grep -q 'pub struct IntentHash' "$APPROVAL_DIR/domain/hash.rs" 2>/dev/null; then
    log_pass "IntentHash struct exists"
else
    log_fail "IntentHash struct not found"
fi

for type in ApprovalRecord DecisionContext ApprovalStatus; do
    if grep -q "pub \(struct\|enum\) $type" "$APPROVAL_DIR/domain/record.rs" 2>/dev/null; then
        log_pass "$type exists"
    else
        log_fail "$type not found in domain/record.rs"
    fi
done

if grep -q 'pub struct ScopeViolation' "$APPROVAL_DIR/domain/violation.rs" 2>/dev/null; then
    log_pass "ScopeViolation struct exists"
else
    log_fail "ScopeViolation struct not found"
fi

if grep -q 'pub enum ApprovalError' "$APPROVAL_DIR/domain/error.rs" 2>/dev/null; then
    log_pass "ApprovalError enum exists"
else
    log_fail "ApprovalError enum not found"
fi

# ---------------------------------------------------------------------------
# Check 5: DTO contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- DTO Contracts ---"

for dto in ApproveInput ApproveOutput; do
    if grep -q "pub struct $dto" "$APPROVAL_DIR/application/dto/mod.rs" 2>/dev/null; then
        log_pass "$dto DTO exists"
    else
        log_fail "$dto DTO not found"
    fi
done

# ---------------------------------------------------------------------------
# Check 6: No frozen stubs left (every contract implemented, nothing todo!-ed)
# ---------------------------------------------------------------------------
echo ""
echo "--- Implementation Completeness ---"

STUBS=$(grep -rn 'todo!\|unimplemented!\|TODO: implemented in ISSUE' "$APPROVAL_DIR" --include="*.rs" 2>/dev/null | grep -v '^.*//' | head -10)
if [ -z "$STUBS" ]; then
    log_pass "no todo!/unimplemented! stubs remain in the approval module"
else
    log_fail "stubs remain — implementation issues not complete"
    echo "$STUBS"
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
    echo "Some approval contracts are missing implementations."
    exit 1
fi

echo "All approval contracts have implementations."
exit 0
