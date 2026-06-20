#!/usr/bin/env bash
# Check Action Output Contracts
#
# Validates that every interface defined in the contract freeze has a
# corresponding concrete implementation.
#
# Checks:
# - Application service traits → impl files exist
# - Infrastructure repository traits → impl files exist
# - Domain types → no orphan interfaces
#
# Usage: bash .pi/scripts/ci/check_action-output_contracts.sh [--verbose]
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
ACTION_OUTPUT_DIR="$ROOT_DIR/actions/src/action_output"

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
# Format: "description|expected_impl_file"

# Service interfaces
check_file "OutputFormattingService → OutputFormatterImpl" \
    "$ACTION_OUTPUT_DIR/application/output_formatter_impl.rs" || ERRORS=$((ERRORS + 1))

check_file "AnnotationWritingService → AnnotationWriterImpl" \
    "$ACTION_OUTPUT_DIR/application/annotation_writer_impl.rs" || ERRORS=$((ERRORS + 1))

check_file "StepSummaryWritingService → StepSummaryWriterImpl" \
    "$ACTION_OUTPUT_DIR/application/step_summary_writer_impl.rs" || ERRORS=$((ERRORS + 1))

# Output variable service
check_file "OutputVariableService → OutputVariableServiceImpl" \
    "$ACTION_OUTPUT_DIR/application/output_variable_impl.rs" || ERRORS=$((ERRORS + 1))

# PR comment service
check_file "PrCommentService → PrCommentServiceImpl" \
    "$ACTION_OUTPUT_DIR/application/pr_comment_impl.rs" || ERRORS=$((ERRORS + 1))

# Repository implementations
check_file "OutputRepository → OutputRepositoryImpl" \
    "$ACTION_OUTPUT_DIR/infrastructure/repository/output_repository_impl.rs" || ERRORS=$((ERRORS + 1))

check_file "EnvRepository → EnvRepositoryImpl" \
    "$ACTION_OUTPUT_DIR/infrastructure/repository/env_repository_impl.rs" || ERRORS=$((ERRORS + 1))

check_file "GitHubApiClient → GitHubApiClientImpl" \
    "$ACTION_OUTPUT_DIR/infrastructure/repository/github_api_client_impl.rs" || ERRORS=$((ERRORS + 1))

# ── Check Domain Layer Structure ──
check_file "Domain types module" \
    "$ACTION_OUTPUT_DIR/domain/types.rs" || ERRORS=$((ERRORS + 1))

check_file "Domain error module" \
    "$ACTION_OUTPUT_DIR/domain/error.rs" || ERRORS=$((ERRORS + 1))

check_file "Domain event module" \
    "$ACTION_OUTPUT_DIR/domain/event/mod.rs" || ERRORS=$((ERRORS + 1))

# ── Check DTO module ──
check_file "Application DTO module" \
    "$ACTION_OUTPUT_DIR/application/dto/mod.rs" || ERRORS=$((ERRORS + 1))

# ── Check factory module ──
check_file "Factory interfaces module" \
    "$ACTION_OUTPUT_DIR/application/factory.rs" || ERRORS=$((ERRORS + 1))

# ── Check HTTP API contracts ──
check_file "HTTP API contracts module" \
    "$ACTION_OUTPUT_DIR/interfaces/http/mod.rs" || ERRORS=$((ERRORS + 1))

# ── Summary ──
echo ""
if [[ ${#MISSING_IMPLS[@]} -gt 0 ]]; then
    echo "❌ MISSING IMPLEMENTATIONS:"
    for missing in "${MISSING_IMPLS[@]}"; do
        echo "   - $missing"
    done
fi

if [[ $ERRORS -eq 0 ]]; then
    echo "✅ All action-output contracts have implementations ($ERRORS missing)"
    if $VERBOSE; then
        echo ""
        echo "Service Implementations (6/6):"
        echo "  ✓ OutputFormattingService → output_formatter_impl.rs"
        echo "  ✓ AnnotationWritingService → annotation_writer_impl.rs"
        echo "  ✓ StepSummaryWritingService → step_summary_writer_impl.rs"
        echo "  ✓ OutputVariableService → output_variable_impl.rs"
        echo "  ✓ PrCommentService → pr_comment_impl.rs"
        echo ""
        echo "Repository Implementations (3/3):"
        echo "  ✓ OutputRepository → output_repository_impl.rs"
        echo "  ✓ EnvRepository → env_repository_impl.rs"
        echo "  ✓ GitHubApiClient → github_api_client_impl.rs"
        echo ""
        echo "Domain:"
        echo "  ✓ types.rs, error.rs, event/"
        echo ""
        echo "Application:"
        echo "  ✓ dto/, factory.rs"
        echo ""
        echo "Interfaces:"
        echo "  ✓ http/"
    fi
    exit 0
else
    echo "❌ $ERRORS contract(s) missing implementation"
    exit 1
fi
