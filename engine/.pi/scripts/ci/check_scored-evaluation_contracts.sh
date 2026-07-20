#!/usr/bin/env bash
# ============================================================================
# check_scored-evaluation_contracts.sh
#
# Validates that every contract interface from the scored-evaluation module
# has a concrete implementation. Uses grep/find to detect trait definitions
# and their implementing structs.
#
# Usage: bash .pi/scripts/ci/check_scored-evaluation_contracts.sh [--help]
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

log_pass() { echo "  ✓ PASS: $1"; PASS=$((PASS + 1)); }
log_fail() { echo "  ✗ FAIL: $1"; ERRORS+=("$1"); FAIL=$((FAIL + 1)); }

if [ ! -d "$SRC_DIR" ]; then
    SRC_DIR="$(cd "$PI_DIR/.." && pwd)/src"
fi
if [ ! -d "$SRC_DIR" ]; then
    log_fail "Cannot find src directory"
    exit 1
fi

SE_DIR="$SRC_DIR/scored_evaluation"

if [ ! -d "$SE_DIR" ]; then
    log_fail "scored_evaluation module not found at $SE_DIR"
    exit 1
fi

echo ""
echo "═══ Scored Evaluation Contract Implementation Check ═══"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Domain layer exists
# ---------------------------------------------------------------------------
echo "--- 1. Domain Layer ---"
echo ""

if [ -f "$SE_DIR/domain/mod.rs" ]; then
    log_pass "domain/mod.rs exists"
else
    log_fail "domain/mod.rs missing"
fi

for file in backend.rs error.rs event.rs node.rs result.rs rubric.rs; do
    if [ -f "$SE_DIR/domain/$file" ]; then
        log_pass "domain/$file exists"
    else
        log_fail "domain/$file missing"
    fi
done

# Check ScoringBackend trait
if grep -q "trait ScoringBackend" "$SE_DIR/domain/backend.rs" 2>/dev/null; then
    log_pass "ScoringBackend trait defined"
else
    log_fail "ScoringBackend trait not found in domain/backend.rs"
fi

# Check ScoredEvaluationError has all 7 variants
ERROR_VARIANTS=$(grep -c '^\s\+[A-Z]' "$SE_DIR/domain/error.rs" 2>/dev/null || echo 0)
if [ "$ERROR_VARIANTS" -ge 7 ]; then
    log_pass "ScoredEvaluationError has 7+ variants"
else
    log_fail "ScoredEvaluationError has $ERROR_VARIANTS variants (expected 7+)"
fi

# Check is_retriable method
if grep -q "fn is_retriable" "$SE_DIR/domain/error.rs" 2>/dev/null; then
    log_pass "is_retriable() method defined on ScoredEvaluationError"
else
    log_fail "is_retriable() method missing from ScoredEvaluationError"
fi

# Check ScoredEvaluationEvent has 3 variants
EVENT_VARIANTS=$(grep -c '^\s\+[A-Z]' "$SE_DIR/domain/event.rs" 2>/dev/null || echo 0)
if [ "$EVENT_VARIANTS" -ge 3 ]; then
    log_pass "ScoredEvaluationEvent has 3+ variants"
else
    log_fail "ScoredEvaluationEvent has $EVENT_VARIANTS variants (expected 3+)"
fi

echo ""

# ---------------------------------------------------------------------------
# Check 2: Application layer exists
# ---------------------------------------------------------------------------
echo "--- 2. Application Layer ---"
echo ""

if [ -f "$SE_DIR/application/mod.rs" ]; then
    log_pass "application/mod.rs exists"
else
    log_fail "application/mod.rs missing"
fi

if [ -f "$SE_DIR/application/service.rs" ]; then
    log_pass "application/service.rs exists"
else
    log_fail "application/service.rs missing"
fi

if [ -f "$SE_DIR/application/service_impl.rs" ]; then
    log_pass "application/service_impl.rs exists"
else
    log_fail "application/service_impl.rs missing"
fi

if [ -f "$SE_DIR/application/dto.rs" ]; then
    log_pass "application/dto.rs exists"
else
    log_fail "application/dto.rs missing"
fi

# Check ScoredEvaluationService trait
if grep -q "trait ScoredEvaluationService" "$SE_DIR/application/service.rs" 2>/dev/null; then
    log_pass "ScoredEvaluationService trait defined"
