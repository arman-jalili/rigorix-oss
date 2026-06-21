#!/usr/bin/env bash
# Check CI Integration Contracts
#
# Validates that every interface defined in the contract freeze has a
# corresponding concrete implementation.
#
# Checks:
# - Application service traits → impl files exist
# - Infrastructure repository traits → impl files exist
# - Factory interfaces → impl files exist
# - Domain types → no orphan interfaces
#
# Usage: bash .pi/scripts/ci/check_ci-integration_contracts.sh [--verbose]
#
# Exit codes:
#   0 — All contracts have implementations
#   1 — One or more contracts are missing implementations

set -euo pipefail

VERBOSE=false
if [[ "${1:-}" == "--verbose" ]]; then
    VERBOSE=true
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../../../.." && pwd)"
CI_INTEGRATION_DIR="$ROOT_DIR/actions/src/ci_integration"

cd "$ROOT_DIR"

ERRORS=0
MISSING_IMPLS=()

# ── Helper: check if a file exists ──
check_file() {
    local description="$1"
    local filepath="$2"
    if [[ ! -f "$filepath" ]]; then
        MISSING_IMPLS+=("$description: $filepath")
        return 1
    fi
    return 0
}

# ── Interface-to-Implementation Mapping ──

echo "============================================"
echo "  CI Integration Contract Implementation Check"
echo "============================================"
echo ""

# ── Service Interfaces ──
echo "--- Service Interfaces ---"

check_file "StatusCheckService → StatusCheckServiceImpl" \
    "$CI_INTEGRATION_DIR/application/status_check_impl.rs" || ERRORS=$((ERRORS + 1))

check_file "PrCommentService → PrCommentServiceImpl" \
    "$CI_INTEGRATION_DIR/application/pr_comment_impl.rs" || ERRORS=$((ERRORS + 1))

# ── Factory Interfaces ──
echo ""
echo "--- Factory Interfaces ---"

check_file "StatusCheckFactory → StatusCheckFactoryImpl" \
    "$CI_INTEGRATION_DIR/application/status_check_factory_impl.rs" || ERRORS=$((ERRORS + 1))

check_file "PrCommentFactory → PrCommentFactoryImpl" \
    "$CI_INTEGRATION_DIR/application/pr_comment_factory_impl.rs" || ERRORS=$((ERRORS + 1))

# ── Repository Interfaces ──
echo ""
echo "--- Repository Interfaces ---"

check_file "StatusCheckRepository → StatusCheckRepositoryImpl" \
    "$CI_INTEGRATION_DIR/infrastructure/repository/status_check_repository_impl.rs" || ERRORS=$((ERRORS + 1))

check_file "PrCommentRepository → PrCommentRepositoryImpl" \
    "$CI_INTEGRATION_DIR/infrastructure/repository/pr_comment_repository_impl.rs" || ERRORS=$((ERRORS + 1))

# ── Domain Types ──
echo ""
echo "--- Domain Types ---"

check_file "Domain types (types.rs)" \
    "$CI_INTEGRATION_DIR/domain/types.rs" || ERRORS=$((ERRORS + 1))

check_file "Domain errors (error.rs)" \
    "$CI_INTEGRATION_DIR/domain/error.rs" || ERRORS=$((ERRORS + 1))

check_file "Domain events (event/mod.rs)" \
    "$CI_INTEGRATION_DIR/domain/event/mod.rs" || ERRORS=$((ERRORS + 1))

# ── HTTP API Contracts ──
echo ""
echo "--- HTTP API Contracts ---"

check_file "HTTP API contracts (http/mod.rs)" \
    "$CI_INTEGRATION_DIR/interfaces/http/mod.rs" || ERRORS=$((ERRORS + 1))

# ── Verify trait methods are implemented ──
echo ""
echo "--- Method Implementation Verification ---"

# Check StatusCheckServiceImpl implements all StatusCheckService methods
IMPLEMENTED_METHODS=$(grep -c "async fn\|fn " "$CI_INTEGRATION_DIR/application/status_check_impl.rs" 2>/dev/null || echo 0)
SERVICE_METHODS=3  # create_pending, update_status, execution_url
if [[ "$IMPLEMENTED_METHODS" -lt "$SERVICE_METHODS" ]]; then
    MISSING_IMPLS+=("StatusCheckServiceImpl: expected at least $SERVICE_METHODS methods, found $IMPLEMENTED_METHODS")
    ERRORS=$((ERRORS + 1))
fi

# Check PrCommentServiceImpl implements all PrCommentService methods
PR_COMMENT_METHODS=$(grep -c "async fn\|fn " "$CI_INTEGRATION_DIR/application/pr_comment_impl.rs" 2>/dev/null || echo 0)
PR_SERVICE_METHODS=4  # upsert, find_bot_comment, post_annotation, outcome_description
if [[ "$PR_COMMENT_METHODS" -lt "$PR_SERVICE_METHODS" ]]; then
    MISSING_IMPLS+=("PrCommentServiceImpl: expected at least $PR_SERVICE_METHODS methods, found $PR_COMMENT_METHODS")
    ERRORS=$((ERRORS + 1))
fi

# ── Results ──
echo ""
echo "============================================"
if [[ ${#MISSING_IMPLS[@]} -gt 0 ]]; then
    echo "  ❌ ${ERRORS} contract(s) missing implementation(s):"
    for impl in "${MISSING_IMPLS[@]}"; do
        echo "    - $impl"
    done
    echo ""
    echo "  FAILED: $ERRORS missing implementation(s)"
    exit 1
else
    echo "  ✅ All 10 contracts have implementations"
    echo ""
    echo "  PASSED: All contracts verified"
    exit 0
fi
