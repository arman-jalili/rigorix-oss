#!/usr/bin/env bash
# ============================================================================
# check_capacity-ledger_contracts.sh — contract implementation check
#
# Verifies that every frozen capacity-ledger interface has a concrete,
# method-complete implementation (or, for storage/factory seams, is at least
# declared and wired) so the contract freeze cannot silently drift.
#
# Checks:
#   1. The four DDD layers exist for engine/src/capacity_ledger/
#   2. Every frozen contract file exists
#   3. Every contract trait is declared
#   4. Every contract that must be implemented in this epic has an `impl`
#   5. Every method of each implemented trait appears in its implementation
#
# Usage:
#   bash .pi/scripts/ci/check_capacity-ledger_contracts.sh
#   bash .pi/scripts/ci/check_capacity-ledger_contracts.sh --help
#
# Exit: 0 = all contracts implemented, 1 = violation(s) found
# ============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="${RIGORIX_CONTRACT_ROOT:-$(cd "${SCRIPT_DIR}/../../.." && pwd)}"
MODULE_DIR="${PROJECT_ROOT}/engine/src/capacity_ledger"

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
    sed -n '3,22p' "$0" | sed 's/^# \{0,1\}//'
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
    cp -R "${MODULE_DIR}" "$TMP/engine/src/capacity_ledger"
    rm "$TMP/engine/src/capacity_ledger/domain/resource.rs"
    if RIGORIX_CONTRACT_ROOT="$TMP" bash "$0" >/dev/null 2>&1; then
        echo "❌ self-test: a removed frozen contract still passed" >&2
        exit 1
    fi
    echo "✅ self-test: seeded contract violation was caught"
    exit 0
fi

echo "============================================"
echo "  Capacity Ledger Contract Check"
echo "============================================"
echo ""

if [[ ! -d "${MODULE_DIR}" ]]; then
    echo -e "${RED}❌ capacity_ledger module not found at ${MODULE_DIR}${NC}"
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
    "domain/mod.rs"
    "domain/error.rs"
    "domain/resource.rs"
    "domain/reservation.rs"
    "domain/grant.rs"
    "domain/exposure.rs"
    "domain/idempotency.rs"
    "domain/event/mod.rs"
    "application/dto/mod.rs"
    "application/service.rs"
    "application/service_impl.rs"
    "application/guard.rs"
    "application/guard_impl.rs"
    "application/factory.rs"
    "interfaces/capacity_ledger.rs"
    "interfaces/http/mod.rs"
    "infrastructure/in_process.rs"
    "infrastructure/http.rs"
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
echo "--- CapacityLedger port ---"
if grep -qF "pub trait CapacityLedger" "${MODULE_DIR}/interfaces/capacity_ledger.rs"; then
    pass "trait declared: CapacityLedger"
else
    fail "trait missing: CapacityLedger"
fi

for adapter in in_process http; do
    require_impl "CapacityLedger/${adapter}" \
        "impl CapacityLedger for " \
        "infrastructure/${adapter}.rs" \
        "async fn declare_resource" \
        "async fn reserve" \
        "async fn commit" \
        "async fn release" \
        "async fn exposure"
done

# ── 4. Application contracts ────────────────────────────────────────────────
echo ""
echo "--- Application contracts ---"
if grep -qF "pub trait CapacityLedgerService" "${MODULE_DIR}/application/service.rs"; then
    pass "trait declared: CapacityLedgerService"
else
    fail "trait missing: CapacityLedgerService"
fi
require_impl "CapacityLedgerService" \
    "impl CapacityLedgerService for " \
    "application/service_impl.rs" \
    "async fn reserve" \
    "async fn commit" \
    "async fn release" \
    "async fn exposure"

if grep -qF "pub trait ReservationGuard" "${MODULE_DIR}/application/guard.rs"; then
    pass "trait declared: ReservationGuard"
else
    fail "trait missing: ReservationGuard"
fi
require_impl "ReservationGuard" \
    "impl ReservationGuard for " \
    "application/guard_impl.rs" \
    "fn reservation_id" \
    "fn is_committed" \
    "fn is_released" \
    "async fn commit"

# ── 5. Storage / factory seams ──────────────────────────────────────────────
echo ""
echo "--- Storage / factory seams ---"
SEAMS=(
    "application/factory.rs:pub trait CapacityLedgerFactory:CapacityLedgerFactory factory interface"
)
for seam in "${SEAMS[@]}"; do
    IFS=':' read -r file marker label <<< "${seam}"
    if grep -qF "${marker}" "${MODULE_DIR}/${file}"; then
        pass "${label} declared (${file})"
    else
        fail "${label} missing (${file})"
    fi
done

# A dead persistence seam must not come back (#1016): storage is the
# adapter's job, and the decision is recorded in the infrastructure module.
if [ -e "${MODULE_DIR}/infrastructure/repository/mod.rs" ]; then
    fail "dead CapacityLedgerRepository persistence port reintroduced (#1016)"
elif grep -qF "No local persistence port" "${MODULE_DIR}/infrastructure/mod.rs"; then
    pass "no local persistence port; decision recorded (#1016)"
else
    fail "persistence decision not recorded in infrastructure/mod.rs"
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
    echo -e "${RED}Capacity Ledger contract check FAILED.${NC}"
    exit 1
fi

echo -e "${GREEN}All capacity-ledger contracts are implemented.${NC}"
exit 0
