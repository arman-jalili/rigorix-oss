#!/usr/bin/env bash
# ============================================================================
# check_event-system_contracts.sh
#
# Validates that every contract interface from the event-system module has
# a concrete implementation. Uses grep/find to detect trait definitions and
# their implementing structs.
#
# Usage: bash .pi/scripts/ci/check_event-system_contracts.sh [--help]
#
# Exit codes: 0 = all contracts implemented, 1 = violations found
# ============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PI_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Determine source directory — try engine/src first, then src
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

log_pass() { echo "  ✓ PASS: $1"; PASS=$((PASS + 1)); }
log_fail() { echo "  ✗ FAIL: $1"; ERRORS+=("$1"); FAIL=$((FAIL + 1)); }

echo ""
echo "═══ Event-System Contract Implementation Check ═══"
echo "Source: $SRC_DIR/event_system"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Service Contracts
# ---------------------------------------------------------------------------
echo "--- Service Contracts ---"

if grep -q 'pub trait EventBusService' "$SRC_DIR/event_system/application/service.rs" 2>/dev/null; then
    if grep -q 'impl.*EventBusService' "$SRC_DIR/event_system/application/event_bus_service_impl.rs" 2>/dev/null; then
        log_pass "EventBusService → EventBusServiceImpl"
    else
        log_fail "EventBusService trait has no implementation"
    fi
else
    log_fail "EventBusService trait not found"
fi

# ---------------------------------------------------------------------------
# Check 2: Factory Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Factory Contracts ---"

if grep -q 'pub trait EventBusFactory' "$SRC_DIR/event_system/application/factory.rs" 2>/dev/null; then
    if grep -q 'impl.*EventBusFactory' "$SRC_DIR/event_system/application/event_bus_factory_impl.rs" 2>/dev/null; then
        log_pass "EventBusFactory → EventBusFactoryImpl"
    else
        log_fail "EventBusFactory trait has no implementation"
    fi
else
    log_fail "EventBusFactory trait not found"
fi

# ---------------------------------------------------------------------------
# Check 3: Repository Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Repository Contracts ---"

if grep -q 'pub trait PersistedEventRepository' "$SRC_DIR/event_system/infrastructure/repository/mod.rs" 2>/dev/null; then
    if grep -q 'impl.*PersistedEventRepository' "$SRC_DIR/event_system/infrastructure/in_memory_event_repository.rs" 2>/dev/null; then
        log_pass "PersistedEventRepository → InMemoryEventRepository"
    else
        log_fail "PersistedEventRepository trait has no implementation"
    fi
else
    log_fail "PersistedEventRepository trait not found"
fi

# ---------------------------------------------------------------------------
# Check 4: Domain entities exist
# ---------------------------------------------------------------------------
echo ""
echo "--- Domain Entities ---"

if grep -q 'pub enum ExecutionEvent' "$SRC_DIR/event_system/domain/event.rs" 2>/dev/null; then
    log_pass "ExecutionEvent enum exists"
else
    log_fail "ExecutionEvent enum not found"
fi

# Verify all 11 variants exist
VARIANT_COUNT=$(grep -cE '    (PlanningStarted|PlanningCompleted|NodeStarted|NodeCompleted|NodeFailed|NodeRetrying|ToolExecuted|ExecutionCompleted|ExecutionFailed|ExecutionCancelled|BudgetWarning) \{' "$SRC_DIR/event_system/domain/event.rs" 2>/dev/null || echo 0)
if [ "$VARIANT_COUNT" -ge 11 ]; then
    log_pass "All 11 ExecutionEvent variants present (found: $VARIANT_COUNT)"
else
    log_fail "Only $VARIANT_COUNT ExecutionEvent variants found (expected 11)"
fi

if grep -q 'pub struct PersistedEvent' "$SRC_DIR/event_system/domain/event.rs" 2>/dev/null; then
    log_pass "PersistedEvent struct exists"
else
    log_fail "PersistedEvent struct not found"
fi

if grep -q 'pub enum EventSystemError' "$SRC_DIR/event_system/domain/error.rs" 2>/dev/null; then
    log_pass "EventSystemError enum exists"
else
    log_fail "EventSystemError enum not found"
fi

# ---------------------------------------------------------------------------
# Check 5: Helper methods exist on ExecutionEvent
# ---------------------------------------------------------------------------
echo ""
echo "--- Helper Methods ---"

for method in event_type_name execution_id timestamp summary is_terminal is_error; do
    if grep -q "fn $method" "$SRC_DIR/event_system/domain/event.rs" 2>/dev/null; then
        log_pass "ExecutionEvent::$method() exists"
    else
        log_fail "ExecutionEvent::$method() not found"
    fi
done

# Verify convenience constructors exist
CONSTRUCTOR_COUNT=$(grep -c 'pub fn new_' "$SRC_DIR/event_system/domain/event.rs" 2>/dev/null || echo 0)
if [ "$CONSTRUCTOR_COUNT" -ge 11 ]; then
    log_pass "All 11 convenience constructors present (found: $CONSTRUCTOR_COUNT)"
else
    log_fail "Only $CONSTRUCTOR_COUNT convenience constructors found (expected 11)"
fi

# ---------------------------------------------------------------------------
# Check 6: DTOs exist
# ---------------------------------------------------------------------------
echo ""
echo "--- DTOs ---"

for dto in PublishEventInput PublishEventOutput SubscribeInput SubscribeOutput \
           DrainPersistedInput DrainPersistedOutput EventBusStatusInput EventBusStatus \
           EventCountOutput QueryEventsInput QueryEventsOutput EventBusConfig; do
    if grep -q "pub struct $dto" "$SRC_DIR/event_system/application/dto/mod.rs" 2>/dev/null; then
        log_pass "$dto DTO exists"
    else
        log_fail "$dto DTO not found"
    fi
done

# ---------------------------------------------------------------------------
# Check 7: HTTP Contracts exist
# ---------------------------------------------------------------------------
echo ""
echo "--- HTTP Contracts ---"

for endpoint in PUBLISH_EVENT_PATH SUBSCRIBE_PATH DRAIN_EVENTS_PATH \
                QUERY_EVENTS_PATH EVENT_BUS_STATUS_PATH CLEAR_EVENTS_PATH; do
    if grep -q "pub const $endpoint" "$SRC_DIR/event_system/interfaces/http/mod.rs" 2>/dev/null; then
        log_pass "HTTP endpoint $endpoint exists"
    else
        log_fail "HTTP endpoint $endpoint not found"
    fi
done

if grep -q 'pub struct ApiErrorResponse' "$SRC_DIR/event_system/interfaces/http/mod.rs" 2>/dev/null; then
    log_pass "ApiErrorResponse exists"
else
    log_fail "ApiErrorResponse not found"
fi

if grep -q 'pub mod error_codes' "$SRC_DIR/event_system/interfaces/http/mod.rs" 2>/dev/null; then
    log_pass "Error codes defined"
else
    log_fail "Error codes not defined"
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
    echo "Some event-system contracts are missing implementations."
    exit 1
fi

echo "All event-system contracts have implementations."
exit 0