else
    log_fail "ScoredEvaluationService trait not found"
fi

# Check ScoredEvaluationServiceImpl
if grep -q "impl ScoredEvaluationService for ScoredEvaluationServiceImpl" "$SE_DIR/application/service_impl.rs" 2>/dev/null; then
    log_pass "ScoredEvaluationServiceImpl implements ScoredEvaluationService"
else
    log_fail "ScoredEvaluationServiceImpl does not implement ScoredEvaluationService"
fi

echo ""

# ---------------------------------------------------------------------------
# Check 3: Infrastructure layer exists
# ---------------------------------------------------------------------------
echo "--- 3. Infrastructure Layer ---"
echo ""

if [ -f "$SE_DIR/infrastructure/mod.rs" ]; then
    log_pass "infrastructure/mod.rs exists"
else
    log_fail "infrastructure/mod.rs missing"
fi

if [ -f "$SE_DIR/infrastructure/repository.rs" ]; then
    log_pass "infrastructure/repository.rs exists"
else
    log_fail "infrastructure/repository.rs missing"
fi

if [ -f "$SE_DIR/infrastructure/repository_impl.rs" ]; then
    log_pass "infrastructure/repository_impl.rs exists"
else
    log_fail "infrastructure/repository_impl.rs missing"
fi

# Check backend implementations
for backend in mcp_backend.rs http_backend.rs local_backend.rs; do
    if [ -f "$SE_DIR/infrastructure/backends/$backend" ]; then
        log_pass "backends/$backend exists"
    else
        log_fail "backends/$backend missing"
    fi
done

# Check each backend implements ScoringBackend
for backend in McpBackend HttpBackend LocalBackend; do
    if grep -q "impl ScoringBackend for $backend" "$SE_DIR/infrastructure/backends/"*.rs 2>/dev/null; then
        log_pass "$backend implements ScoringBackend"
    else
        log_fail "$backend does not implement ScoringBackend"
    fi
done

echo ""

# ---------------------------------------------------------------------------
# Check 4: Cross-module contracts (PolicyCondition, AuditEnvelope)
# ---------------------------------------------------------------------------
echo "--- 4. Cross-Module Contracts ---"
echo ""

POLICY_DIR="$SRC_DIR/policy_engine/domain"
if [ -f "$POLICY_DIR/condition.rs" ]; then
    if grep -q "ScoreAbove" "$POLICY_DIR/condition.rs" 2>/dev/null; then
        log_pass "ScoreAbove condition defined in PolicyCondition"
    else
        log_fail "ScoreAbove condition missing from PolicyCondition"
    fi
    if grep -q "ScoreBelow" "$POLICY_DIR/condition.rs" 2>/dev/null; then
        log_pass "ScoreBelow condition defined in PolicyCondition"
    else
        log_fail "ScoreBelow condition missing from PolicyCondition"
    fi
else
    log_fail "policy_engine/domain/condition.rs not found"
fi

AUDIT_DIR="$SRC_DIR/audit/domain"
if [ -f "$AUDIT_DIR/envelope.rs" ]; then
    if grep -q "ScoringResultRef" "$AUDIT_DIR/envelope.rs" 2>/dev/null; then
        log_pass "ScoringResultRef defined in AuditEnvelope"
    else
        log_fail "ScoringResultRef missing from AuditEnvelope"
    fi
    if grep -q "scoring_results" "$AUDIT_DIR/envelope.rs" 2>/dev/null; then
        log_pass "scoring_results field defined in AuditEnvelope"
    else
        log_fail "scoring_results field missing from AuditEnvelope"
    fi
else
    log_fail "audit/domain/envelope.rs not found"
fi

echo ""

# ---------------------------------------------------------------------------
# Check 5: Module registered in lib.rs
# ---------------------------------------------------------------------------
echo "--- 5. Module Registration ---"
echo ""

LIB_RS="$SRC_DIR/lib.rs"
if [ -f "$LIB_RS" ]; then
    if grep -q "pub mod scored_evaluation" "$LIB_RS" 2>/dev/null; then
        log_pass "scored_evaluation module registered in lib.rs"
    else
        log_fail "scored_evaluation module NOT registered in lib.rs"
    fi
else
    log_fail "lib.rs not found"
fi

echo ""

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
    echo "Scored Evaluation contract check FAILED."
    exit 1
fi

echo "Scored Evaluation contract check PASSED."
exit 0
