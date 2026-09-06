#!/usr/bin/env bash
# ============================================================================
# check_sequence-policy_contracts.sh
#
# Validates that every contract interface from the sequence-policy module has
# a concrete implementation. Uses grep/find to detect trait definitions and
# their implementing structs — no frameworks, no dependencies.
#
# Usage: bash .pi/scripts/ci/check_sequence-policy_contracts.sh [--help]
#
# Exit codes: 0 = all contracts implemented, 1 = violations found
# ============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ENGINE_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
SRC_DIR="$ENGINE_ROOT/src"
SP_DIR="$SRC_DIR/sequence_policy"

PASS=0
FAIL=0
ERRORS=()

log_pass() { echo "  ✓ PASS: $1"; PASS=$((PASS + 1)); }
log_fail() { echo "  ✗ FAIL: $1"; ERRORS+=("$1"); FAIL=$((FAIL + 1)); }

if [ ! -d "$SP_DIR" ]; then
    echo "sequence-policy module not found at $SP_DIR" >&2
    exit 1
fi

echo ""
echo "═══ Sequence-Policy Contract Implementation Check ═══"
echo "Source: $SP_DIR"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Module registration
# ---------------------------------------------------------------------------
echo "--- Module Registration ---"

if grep -q 'pub mod sequence_policy;' "$SRC_DIR/lib.rs" 2>/dev/null; then
    log_pass "sequence_policy module registered in lib.rs"
else
    log_fail "pub mod sequence_policy; missing from src/lib.rs"
fi

# ---------------------------------------------------------------------------
# Check 2: Service Contracts (SequencePolicyService → SequencePolicyServiceImpl)
# ---------------------------------------------------------------------------
echo ""
echo "--- Service Contracts ---"

if grep -q 'pub trait SequencePolicyService' "$SP_DIR/application/service.rs" 2>/dev/null; then
    if grep -q 'impl SequencePolicyService for SequencePolicyServiceImpl' "$SP_DIR/application/service_impl.rs" 2>/dev/null; then
        log_pass "SequencePolicyService → SequencePolicyServiceImpl"
    else
        log_fail "SequencePolicyService trait has no implementation in service_impl.rs"
    fi
else
    log_fail "SequencePolicyService trait not found in application/service.rs"
fi

# R2 plan-time evaluation contract.
if grep -q 'async fn evaluate_plan' "$SP_DIR/application/service.rs" 2>/dev/null; then
    log_pass "evaluate_plan (R2 plan-time) declared"
else
    log_fail "evaluate_plan missing from the SequencePolicyService trait"
fi

# R3 run-time prefix contract.
if grep -q 'async fn evaluate_prefix' "$SP_DIR/application/service.rs" 2>/dev/null; then
    log_pass "evaluate_prefix (R3 run-time prefix) declared"
else
    log_fail "evaluate_prefix missing from the SequencePolicyService trait"
fi

# Factory contract.
if grep -q 'pub trait SequencePolicyFactory' "$SP_DIR/application/factory.rs" 2>/dev/null; then
    log_pass "SequencePolicyFactory trait defined"
else
    log_fail "SequencePolicyFactory trait not found"
fi

# ---------------------------------------------------------------------------
# Check 3: Repository Contracts (SequencePolicyRepository → TomlSequencePolicyRepository)
# ---------------------------------------------------------------------------
echo ""
echo "--- Repository Contracts ---"

if grep -q 'pub trait SequencePolicyRepository' "$SP_DIR/infrastructure/repository/mod.rs" 2>/dev/null; then
    if grep -q 'impl SequencePolicyRepository for TomlSequencePolicyRepository' "$SP_DIR/infrastructure/repository/toml_repository.rs" 2>/dev/null; then
        log_pass "SequencePolicyRepository → TomlSequencePolicyRepository"
    else
        log_fail "TomlSequencePolicyRepository does not implement SequencePolicyRepository"
    fi
else
    log_fail "SequencePolicyRepository trait not found in infrastructure/repository/mod.rs"
fi

# ---------------------------------------------------------------------------
# Check 4: Domain entities exist
# ---------------------------------------------------------------------------
echo ""
echo "--- Domain Entities ---"

for type in SequenceRule StepPredicate ParamPredicate ParamMatchKind RuleAction; do
    if grep -q "pub \(struct\|enum\) $type" "$SP_DIR/domain/rule.rs" 2>/dev/null; then
        log_pass "$type exists in domain/rule.rs"
    else
        log_fail "$type not found in domain/rule.rs"
    fi
done

if grep -q 'pub struct SequenceMatch' "$SP_DIR/domain/sequence_match.rs" 2>/dev/null; then
    log_pass "SequenceMatch exists"
else
    log_fail "SequenceMatch struct not found"
fi

for type in SequencePolicyConfig SafetyCaps; do
    if grep -q "pub struct $type" "$SP_DIR/domain/config.rs" 2>/dev/null; then
        log_pass "$type exists in domain/config.rs"
    else
        log_fail "$type not found in domain/config.rs"
    fi
done

if grep -q 'pub enum SequencePolicyError' "$SP_DIR/domain/error.rs" 2>/dev/null; then
    log_pass "SequencePolicyError enum exists"
else
    log_fail "SequencePolicyError enum not found"
fi

# ---------------------------------------------------------------------------
# Check 5: DTO contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- DTO Contracts ---"

for dto in PlannedStep DispatchedStep; do
    if grep -q "pub struct $dto" "$SP_DIR/application/dto/mod.rs" 2>/dev/null; then
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

STUBS=$(grep -rn 'todo!\|unimplemented!\|TODO: implemented in ISSUE' "$SP_DIR" --include="*.rs" 2>/dev/null | grep -v '^.*//' | head -10)
if [ -z "$STUBS" ]; then
    log_pass "no todo!/unimplemented! stubs remain in the sequence_policy module"
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
    echo "Some sequence-policy contracts are missing implementations."
    exit 1
fi

echo "All sequence-policy contracts have implementations."
exit 0
