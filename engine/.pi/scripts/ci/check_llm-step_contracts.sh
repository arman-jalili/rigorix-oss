#!/usr/bin/env bash
# ============================================================================
# check_llm-step_contracts.sh
#
# Validates that every contract interface from the llm-step module has a
# concrete implementation. Uses grep/find to detect trait definitions and
# their implementing structs.
#
# Usage: bash .pi/scripts/ci/check_llm-step_contracts.sh [--help]
#
# Exit codes: 0 = all contracts implemented, 1 = violations found
# ============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PI_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Determine source directory
if [ -d "$(cd "$PI_DIR/.." && pwd)/engine/src" ]; then
    SRC_DIR="$(cd "$PI_DIR/.." && pwd)/engine/src"
elif [ -d "$(cd "$PI_DIR/.." && pwd)/src" ]; then
    SRC_DIR="$(cd "$PI_DIR/.." && pwd)/src"
else
    echo "ERROR: Source directory not found"
    exit 1
fi

PASS=0
FAIL=0
ERRORS=()

MODULE="llm_step"

log_pass() { echo "  ✓ PASS: $1"; PASS=$((PASS + 1)); }
log_fail() { echo "  ✗ FAIL: $1"; ERRORS+=("$1"); FAIL=$((FAIL + 1)); }

echo ""
echo "═══ LLM-Step Contract Implementation Check ═══"
echo "Source: $SRC_DIR/$MODULE"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Service Contracts — LlmStepService
# ---------------------------------------------------------------------------
echo "--- Service Contracts ---"

if grep -q 'pub trait LlmStepService' "$SRC_DIR/$MODULE/application/service.rs" 2>/dev/null; then
    if grep -q 'impl.*LlmStepService' "$SRC_DIR/$MODULE/application/service_impl.rs" 2>/dev/null; then
        log_pass "LlmStepService → LlmStepServiceImpl"
    else
        log_fail "LlmStepService trait has no implementation"
    fi
else
    log_fail "LlmStepService trait not found"
fi

if grep -q 'pub trait LlmContextBuilderService' "$SRC_DIR/$MODULE/application/service.rs" 2>/dev/null; then
    if grep -q 'impl.*LlmContextBuilderService' "$SRC_DIR/$MODULE/application/service_impl.rs" 2>/dev/null; then
        log_pass "LlmContextBuilderService → LlmContextBuilderServiceImpl"
    else
        log_fail "LlmContextBuilderService trait has no implementation"
    fi
else
    log_fail "LlmContextBuilderService trait not found"
fi

# ---------------------------------------------------------------------------
# Check 2: Factory Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Factory Contracts ---"

if grep -q 'pub trait LlmStepFactory' "$SRC_DIR/$MODULE/application/factory.rs" 2>/dev/null; then
    if grep -q 'impl.*LlmStepFactory' "$SRC_DIR/$MODULE/application/factory_impl.rs" 2>/dev/null; then
        log_pass "LlmStepFactory → LlmStepFactoryImpl"
    else
        log_fail "LlmStepFactory trait has no implementation"
    fi
else
    log_fail "LlmStepFactory trait not found"
fi

if grep -q 'pub trait LlmContextBuilderFactory' "$SRC_DIR/$MODULE/application/factory.rs" 2>/dev/null; then
    if grep -q 'impl.*LlmContextBuilderFactory' "$SRC_DIR/$MODULE/application/factory_impl.rs" 2>/dev/null; then
        log_pass "LlmContextBuilderFactory → LlmContextBuilderFactoryImpl"
    else
        log_fail "LlmContextBuilderFactory trait has no implementation"
    fi
else
    log_fail "LlmContextBuilderFactory trait not found"
fi

if grep -q 'pub trait LlmProviderClientFactory' "$SRC_DIR/$MODULE/application/factory.rs" 2>/dev/null; then
    if grep -q 'impl.*LlmProviderClientFactory' "$SRC_DIR/$MODULE/application/factory_impl.rs" 2>/dev/null; then
        log_pass "LlmProviderClientFactory → LlmProviderClientFactoryImpl"
    else
        log_fail "LlmProviderClientFactory trait has no implementation"
    fi
