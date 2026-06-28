#!/usr/bin/env bash
# ============================================================================
# check_planning-pipeline_contracts.sh
#
# Validates that every contract interface from the planning-pipeline module
# has a concrete implementation. Uses grep/find to detect trait definitions
# and their implementing structs.
#
# Usage: bash .pi/scripts/ci/check_planning-pipeline_contracts.sh [--help]
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

# Determine source directory
if [ ! -d "$SRC_DIR" ]; then
    SRC_DIR="$(cd "$PI_DIR/.." && pwd)/src"
fi
if [ ! -d "$SRC_DIR" ]; then
    log_fail "Source directory not found"
    exit 1
fi

MODULE="planning"
SRC="$SRC_DIR/$MODULE"

echo ""
echo "═══ Planning-Pipeline Contract Implementation Check ═══"
echo "Source: $SRC"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Service Contracts
# ---------------------------------------------------------------------------
echo "--- Service Contracts ---"

if grep -q 'pub trait PlanningPipelineService' "$SRC/application/service.rs" 2>/dev/null; then
    if grep -q 'impl.*PlanningPipelineService' "$SRC/application/pipeline_impl.rs" 2>/dev/null; then
        log_pass "PlanningPipelineService → PlanningPipelineImpl"
    else
        log_fail "PlanningPipelineService trait has no implementation"
    fi
else
    log_fail "PlanningPipelineService trait not found"
fi

# ---------------------------------------------------------------------------
# Check 2: Factory Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Factory Contracts ---"

if grep -q 'pub trait PlanningPipelineFactory' "$SRC/application/factory.rs" 2>/dev/null; then
    if grep -q 'impl.*PlanningPipelineFactory' "$SRC/application/pipeline_factory_impl.rs" 2>/dev/null; then
        log_pass "PlanningPipelineFactory → PlanningPipelineFactoryImpl"
    else
        log_fail "PlanningPipelineFactory trait has no implementation"
    fi
else
    log_fail "PlanningPipelineFactory trait not found"
fi

if grep -q 'pub trait CompositeValidator' "$SRC/application/factory.rs" 2>/dev/null; then
    log_pass "CompositeValidator trait defined"
else
    log_fail "CompositeValidator trait not found"
fi

# ---------------------------------------------------------------------------
# Check 3: Domain trait contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Domain Trait Contracts ---"

if grep -q 'pub trait Classifier' "$SRC/domain/classification.rs" 2>/dev/null; then
    log_pass "Classifier trait defined"

    if grep -q 'impl Classifier for MockClassifier' "$SRC/application/mock_classifier.rs" 2>/dev/null; then
        log_pass "Classifier → MockClassifier implementation"
    else
        log_fail "MockClassifier implementation not found"
    fi

    if grep -q 'impl Classifier for ClaudeClassifier' "$SRC/infrastructure/claude_classifier.rs" 2>/dev/null; then
        log_pass "Classifier → ClaudeClassifier implementation"
    else
        log_fail "ClaudeClassifier implementation not found"
    fi

    if grep -q 'impl Classifier for OpenaiClassifier' "$SRC/infrastructure/openai_classifier.rs" 2>/dev/null; then
        log_pass "Classifier → OpenaiClassifier implementation"
    else
        log_fail "OpenaiClassifier implementation not found"
    fi
else
    log_fail "Classifier trait not found"
fi

if grep -q 'pub trait ParameterExtractor' "$SRC/domain/extractor.rs" 2>/dev/null; then
    log_pass "ParameterExtractor trait defined"

    if grep -q 'impl ParameterExtractor for MockParameterExtractor' "$SRC/application/mock_extractor.rs" 2>/dev/null; then
        log_pass "ParameterExtractor → MockParameterExtractor implementation"
    else
        log_fail "MockParameterExtractor implementation not found"
    fi
else
    log_fail "ParameterExtractor trait not found"
fi

if grep -q 'pub trait TemplateGenerator' "$SRC_DIR/template_generation/domain/generator.rs" 2>/dev/null; then
    log_pass "TemplateGenerator trait defined"
else
    log_fail "TemplateGenerator trait not found"
fi

# ---------------------------------------------------------------------------
# Check 4: Domain entities
# ---------------------------------------------------------------------------
echo ""
echo "--- Domain Entities ---"

if grep -q 'pub struct UserIntent' "$SRC/domain/intent.rs" 2>/dev/null; then
    log_pass "UserIntent struct exists"
else
    log_fail "UserIntent struct not found"
fi

if grep -q 'pub struct PlanningResult' "$SRC/domain/result.rs" 2>/dev/null; then
    log_pass "PlanningResult struct exists"
else
    log_fail "PlanningResult struct not found"
fi

if grep -q 'pub struct PlanOutput' "$SRC/domain/result.rs" 2>/dev/null; then
    log_pass "PlanOutput struct exists"
else
    log_fail "PlanOutput struct not found"
fi

