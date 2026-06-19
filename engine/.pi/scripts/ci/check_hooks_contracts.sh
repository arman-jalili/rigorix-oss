#!/usr/bin/env bash
# ============================================================================
# check_hooks_contracts.sh
#
# Validates that every contract interface from the hooks module has a
# concrete implementation. Uses grep/find to detect trait definitions and
# their implementing structs.
#
# Usage: bash .pi/scripts/ci/check_hooks_contracts.sh [--help]
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

HOOKS_DIR="$SRC_DIR/hooks"

PASS=0
FAIL=0
ERRORS=()

log_pass() { echo "  ✓ PASS: $1"; ((PASS++)); }
log_fail() { echo "  ✗ FAIL: $1"; ERRORS+=("$1"); ((FAIL++)); }

echo ""
echo "═══ Hooks Contract Implementation Check ═══"
echo "Source: $HOOKS_DIR"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Service Contracts
# ---------------------------------------------------------------------------
echo "--- Service Contracts ---"

if grep -q 'pub trait HookRunnerService' "$HOOKS_DIR/application/service.rs" 2>/dev/null; then
    # Check for any implementation of HookRunnerService
    ANY_IMPL=$(grep -rl 'impl.*HookRunnerService' "$HOOKS_DIR" 2>/dev/null || true)
    if [ -n "$ANY_IMPL" ]; then
        IMPL_FILE=$(basename "$(echo "$ANY_IMPL" | head -1)")
        log_pass "HookRunnerService → $IMPL_FILE"
    else
        log_fail "HookRunnerService trait has no implementation"
    fi
else
    log_fail "HookRunnerService trait not found"
fi

if grep -q 'pub trait HookCommandExecutor' "$HOOKS_DIR/application/service.rs" 2>/dev/null; then
    ANY_IMPL=$(grep -rl 'impl.*HookCommandExecutor' "$HOOKS_DIR" 2>/dev/null || true)
    if [ -n "$ANY_IMPL" ]; then
        IMPL_FILE=$(basename "$(echo "$ANY_IMPL" | head -1)")
        log_pass "HookCommandExecutor → $IMPL_FILE"
    else
        log_fail "HookCommandExecutor trait has no implementation"
    fi
else
    log_fail "HookCommandExecutor trait not found"
fi

# ---------------------------------------------------------------------------
# Check 2: Factory Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Factory Contracts ---"

if grep -q 'pub trait HookRunnerFactory' "$HOOKS_DIR/application/factory.rs" 2>/dev/null; then
    ANY_IMPL=$(grep -rl 'impl.*HookRunnerFactory' "$HOOKS_DIR" 2>/dev/null || true)
    if [ -n "$ANY_IMPL" ]; then
        IMPL_FILE=$(basename "$(echo "$ANY_IMPL" | head -1)")
        log_pass "HookRunnerFactory → $IMPL_FILE"
    else
        log_fail "HookRunnerFactory trait has no implementation"
    fi
else
    log_fail "HookRunnerFactory trait not found"
fi

# ---------------------------------------------------------------------------
# Check 3: Repository Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Repository Contracts ---"

if grep -q 'pub trait HookCommandRepository' "$HOOKS_DIR/infrastructure/repository/mod.rs" 2>/dev/null; then
    ANY_IMPL=$(grep -rl 'impl.*HookCommandRepository' "$HOOKS_DIR" 2>/dev/null || true)
    if [ -n "$ANY_IMPL" ]; then
        IMPL_FILE=$(basename "$(echo "$ANY_IMPL" | head -1)")
        log_pass "HookCommandRepository → $IMPL_FILE"
    else
        log_fail "HookCommandRepository trait has no implementation"
    fi
else
    log_fail "HookCommandRepository trait not found"
fi

# ---------------------------------------------------------------------------
# Check 4: Domain entities exist
# ---------------------------------------------------------------------------
echo ""
echo "--- Domain Entities ---"

if grep -q 'pub enum HookEvent' "$HOOKS_DIR/domain/event.rs" 2>/dev/null; then
    log_pass "HookEvent enum exists"
    VARIANT_COUNT=$(grep -cE '    (PreToolUse|PostToolUse|PostToolUseFailure)($|[ ,])' "$HOOKS_DIR/domain/event.rs" 2>/dev/null || echo 0)
    if [ "$VARIANT_COUNT" -ge 3 ]; then
        log_pass "All 3 HookEvent variants present (found: $VARIANT_COUNT)"
    else
        log_fail "Only $VARIANT_COUNT HookEvent variants found (expected 3)"
    fi
else
    log_fail "HookEvent enum not found"
fi

if grep -q 'pub struct HookRunResult' "$HOOKS_DIR/domain/result.rs" 2>/dev/null; then
    log_pass "HookRunResult struct exists"
else
    log_fail "HookRunResult struct not found"
fi

if grep -q 'pub struct HookConfig' "$HOOKS_DIR/domain/config.rs" 2>/dev/null; then
    log_pass "HookConfig struct exists"