else
    log_fail "LlmProviderClientFactory trait not found"
fi

if grep -q 'pub trait LlmProviderClient' "$SRC_DIR/$MODULE/application/factory.rs" 2>/dev/null; then
    log_pass "LlmProviderClient trait defined"
else
    log_fail "LlmProviderClient trait not found"
fi

# ---------------------------------------------------------------------------
# Check 3: Repository Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Repository Contracts ---"

REPO_FILE="$SRC_DIR/$MODULE/infrastructure/repository/mod.rs"

if grep -q 'pub trait LlmGenerateNodeRepository' "$REPO_FILE" 2>/dev/null; then
    if grep -q 'impl.*LlmGenerateNodeRepository' "$SRC_DIR/$MODULE/infrastructure/repository/node_repository_impl.rs" 2>/dev/null; then
        log_pass "LlmGenerateNodeRepository → InMemoryNodeRepository"
    else
        log_fail "LlmGenerateNodeRepository trait has no implementation"
    fi
else
    log_fail "LlmGenerateNodeRepository trait not found"
fi

# ---------------------------------------------------------------------------
# Check 4: Domain Entities and Types
# ---------------------------------------------------------------------------
echo ""
echo "--- Domain Entities ---"

DOMAIN_DIR="$SRC_DIR/$MODULE/domain"

if grep -q 'pub struct LlmGenerateNode' "$DOMAIN_DIR/generate_node.rs" 2>/dev/null; then
    log_pass "LlmGenerateNode struct exists"
else
    log_fail "LlmGenerateNode struct not found"
fi

if grep -q 'pub struct LlmModelConfig' "$DOMAIN_DIR/generate_node.rs" 2>/dev/null; then
    log_pass "LlmModelConfig struct exists"
else
    log_fail "LlmModelConfig struct not found"
fi

if grep -q 'pub struct LlmOutputSchema' "$DOMAIN_DIR/generate_node.rs" 2>/dev/null; then
    log_pass "LlmOutputSchema struct exists"
else
    log_fail "LlmOutputSchema struct not found"
fi

if grep -q 'pub enum LlmOutputFormat' "$DOMAIN_DIR/generate_node.rs" 2>/dev/null; then
    log_pass "LlmOutputFormat enum exists"
else
    log_fail "LlmOutputFormat enum not found"
fi

if grep -q 'pub enum LlmGenerateNodeState' "$DOMAIN_DIR/generate_node.rs" 2>/dev/null; then
    log_pass "LlmGenerateNodeState enum exists"
else
    log_fail "LlmGenerateNodeState enum not found"
fi

if grep -q 'pub struct LlmGenerationOutput' "$DOMAIN_DIR/generate_node.rs" 2>/dev/null; then
    log_pass "LlmGenerationOutput struct exists"
else
    log_fail "LlmGenerationOutput struct not found"
fi

if grep -q 'pub struct LlmStepContext' "$DOMAIN_DIR/step_context.rs" 2>/dev/null; then
    log_pass "LlmStepContext struct exists"
else
    log_fail "LlmStepContext struct not found"
fi

if grep -q 'pub struct SourceContext' "$DOMAIN_DIR/step_context.rs" 2>/dev/null; then
    log_pass "SourceContext struct exists"
else
    log_fail "SourceContext struct not found"
fi

if grep -q 'pub struct FailureContext' "$DOMAIN_DIR/step_context.rs" 2>/dev/null; then
    log_pass "FailureContext struct exists"
else
    log_fail "FailureContext struct not found"
fi

if grep -q 'pub struct ExecutionContext' "$DOMAIN_DIR/step_context.rs" 2>/dev/null; then
    log_pass "ExecutionContext struct exists"
else
    log_fail "ExecutionContext struct not found"
fi

if grep -q 'pub struct PreviousAttempt' "$DOMAIN_DIR/step_context.rs" 2>/dev/null; then
    log_pass "PreviousAttempt struct exists"
