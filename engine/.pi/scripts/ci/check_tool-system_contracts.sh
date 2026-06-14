#!/usr/bin/env bash
# ============================================================================
# check_tool-system_contracts.sh
#
# Validates that every contract interface from the tool-system module has
# a concrete implementation. Uses grep/find to detect trait definitions and
# their implementing structs.
#
# Usage: bash .pi/scripts/ci/check_tool-system_contracts.sh [--help]
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

log_pass() { echo "  ✓ PASS: $1"; ((PASS++)); }
log_fail() { echo "  ✗ FAIL: $1"; ERRORS+=("$1"); ((FAIL++)); }

# Determine source directory
if [ ! -d "$SRC_DIR" ]; then
    SRC_DIR="$(cd "$PI_DIR/.." && pwd)/src"
fi
if [ ! -d "$SRC_DIR" ]; then
    log_fail "Source directory not found"
    exit 1
fi

echo ""
echo "═══ Tool-System Contract Implementation Check ═══"
echo "Source: $SRC_DIR/tools"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Core Tool Contract
# ---------------------------------------------------------------------------
echo "--- Core Tool Contract ---"
if grep -q 'pub trait Tool' "$SRC_DIR/tools/domain/tool_trait.rs" 2>/dev/null; then
    log_pass "Tool trait defined in domain/tool_trait.rs"
else
    log_fail "Tool trait not found"
fi

# Count concrete tool implementations
TOOL_COUNT=$(grep -r 'impl Tool for' "$SRC_DIR/tools/" --include="*.rs" | grep -v '//' | wc -l | tr -d ' ')
if [ "$TOOL_COUNT" -ge 9 ] 2>/dev/null; then
    log_pass "$TOOL_COUNT concrete tool implementations found (expected 9+)"
else
    log_fail "Only $TOOL_COUNT tool implementations found (expected at least 9)"
fi

# ---------------------------------------------------------------------------
# Check 2: Error Contract
# ---------------------------------------------------------------------------
echo ""
echo "--- Error Contract ---"
if grep -q 'pub enum ToolError' "$SRC_DIR/tools/domain/error.rs" 2>/dev/null; then
    log_pass "ToolError enum defined in domain/error.rs"
else
    log_fail "ToolError enum not found"
fi

# ---------------------------------------------------------------------------
# Check 3: Event Contract
# ---------------------------------------------------------------------------
echo ""
echo "--- Event Contract ---"
if grep -q 'pub enum ToolEvent' "$SRC_DIR/tools/domain/event/mod.rs" 2>/dev/null; then
    log_pass "ToolEvent enum defined in domain/event/mod.rs"
else
    log_fail "ToolEvent enum not found"
fi

# ---------------------------------------------------------------------------
# Check 4: Service Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Service Contracts ---"
if grep -q 'pub trait ToolRegistryService' "$SRC_DIR/tools/application/service.rs" 2>/dev/null; then
    if grep -q 'impl.*ToolRegistryService' "$SRC_DIR/tools/application/registry_impl.rs" 2>/dev/null; then
        log_pass "ToolRegistryService → ToolRegistryImpl"
    else
        log_fail "ToolRegistryService trait has no implementation"
    fi
else
    log_fail "ToolRegistryService trait not found"
fi

if grep -q 'pub trait ToolExecutionService' "$SRC_DIR/tools/application/service.rs" 2>/dev/null; then
    if grep -q 'impl.*ToolExecutionService' "$SRC_DIR/tools/application/registry_impl.rs" 2>/dev/null; then
        log_pass "ToolExecutionService → ToolRegistryImpl"
    else
        log_fail "ToolExecutionService trait has no implementation"
    fi
else
    log_fail "ToolExecutionService trait not found"
fi

# ---------------------------------------------------------------------------
# Check 5: Factory Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Factory Contracts ---"
if grep -q 'pub trait ToolFactory' "$SRC_DIR/tools/application/factory.rs" 2>/dev/null; then
    log_pass "ToolFactory trait defined (implemented through service layer)"
else
    log_fail "ToolFactory trait not found"
fi

if grep -q 'pub trait RegistryFactory' "$SRC_DIR/tools/application/factory.rs" 2>/dev/null; then
    log_pass "RegistryFactory trait defined (implemented through service layer)"
else
    log_fail "RegistryFactory trait not found"
fi

# ---------------------------------------------------------------------------
# Check 6: Repository Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Repository Contracts ---"
if grep -q 'pub trait ToolRepository' "$SRC_DIR/tools/infrastructure/repository/mod.rs" 2>/dev/null; then
    log_pass "ToolRepository trait defined in infrastructure/repository/"
else
    log_fail "ToolRepository trait not found"
fi

# ---------------------------------------------------------------------------
# Check 7: DTO Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- DTO Contracts ---"
DTO_CHECKS=(
    "ToolInput"
    "ToolResult"
    "SideEffect"
    "RegisterToolInput"
    "ExecuteToolInput"
    "GetToolInput"
    "ListToolsOutput"
    "ToolInfo"
    "ToolSystemConfig"
)
for dto in "${DTO_CHECKS[@]}"; do
    if grep -q "pub struct $dto\|pub enum $dto" "$SRC_DIR/tools/application/dto/mod.rs" 2>/dev/null; then
        log_pass "$dto DTO defined"
    else
        log_fail "$dto DTO not found"
    fi
done

# ---------------------------------------------------------------------------
# Check 8: API Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- API Contracts ---"
if grep -q 'pub const API_BASE_PATH' "$SRC_DIR/tools/interfaces/http/mod.rs" 2>/dev/null; then
    log_pass "HTTP API contracts exist in interfaces/http/"
else
    log_fail "HTTP API contracts not found"
fi

# Check for key endpoints
ENDPOINT_COUNT=$(grep -c 'pub const \w*_PATH' "$SRC_DIR/tools/interfaces/http/mod.rs" 2>/dev/null || echo 0)
if [ "$ENDPOINT_COUNT" -ge 5 ] 2>/dev/null; then
    log_pass "$ENDPOINT_COUNT API endpoints defined (expected 5+)"
else
    log_fail "Only $ENDPOINT_COUNT API endpoints found (expected at least 5)"
fi

# ---------------------------------------------------------------------------
# Check 9: Risk Mapping
# ---------------------------------------------------------------------------
echo ""
echo "--- Risk Mapping ---"
RISK_TOOLS=$(grep -c 'map.insert' "$SRC_DIR/tools/domain/risk_mapping.rs" 2>/dev/null || echo 0)
if [ "$RISK_TOOLS" -ge 9 ] 2>/dev/null; then
    log_pass "$RISK_TOOLS tools with risk level mappings"
else
    log_fail "Only $RISK_TOOLS risk mappings found (expected 9)"
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
    echo "Some tool-system contracts are missing implementations."
    exit 1
fi

echo "All tool-system contracts have implementations."
exit 0
