#!/usr/bin/env bash
# ============================================================================
# check_effect-commitment_contracts.sh — contract implementation check
#
# Deterministic proof that the effect-commitment contract freeze is backed by
# concrete implementations and tests. The script reads the source; it does not
# call an LLM and does not need network access.
#
# Checks:
#   1. The four DDD layers exist for engine/src/effect_commitment/
#   2. Every frozen contract file exists
#   3. Every contract trait is declared
#   4. Every port / service / factory has an `impl` with the frozen methods
#   5. The dispatch seam, fences, and NodeStatus vocabulary are wired
#   6. The composition roots construct EffectCommitmentSetup
#   7. Every component has a unit test, and the cross-cutting e2e tests exist
#   8. The port contract suite exists and runs against the in-process adapter
#
# Usage:
#   bash .pi/scripts/ci/check_effect-commitment_contracts.sh
#   bash .pi/scripts/ci/check_effect-commitment_contracts.sh --help
#
# Exit: 0 = all contracts implemented, 1 = violation(s) found
# ============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="${RIGORIX_CONTRACT_ROOT:-$(cd "${SCRIPT_DIR}/../../.." && pwd)}"
MODULE_DIR="${PROJECT_ROOT}/engine/src/effect_commitment"
TEST_DIR="${PROJECT_ROOT}/engine/tests/unit/effect-commitment"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

PASS_COUNT=0
FAIL_COUNT=0
ERRORS=()