else
    log_fail "HookConfig struct not found"
fi

if grep -q 'pub enum HookError' "$HOOKS_DIR/domain/error.rs" 2>/dev/null; then
    log_pass "HookError enum exists"
else
    log_fail "HookError enum not found"
fi

if grep -q 'pub struct HookAbortSignal' "$HOOKS_DIR/domain/abort.rs" 2>/dev/null; then
    log_pass "HookAbortSignal struct exists"
else
    log_fail "HookAbortSignal struct not found"
fi

# ---------------------------------------------------------------------------
# Check 5: Protocol types exist
# ---------------------------------------------------------------------------
echo ""
echo "--- Protocol Types ---"

for proto in HookStdinPayload HookStdoutResponse HookDecision HookPermissionOverride; do
    if grep -q "pub \\(enum\\|struct\\) $proto" "$HOOKS_DIR/domain/protocol.rs" 2>/dev/null; then
        log_pass "$proto exists"
    else
        log_fail "$proto not found"
    fi
done

# ---------------------------------------------------------------------------
# Check 6: Event payload types exist
# ---------------------------------------------------------------------------
echo ""
echo "--- Event Payload Types ---"

for payload in HookExecutionStartedPayload HookExecutionCompletedPayload \
               HookExecutionFailedPayload HookExecutionAbortedPayload HookEventPayload; do
    if grep -q "pub \\(enum\\|struct\\) $payload" "$HOOKS_DIR/domain/event_payload.rs" 2>/dev/null; then
        log_pass "$payload exists"
    else
        log_fail "$payload not found"
    fi
done

# ---------------------------------------------------------------------------
# Check 7: DTOs exist
# ---------------------------------------------------------------------------
echo ""
echo "--- DTOs ---"

for dto in RunPreToolUseInput RunPreToolUseOutput RunPostToolUseInput RunPostToolUseOutput \
           RunPostToolUseFailureInput RunPostToolUseFailureOutput RunHooksInput RunHooksOutput \
           HookRunnerConfigInput HookRunnerStatus; do
    if grep -q "pub struct $dto" "$HOOKS_DIR/application/dto/mod.rs" 2>/dev/null; then
        log_pass "$dto DTO exists"
    else
        log_fail "$dto DTO not found"
    fi
done

# ---------------------------------------------------------------------------
# Check 8: HTTP Contracts exist
# ---------------------------------------------------------------------------
echo ""
echo "--- HTTP Contracts ---"

for endpoint in RUN_HOOKS_PATH GET_HOOKS_CONFIG_PATH UPDATE_HOOKS_CONFIG_PATH TEST_HOOK_PATH; do
    if grep -q "pub const $endpoint" "$HOOKS_DIR/interfaces/http/mod.rs" 2>/dev/null; then
        log_pass "HTTP endpoint $endpoint exists"
    else
        log_fail "HTTP endpoint $endpoint not found"
    fi
done

for req_resp in RunHooksRequest RunHooksResponse HookConfigResponse UpdateHookConfigRequest \
                TestHookRequest TestHookResponse ApiErrorResponse; do
    if grep -q "pub struct $req_resp" "$HOOKS_DIR/interfaces/http/mod.rs" 2>/dev/null; then
        log_pass "$req_resp exists"
    else
        log_fail "$req_resp not found"
    fi
done

if grep -q 'pub mod error_codes' "$HOOKS_DIR/interfaces/http/mod.rs" 2>/dev/null; then
    log_pass "Error codes defined"
else
    log_fail "Error codes not defined"
fi

# ---------------------------------------------------------------------------
# Helper methods check
# ---------------------------------------------------------------------------
echo ""
echo "--- Helper Methods ---"

for method in is_pre_tool_use is_post_tool_use is_post_tool_use_failure as_str; do
    if grep -q "fn $method" "$HOOKS_DIR/domain/event.rs" 2>/dev/null; then
        log_pass "HookEvent::$method() exists"
    else
        log_fail "HookEvent::$method() not found"
    fi
done

for method in is_allowed is_denied allow deny; do
    if grep -q "fn $method" "$HOOKS_DIR/domain/protocol.rs" 2>/dev/null; then
        log_pass "HookStdoutResponse::$method() exists"
    else
        log_fail "HookStdoutResponse::$method() not found"
    fi
done

for method in is_allowed is_denied is_failed is_cancelled merge modified_input; do
    if grep -q "fn $method" "$HOOKS_DIR/domain/result.rs" 2>/dev/null; then
        log_pass "HookRunResult::$method() exists"
    else
        log_fail "HookRunResult::$method() not found"
    fi
done

for method in commands_for has_commands_for is_empty total_command_count; do
    if grep -q "fn $method" "$HOOKS_DIR/domain/config.rs" 2>/dev/null; then
        log_pass "HookConfig::$method() exists"
    else
        log_fail "HookConfig::$method() not found"
    fi
done

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
    echo "Some hooks contracts are missing implementations."
    exit 1
fi

echo "All hooks contracts have implementations."
exit 0
