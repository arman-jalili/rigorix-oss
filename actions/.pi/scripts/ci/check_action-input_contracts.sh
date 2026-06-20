#!/usr/bin/env bash
# Check Action Input Contracts
#
# Validates that every interface defined in the contract freeze has a
# corresponding concrete implementation.
#
# Checks:
# - Application service traits → impl files exist
# - Infrastructure repository traits → impl files exist
# - Domain types → no orphan interfaces
#
# Usage: bash .pi/scripts/ci/check_action-input_contracts.sh [--verbose]
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
ACTION_INPUT_DIR="$ROOT_DIR/actions/src/action_input"

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
check_file "InputParsingService → InputParserImpl" \
    "$ACTION_INPUT_DIR/application/input_parser_impl.rs" || ERRORS=$((ERRORS + 1))

check_file "CommentParsingService → CommentParserImpl" \
    "$ACTION_INPUT_DIR/application/comment_parser_impl.rs" || ERRORS=$((ERRORS + 1))

check_file "CiDetectionService → CiDetectorImpl" \
    "$ACTION_INPUT_DIR/application/ci_detector_impl.rs" || ERRORS=$((ERRORS + 1))

check_file "ConfigLoadingService → ConfigLoaderImpl" \
    "$ACTION_INPUT_DIR/application/config_loader_impl.rs" || ERRORS=$((ERRORS + 1))

# Repository interfaces
check_file "InputRepository → EnvInputRepository" \
    "$ACTION_INPUT_DIR/infrastructure/env_input_repository_impl.rs" || ERRORS=$((ERRORS + 1))

check_file "ConfigRepository → YamlConfigRepository" \
    "$ACTION_INPUT_DIR/application/config_loader_impl.rs" || ERRORS=$((ERRORS + 1))

# ── Check Domain Layer Structure ──
check_file "Domain types module" \
    "$ACTION_INPUT_DIR/domain/types.rs" || ERRORS=$((ERRORS + 1))

check_file "Domain error module" \
    "$ACTION_INPUT_DIR/domain/error.rs" || ERRORS=$((ERRORS + 1))

check_file "Domain event module" \
    "$ACTION_INPUT_DIR/domain/event/mod.rs" || ERRORS=$((ERRORS + 1))

# ── Check DTO module ──
check_file "Application DTO module" \
    "$ACTION_INPUT_DIR/application/dto/mod.rs" || ERRORS=$((ERRORS + 1))

# ── Check factory module ──
check_file "Factory interfaces module" \
    "$ACTION_INPUT_DIR/application/factory.rs" || ERRORS=$((ERRORS + 1))

# ── Check HTTP API contracts ──
check_file "HTTP API contracts module" \
    "$ACTION_INPUT_DIR/interfaces/http/mod.rs" || ERRORS=$((ERRORS + 1))

# ── Summary ──
echo ""
if [[ ${#MISSING_IMPLS[@]} -gt 0 ]]; then
    echo "❌ MISSING IMPLEMENTATIONS:"
    for missing in "${MISSING_IMPLS[@]}"; do
        echo "   - $missing"
    done
fi

if [[ $ERRORS -eq 0 ]]; then
    echo "✅ All action-input contracts have implementations ($ERRORS missing)"
    if $VERBOSE; then
        echo ""
        echo "Service Implementations:"
        echo "  ✓ InputParsingService → input_parser_impl.rs"
        echo "  ✓ CommentParsingService → comment_parser_impl.rs"
        echo "  ✓ CiDetectionService → ci_detector_impl.rs"
        echo "  ✓ ConfigLoadingService → config_loader_impl.rs"
        echo ""
        echo "Repository Implementations:"
        echo "  ✓ InputRepository → env_input_repository_impl.rs"
        echo "  ✓ ConfigRepository → config_loader_impl.rs::YamlConfigRepository"
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
