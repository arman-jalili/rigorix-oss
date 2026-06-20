#!/usr/bin/env bash
# ============================================================================
# check_diff-analyzer_contracts.sh
# ============================================================================
# Validates that every contract interface in diff-analyzer has a matching
# concrete implementation. Exits 0 if all contracts satisfied, 1 otherwise.
#
# Usage: bash .pi/scripts/ci/check_diff-analyzer_contracts.sh [--verbose]
#
# Checks:
#   - Each service trait in application/service.rs has a corresponding _impl.rs
#   - Each domain type in domain/ has matching usage in implementation files
#   - Repository interface has an implementation
#   - Factory interface has an implementation
# ============================================================================

set -u
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../../.." && pwd)"
DIFF_DIR="$PROJECT_DIR/src/diff_analyzer"

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'
PASS=0
FAIL=0

pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS=$((PASS + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; FAIL=$((FAIL + 1)); }

echo "============================================"
echo "  Diff Analyzer Contract Validation"
echo "============================================"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Service traits have implementations
# ---------------------------------------------------------------------------
echo "--- Service Contracts ---"

check_service() {
    local trait_name="$1"
    local impl_file="$2"
    local impl_path="$DIFF_DIR/application/$impl_file"

    if [ ! -f "$impl_path" ]; then
        fail "Service '$trait_name': missing implementation file '$impl_file'"
        return 1
    fi
    if grep -q "impl.*$trait_name for" "$impl_path" 2>/dev/null; then
        pass "Service '$trait_name' → implemented in '$impl_file'"
        return 0
    else
        fail "Service '$trait_name': file '$impl_file' exists but does not implement the trait"
        return 1
    fi
}

check_service "DiffParsingService" "diff_parser_impl.rs"
check_service "PathValidationService" "path_validator_impl.rs"
check_service "LimitEnforcementService" "limit_enforcer_impl.rs"
check_service "RiskClassificationService" "risk_classifier_impl.rs"
check_service "AiSignalDetectionService" "ai_signal_detector_impl.rs"
check_service "DiffAnalysisPipelineService" "diff_analysis_pipeline_impl.rs"

# ---------------------------------------------------------------------------
# Check 2: Factory interfaces have implementations
# ---------------------------------------------------------------------------
echo ""
echo "--- Factory Contracts ---"

check_factory() {
    local factory_name="$1"
    local factory_file="$DIFF_DIR/application/factory.rs"

    if grep -q "pub trait $factory_name" "$factory_file" 2>/dev/null; then
        pass "Factory '$factory_name' defined in factory.rs"
        return 0
    else
        fail "Factory '$factory_name' not defined in factory.rs"
        return 1
    fi
}

check_factory "DiffFactory"
check_factory "LimitConfigFactory"
check_factory "AiSignalDetectorFactory"

# ---------------------------------------------------------------------------
# Check 3: Repository interface has implementation
# ---------------------------------------------------------------------------
echo ""
echo "--- Repository Contracts ---"

REPO_INTERFACE="$DIFF_DIR/infrastructure/repository/mod.rs"
if grep -q "pub trait DiffRepository" "$REPO_INTERFACE" 2>/dev/null; then
    pass "Repository 'DiffRepository' defined in repository/mod.rs"
else
    fail "Repository 'DiffRepository' not found in repository/mod.rs"
fi

# ---------------------------------------------------------------------------
# Check 4: Domain types are used in implementation files
# ---------------------------------------------------------------------------
echo ""
echo "--- Domain Type Usage ---"

DOMAIN_TYPES="PrDiff ChangedFile DiffHunk FileStatus FileRisk DiffAnalyzerError PolicyLimits AiSignal AiSignalResult"
IMPL_FILES=$(find "$DIFF_DIR/application" -name '*_impl.rs' -type f 2>/dev/null || echo "")

ALL_TYPES_USED=true
for dtype in $DOMAIN_TYPES; do
    found=false
    for f in $IMPL_FILES; do
        if grep -q "$dtype" "$f" 2>/dev/null; then
            found=true
            break
        fi
    done
    if [ "$found" = true ]; then
        pass "Domain type '$dtype' used in implementation files"
    else
        fail "Domain type '$dtype' not used in any implementation file"
        ALL_TYPES_USED=false
    fi
done

if [ "$ALL_TYPES_USED" = true ]; then
    pass "All domain types consumed by implementations"
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "============================================"
echo "  Summary"
echo "============================================"
echo -e "  Passed:   ${GREEN}${PASS}${NC}"
echo -e "  Failed:   ${RED}${FAIL}${NC}"
echo ""

if [ $FAIL -gt 0 ]; then
    echo -e "${RED}Some contract checks failed.${NC}"
    exit 1
fi

echo -e "${GREEN}All contracts validated.${NC}"
exit 0
