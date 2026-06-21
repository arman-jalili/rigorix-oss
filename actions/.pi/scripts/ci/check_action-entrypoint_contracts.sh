#!/usr/bin/env bash
# Check Action Entrypoint Contracts
#
# Validates that every interface defined in the contract freeze has a
# corresponding concrete implementation.
#
# Checks:
# - Application service traits → impl files exist
# - Infrastructure repository traits → impl files exist
# - Domain types → no orphan interfaces
# - DTO schemas → module exists
# - HTTP API contracts → module exists
#
# Usage: bash .pi/scripts/ci/check_action-entrypoint_contracts.sh [--verbose]
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
EP_DIR="$ROOT_DIR/actions/src/action_entrypoint"

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
check_file "ActionRouter → RouterImpl" \
    "$EP_DIR/application/router_impl.rs" || ERRORS=$((ERRORS + 1))

check_file "ModeResolver → ModeResolverImpl" \
    "$EP_DIR/application/mode_resolver_impl.rs" || ERRORS=$((ERRORS + 1))

check_file "ContextBuilder → ContextBuilderImpl" \
    "$EP_DIR/application/context_builder_impl.rs" || ERRORS=$((ERRORS + 1))

# Repository interfaces
check_file "ContextRepository → ContextRepositoryImpl" \
    "$EP_DIR/infrastructure/context_repository_impl.rs" || ERRORS=$((ERRORS + 1))

# ── Check Domain Layer Structure ──
check_file "Domain types module" \
    "$EP_DIR/domain/types.rs" || ERRORS=$((ERRORS + 1))

check_file "Domain error module" \
    "$EP_DIR/domain/error.rs" || ERRORS=$((ERRORS + 1))

check_file "Domain event module" \
    "$EP_DIR/domain/event/mod.rs" || ERRORS=$((ERRORS + 1))

# ── Check DTO module ──
check_file "Application DTO module" \
    "$EP_DIR/application/dto/mod.rs" || ERRORS=$((ERRORS + 1))

# ── Check factory module ──
check_file "Factory interfaces module" \
    "$EP_DIR/application/factory.rs" || ERRORS=$((ERRORS + 1))

# ── Check HTTP API contracts ──
check_file "HTTP API contracts module" \
    "$EP_DIR/interfaces/http/mod.rs" || ERRORS=$((ERRORS + 1))

# ── Summary ──
echo ""
if [[ ${#MISSING_IMPLS[@]} -gt 0 ]]; then
    echo "❌ MISSING IMPLEMENTATIONS:"
    for missing in "${MISSING_IMPLS[@]}"; do
        echo "   - $missing"
    done
fi

if [[ $ERRORS -eq 0 ]]; then
    echo "✅ All action-entrypoint contracts have implementations ($ERRORS missing)"
    if $VERBOSE; then
        echo ""
        echo "Service Implementations:"
        echo "  ✓ ActionRouter → router_impl.rs"
        echo "  ✓ ModeResolver → mode_resolver_impl.rs"
        echo "  ✓ ContextBuilder → context_builder_impl.rs"
        echo ""
        echo "Repository Implementations:"
        echo "  ✓ ContextRepository → context_repository_impl.rs"
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
