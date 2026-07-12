#!/usr/bin/env bash
# Check MCP Server Test Coverage
#
# Runs cargo test for the mcp-server module and counts test functions.
# Asserts minimum coverage thresholds per module.
#
# Usage: bash .pi/scripts/ci/check_mcp-server_coverage.sh
# Exit: 0 = coverage meets thresholds, 1 = insufficient coverage

set -euo pipefail

MIN_TESTS=15

cd "$(git rev-parse --show-toplevel 2>/dev/null || echo ".")"

echo "═══ MCP Server Coverage Checks ═══"
echo ""

# Run tests first
echo "Running tests..."
if cargo test -p rigorix-mcp 2>&1 | tail -5; then
    echo "  ✓ All tests pass"
else
    echo "  ✗ Tests failed"
    exit 1
fi

# Count test functions
TEST_COUNT=$(grep -rh "fn test_" mcp/src/ mcp/tests/ --include="*.rs" 2>/dev/null | wc -l | tr -d ' ')
echo "  Test functions found: $TEST_COUNT"
echo "  Minimum required: $MIN_TESTS"

if [ "$TEST_COUNT" -lt "$MIN_TESTS" ]; then
    echo "  ✗ FAIL: Insufficient test coverage ($TEST_COUNT < $MIN_TESTS)"
    exit 1
fi

echo "  ✓ PASS: Coverage threshold met"
echo ""
echo "═══ Result: PASSED ═══"
exit 0