if grep -q 'pub struct PlanningHash' "$SRC/domain/result.rs" 2>/dev/null; then
    log_pass "PlanningHash struct exists"
else
    log_fail "PlanningHash struct not found"
fi

if grep -q 'pub struct ClassificationResult' "$SRC/domain/classification.rs" 2>/dev/null; then
    log_pass "ClassificationResult struct exists"
else
    log_fail "ClassificationResult struct not found"
fi

if grep -q 'pub struct ExtractedParameters' "$SRC/domain/extractor.rs" 2>/dev/null; then
    log_pass "ExtractedParameters struct exists"
else
    log_fail "ExtractedParameters struct not found"
fi

if grep -q 'pub struct GeneratedTemplate' "$SRC_DIR/template_generation/domain/generator.rs" 2>/dev/null; then
    log_pass "GeneratedTemplate struct exists"
else
    log_fail "GeneratedTemplate struct not found"
fi

if grep -q 'pub enum PlanningError' "$SRC/domain/error.rs" 2>/dev/null; then
    log_pass "PlanningError enum exists"
else
    log_fail "PlanningError enum not found"
fi

if grep -q 'pub enum PlanningEvent' "$SRC/domain/event/mod.rs" 2>/dev/null; then
    log_pass "PlanningEvent enum exists"
else
    log_fail "PlanningEvent enum not found"
fi

# ---------------------------------------------------------------------------
# Check 5: DTO schemas
# ---------------------------------------------------------------------------
echo ""
echo "--- DTO Schemas ---"

DTO_COUNT=$(grep -c 'pub struct.*\(Input\|Output\|Summary\|Status\|Error\|Warning\)' "$SRC/application/dto/mod.rs" 2>/dev/null || echo 0)
if [ "$DTO_COUNT" -ge 8 ]; then
    log_pass "DTO schemas exist ($DTO_COUNT DTOs)"
else
    log_fail "Fewer than 8 DTO schemas found ($DTO_COUNT)"
fi

# ---------------------------------------------------------------------------
# Check 6: API contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- API Contracts ---"

if grep -q 'pub const.*_PATH' "$SRC/interfaces/http/mod.rs" 2>/dev/null; then
    ENDPOINT_COUNT=$(grep -c 'pub const.*_PATH' "$SRC/interfaces/http/mod.rs" || echo 0)
    log_pass "API endpoint contracts exist ($ENDPOINT_COUNT endpoints)"
else
    log_fail "No API endpoint contracts found"
fi

if grep -q 'pub struct.*ApiError' "$SRC/interfaces/http/mod.rs" 2>/dev/null; then
    log_pass "Error response format defined"
else
    log_fail "Error response format not found"
fi

if grep -q 'pub mod error_codes' "$SRC/interfaces/http/mod.rs" 2>/dev/null; then
    log_pass "Standardized error codes defined"
else
    log_fail "Standardized error codes not found"
fi

# ---------------------------------------------------------------------------
# Check 7: Repository contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Repository Contracts ---"

if grep -q 'pub trait PlanningResultRepository' "$SRC/infrastructure/repository/mod.rs" 2>/dev/null; then
    log_pass "PlanningResultRepository trait defined"
else
    log_fail "PlanningResultRepository trait not found"
fi

# ---------------------------------------------------------------------------
# Check 8: Module structure
# ---------------------------------------------------------------------------
echo ""
echo "--- Module Structure ---"

MODULES=(
    "planning/mod.rs:pub mod"
    "planning/domain/mod.rs:pub mod"
    "planning/application/mod.rs:pub mod"
    "planning/infrastructure/mod.rs:pub mod"
    "planning/interfaces/mod.rs:pub mod"
)
for entry in "${MODULES[@]}"; do
    FILE="${entry%%:*}"
    EXPECTED="${entry##*:}"
    if grep -q "$EXPECTED" "$SRC_DIR/$FILE" 2>/dev/null; then
        log_pass "Module file exists: $FILE"
    else
        log_fail "Module file missing or invalid: $FILE"
    fi
done

# ---------------------------------------------------------------------------
# Check 9: Tests exist
# ---------------------------------------------------------------------------
echo ""
echo "--- Tests ---"

if grep -q "#\[cfg(test)\]" "$SRC/mod.rs" 2>/dev/null || [ -f "$SRC/tests.rs" ]; then
    TEST_COUNT=$(grep -c '#\[tokio::test\]\|#\[test\]' "$SRC/tests.rs" 2>/dev/null || echo 0)
    if [ "$TEST_COUNT" -ge 40 ]; then
        log_pass "Tests exist ($TEST_COUNT test functions)"
    else
        log_fail "Fewer than 40 tests ($TEST_COUNT found)"
    fi
else
    log_fail "No test module found"
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
    echo "Some planning-pipeline contracts are missing implementations."
    exit 1
fi

echo "All planning-pipeline contracts have implementations."
exit 0