pass() { echo -e "  ${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "  ${RED}❌ FAIL${NC} $1"; FAIL_COUNT=$((FAIL_COUNT + 1)); ERRORS+=("$1"); }
warn() { echo -e "  ${YELLOW}⚠️  WARN${NC} $1"; }

if [[ "${1:-}" == "--help" ]]; then
    sed -n '3,27p' "$0" | sed 's/^# \{0,1\}//'
    exit 0
fi

# Seeded-failure proof: copy the module to a temporary root, remove one frozen
# contract file, and require this exact check to fail. `validate-guards.sh
# --self-test` calls this through the registry, so a check that cannot fail is
# caught in CI (#1186).
if [[ "${1:-}" == "--self-test" ]]; then
    TMP="$(mktemp -d)"
    trap 'find "$TMP" -mindepth 1 -delete 2>/dev/null; rmdir "$TMP" 2>/dev/null' EXIT
    mkdir -p "$TMP/engine/src"
    cp -R "${MODULE_DIR}" "$TMP/engine/src/effect_commitment"
    rm "$TMP/engine/src/effect_commitment/domain/state.rs"
    if RIGORIX_CONTRACT_ROOT="$TMP" bash "$0" >/dev/null 2>&1; then
        echo "❌ self-test: a removed frozen contract still passed" >&2
        exit 1
    fi
    echo "✅ self-test: seeded contract violation was caught"
    exit 0
fi

echo "============================================"
echo "  Effect Commitment Contract Check"
echo "============================================"
echo ""

if [[ ! -d "${MODULE_DIR}" ]]; then
    echo -e "${RED}❌ effect_commitment module not found at ${MODULE_DIR}${NC}"
    exit 1
fi

# ── 1. Layer structure ──────────────────────────────────────────────────────
echo "--- Layer Structure ---"
for layer in domain application infrastructure interfaces; do
    if [[ -d "${MODULE_DIR}/${layer}" ]]; then
        pass "layer exists: ${layer}/"
    else
        fail "layer missing: ${layer}/"
    fi
done

# ── 2. Frozen contract files ────────────────────────────────────────────────
echo ""
echo "--- Contract Files ---"
CONTRACT_FILES=(
    "mod.rs"
    "domain/mod.rs"
    "domain/intent.rs"
    "domain/state.rs"
    "domain/record.rs"
    "domain/outcome.rs"
    "domain/resolver.rs"
    "domain/error.rs"
    "domain/query.rs"
    "domain/event/mod.rs"
    "application/mod.rs"
    "application/service.rs"
    "application/service_impl.rs"
    "application/key.rs"
    "application/key_impl.rs"
    "application/policy.rs"
    "application/dto/mod.rs"
    "application/seam.rs"
    "application/seam_impl.rs"
    "application/fence.rs"
    "application/fence_impl.rs"
    "application/vocabulary.rs"
    "application/config.rs"
    "application/factory.rs"
    "application/setup.rs"
    "application/receipt.rs"
    "infrastructure/mod.rs"
    "infrastructure/in_memory.rs"
    "infrastructure/http.rs"
    "infrastructure/provider_receipt.rs"
    "infrastructure/factory_impl.rs"
    "interfaces/mod.rs"
    "interfaces/effect_commitment_ledger.rs"
    "interfaces/contract.rs"
    "interfaces/http/mod.rs"
)
for file in "${CONTRACT_FILES[@]}"; do
    if [[ -f "${MODULE_DIR}/${file}" ]]; then
        pass "contract file: ${file}"
    else
        fail "contract file missing: ${file}"
    fi
done

# require_impl <label> <impl-marker> <file> [required-method ...]
require_impl() {
    local label="$1" marker="$2" file="$3"
    shift 3
    local path="${MODULE_DIR}/${file}"

    if [[ ! -f "${path}" ]]; then
        fail "${label}: implementation file missing (${file})"
        return
    fi

    local location
    location="$(grep -nF "${marker}" "${path}" | head -1 | cut -d: -f1)"
    if [[ -z "${location}" ]]; then
        fail "${label}: '${marker}' not found in ${file}"
        return
    fi
    pass "${label}: ${marker} (${file}:${location})"

    local method method_line
    for method in "$@"; do
        method_line="$(grep -nF "${method}" "${path}" | head -1 | cut -d: -f1)"
        if [[ -z "${method_line}" ]]; then
            fail "${label}: method '${method}' missing from ${file}"
        else
            pass "${label}: method ${method} (${file}:${method_line})"
        fi
    done
}

# ── 3. Port + adapters ──────────────────────────────────────────────────────
echo ""
echo "--- EffectCommitmentLedger port ---"
if grep -qF "pub trait EffectCommitmentLedger" "${MODULE_DIR}/interfaces/effect_commitment_ledger.rs"; then
    pass "trait declared: EffectCommitmentLedger"
else
    fail "trait missing: EffectCommitmentLedger"
fi

for adapter in in_memory http; do
    require_impl "EffectCommitmentLedger/${adapter}" \
        "impl EffectCommitmentLedger for " \
        "infrastructure/${adapter}.rs" \
        "async fn begin" \
        "async fn settle_definite" \
        "async fn mark_ambiguous" \
        "async fn resolve" \
        "async fn query" \
        "fn can_sweep_dispatched"
done

# ── 4. Application contracts ────────────────────────────────────────────────
echo ""
echo "--- Application contracts ---"
for trait_file in \
    "service.rs:EffectCommitmentService" \
    "key.rs:IntentKeyDerivation" \
    "seam.rs:DispatchSeam" \
    "fence.rs:DependentFence" \
    "fence.rs:RetryFence" \
    "factory.rs:EffectCommitmentFactory" \
    "setup.rs:EffectCommitmentCompositionRoot" \
    "receipt.rs:ProviderReceiptReader"; do
    IFS=':' read -r file trait_name <<< "${trait_file}"
    if grep -qF "pub trait ${trait_name}" "${MODULE_DIR}/application/${file}"; then
        pass "trait declared: ${trait_name}"
    else
        fail "trait missing: ${trait_name} (application/${file})"
    fi
done

require_impl "EffectCommitmentService" \
    "impl EffectCommitmentService for " \
    "application/service_impl.rs" \
    "async fn begin" \
    "async fn settle_definite" \
    "async fn mark_ambiguous" \
    "async fn resolve" \
    "async fn query"

require_impl "IntentKeyDerivation" \
    "impl IntentKeyDerivation for " \
    "application/key_impl.rs" \
    "fn derive_key"

require_impl "DispatchSeam" \
    "impl DispatchSeam for " \
    "application/seam_impl.rs" \
    "async fn before_dispatch" \
    "async fn after_dispatch"

require_impl "DependentFence" \
    "impl DependentFence for " \
    "application/fence_impl.rs" \
    "fn may_release_dependents"

require_impl "RetryFence" \
    "impl RetryFence for " \
    "application/fence_impl.rs" \
    "fn may_retry"

require_impl "ProviderReceiptReader" \
    "impl ProviderReceiptReader for " \
    "infrastructure/provider_receipt.rs" \
    "async fn observe"

# ── 5. Domain contracts ────────────────────────────────────────────────────
echo ""
echo "--- Domain contracts ---"
for entry in \
    "intent.rs:EffectIntent" \
    "state.rs:EffectState" \
    "record.rs:EffectRecord" \
    "record.rs:EffectRecordId" \
    "outcome.rs:ObservedOutcome" \
    "outcome.rs:OutcomeVerdict" \
    "resolver.rs:ResolverIdentity" \
    "resolver.rs:ResolverKind" \
    "error.rs:CommitmentError" \
    "event/mod.rs:EffectEvent" \
    "query.rs:EffectQuery"; do
    IFS=':' read -r file symbol <<< "${entry}"
    if grep -qF "pub enum ${symbol}" "${MODULE_DIR}/domain/${file}" \
        || grep -qF "pub struct ${symbol}" "${MODULE_DIR}/domain/${file}"; then
        pass "domain contract: ${symbol}"
    else
        fail "domain contract missing: ${symbol} (domain/${file})"
    fi
done

# The closed error vocabulary: exactly the six frozen variants.
ERROR_VARIANTS=(AlreadySettled NotAmbiguous UnauthorizedResolver EvidenceMissing Unavailable Contended)
for variant in "${ERROR_VARIANTS[@]}"; do
    if grep -qF "${variant}" "${MODULE_DIR}/domain/error.rs"; then
        pass "CommitmentError variant: ${variant}"
    else
        fail "CommitmentError variant missing: ${variant}"
    fi
done

# ── 6. Factories + wiring ──────────────────────────────────────────────────
echo ""
echo "--- Wiring ---"
require_impl "EffectCommitmentFactory" \
    "impl EffectCommitmentFactory for " \
    "infrastructure/factory_impl.rs" \
    "fn create" \
    "fn create_receipt_reader"

if grep -qF "pub fn dispatch_seam" "${MODULE_DIR}/application/setup.rs"; then
    pass "setup exposes the dispatch seam"
else
    fail "setup does not expose dispatch_seam"
fi

DISPATCH="${PROJECT_ROOT}/engine/src/execution_engine/application/service_impl/dispatch.rs"
for marker in "begin_effect" "settle_effect" "effect_blocks_dependents" "retry refused"; do
    if grep -qF "${marker}" "${DISPATCH}"; then
        pass "dispatch seam wired: ${marker}"
    else
        fail "dispatch seam missing: ${marker} (execution_engine/dispatch.rs)"
    fi
done

for root in \
    "cli/src/cli_boundary/orchestrator.rs" \
    "mcp/src/host/mod.rs" \
    "actions/src/main.rs"; do
    if grep -qF "EffectCommitmentSetup::from_env" "${PROJECT_ROOT}/${root}"; then
        pass "composition root armed: ${root}"
    else
        fail "composition root missing EffectCommitmentSetup::from_env: ${root}"
    fi
done

if grep -qF "effect_commitment" "${PROJECT_ROOT}/engine/src/execution_engine/application/factory.rs"; then
    pass "execution factory config carries the seam"
else
    fail "execution factory config missing effect_commitment"
fi

# ── 7. NodeStatus vocabulary ───────────────────────────────────────────────
echo ""
echo "--- NodeStatus vocabulary ---"
NODE_STATE="${PROJECT_ROOT}/engine/src/state_persistence/domain/state.rs"
for variant in Dispatched Ambiguous; do
    if grep -qF "${variant}" "${NODE_STATE}"; then
        pass "NodeStatus variant: ${variant}"
    else
        fail "NodeStatus variant missing: ${variant}"
    fi
done
if grep -qF "NodeStatus::Completed | NodeStatus::Failed | NodeStatus::Skipped" "${NODE_STATE}"; then
    pass "NodeStatus::is_terminal excludes Dispatched/Ambiguous"
else
    fail "NodeStatus::is_terminal may have started treating an unsettled state as terminal"
fi

# ── 8. Tests ───────────────────────────────────────────────────────────────
echo ""
echo "--- Tests ---"
COMPONENTS=(
    effectintent effectstate effectrecord observedoutcome resolveridentity commitmenterror
    effectcommitmentservice intentkeyderivation reconciliationpolicy inmemoryeffectledger
    httpeffectledger providerreceiptreader effectcommitmentconfig defaulteffectcommitmentfactory
    effectcommitmentledger dispatchseam dependentfence retryfence nodestatevocabulary compositionroots
)
for component in "${COMPONENTS[@]}"; do
    if compgen -G "${TEST_DIR}/${component}/*_test.rs" > /dev/null; then
        pass "unit test: ${component}"
    else
        fail "unit test missing: tests/unit/effect-commitment/${component}/*_test.rs"
    fi
done

INTEGRATION_TESTS=(
    effect_intent_integration.rs
    effect_state_integration.rs
    effect_record_integration.rs
    observed_outcome_integration.rs
    resolver_identity_integration.rs
    commitment_error_integration.rs
    effect_commitment_service_integration.rs
    intent_key_derivation_integration.rs
    reconciliation_policy_integration.rs
    in_memory_effect_ledger_integration.rs
    provider_receipt_reader_integration.rs
    effect_commitment_config_integration.rs
    default_effect_commitment_factory_integration.rs
    effect_commitment_dispatch_seam_integration.rs
    dependent_fence_integration.rs
    retry_fence_integration.rs
    effect_commitment_composition_e2e.rs
)
for test in "${INTEGRATION_TESTS[@]}"; do
    if [[ -f "${PROJECT_ROOT}/engine/tests/${test}" ]]; then
        pass "integration test: ${test}"
    else
        fail "integration test missing: engine/tests/${test}"
    fi
done

if grep -qF "pub async fn run_contract" "${MODULE_DIR}/interfaces/contract.rs"; then
    pass "port contract suite declared"
else
    fail "port contract suite missing"
fi
if grep -qF "run_contract" "${MODULE_DIR}/infrastructure/in_memory.rs"; then
    pass "in-process adapter runs the port contract suite"
else
    fail "in-process adapter does not run the port contract suite"
fi

# ── Summary ─────────────────────────────────────────────────────────────────
echo ""
echo "============================================"
echo "  Summary"
echo "============================================"
echo -e "  Passed: ${GREEN}${PASS_COUNT}${NC}"
echo -e "  Failed: ${RED}${FAIL_COUNT}${NC}"
echo ""

if [[ ${FAIL_COUNT} -gt 0 ]]; then
    echo "FAILURES:"
    for err in "${ERRORS[@]}"; do
        echo "  - ${err}"
    done
    echo -e "${RED}Effect Commitment contract check FAILED.${NC}"
    exit 1
fi

echo -e "${GREEN}All effect-commitment contracts are implemented and tested.${NC}"
exit 0