else
    log_fail "PreviousAttempt struct not found"
fi

if grep -q 'pub enum LlmStepError' "$DOMAIN_DIR/error.rs" 2>/dev/null; then
    log_pass "LlmStepError enum exists"
else
    log_fail "LlmStepError enum not found"
fi

if grep -q 'pub enum LlmStepEvent' "$DOMAIN_DIR/event/mod.rs" 2>/dev/null; then
    log_pass "LlmStepEvent enum exists"
else
    log_fail "LlmStepEvent enum not found"
fi

# ---------------------------------------------------------------------------
# Check 5: DTOs exist
# ---------------------------------------------------------------------------
echo ""
echo "--- DTOs ---"

DTO_FILE="$SRC_DIR/$MODULE/application/dto/mod.rs"

for dto in CreateNodeInput CreateNodeOutput BuildContextInput BuildContextOutput \
           GenerateInput GenerateOutput ExecuteStepInput ExecuteStepOutput \
           GetSourceContextInput GetSourceContextOutput GetFailureContextInput GetFailureContextOutput \
           RetryGenerationInput RetryGenerationOutput ValidateNodeConfigInput ValidateNodeConfigOutput \
           LlmStepSummary; do
    if grep -q "pub struct $dto" "$DTO_FILE" 2>/dev/null; then
        log_pass "$dto DTO exists"
    else
        log_fail "$dto DTO not found"
    fi
done

# ---------------------------------------------------------------------------
# Check 6: HTTP Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- HTTP Contracts ---"

HTTP_FILE="$SRC_DIR/$MODULE/interfaces/http/mod.rs"

for endpoint in API_BASE_PATH CREATE_NODE_PATH GET_NODE_PATH BUILD_CONTEXT_PATH \
                EXECUTE_STEP_PATH RETRY_STEP_PATH LIST_NODES_PATH DELETE_NODE_PATH \
                VALIDATE_CONFIG_PATH HEALTH_PATH; do
    if grep -q "pub const $endpoint" "$HTTP_FILE" 2>/dev/null; then
        log_pass "HTTP endpoint $endpoint exists"
    else
        log_fail "HTTP endpoint $endpoint not found"
    fi
done

if grep -q 'pub struct ApiErrorResponse' "$HTTP_FILE" 2>/dev/null; then
    log_pass "ApiErrorResponse exists"
else
    log_fail "ApiErrorResponse not found"
fi

if grep -q 'pub mod error_codes' "$HTTP_FILE" 2>/dev/null; then
    log_pass "Error codes defined"
else
    log_fail "Error codes not defined"
fi

if grep -q 'pub mod status_codes' "$HTTP_FILE" 2>/dev/null; then
    log_pass "Status codes defined"
else
    log_fail "Status codes not defined"
fi

# ---------------------------------------------------------------------------
# Check 7: Provider Client Implementations
# ---------------------------------------------------------------------------
echo ""
echo "--- Provider Clients ---"

CLIENT_FILE="$SRC_DIR/$MODULE/infrastructure/llm_provider_client_impl.rs"

if grep -q 'impl.*LlmProviderClient for MockLlmProviderClient' "$CLIENT_FILE" 2>/dev/null; then
    log_pass "MockLlmProviderClient implements LlmProviderClient"
else
    log_fail "MockLlmProviderClient implementation not found"
fi

if grep -q 'impl.*LlmProviderClient for AnthropicProviderClient' "$CLIENT_FILE" 2>/dev/null; then
    log_pass "AnthropicProviderClient implements LlmProviderClient"
else
    log_fail "AnthropicProviderClient implementation not found"
fi

if grep -q 'impl.*LlmProviderClient for OpenAiProviderClient' "$CLIENT_FILE" 2>/dev/null; then
    log_pass "OpenAiProviderClient implements LlmProviderClient"
else
    log_fail "OpenAiProviderClient implementation not found"
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
    echo "Some llm-step contracts are missing implementations."
    exit 1
fi

echo "All llm-step contracts have implementations."
exit 0
