#!/usr/bin/env bash
# ============================================================================
# check_failure-parser_contracts.sh
#
# Validates that every contract interface from the failure-parser
# module has a concrete implementation. Uses grep/find to detect trait
# definitions and their implementing structs.
#
# Usage: bash .pi/scripts/ci/check_failure-parser_contracts.sh [--help]
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

FP_DIR="$SRC_DIR/failure_parser"
echo ""
echo "═══ Failure Parser Contract Implementation Check ═══"
echo "Source: $FP_DIR"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Domain Entities
# ---------------------------------------------------------------------------
echo "--- Domain Entities ---"

if [ -f "$FP_DIR/domain/failure.rs" ] && grep -q 'pub enum TemplateFailure' "$FP_DIR/domain/failure.rs" 2>/dev/null; then
    log_pass "TemplateFailure enum exists"
else
    log_fail "TemplateFailure enum not found"
fi

if [ -f "$FP_DIR/domain/failure.rs" ] && grep -q 'pub struct SourceLocation' "$FP_DIR/domain/failure.rs" 2>/dev/null; then
    log_pass "SourceLocation struct exists"
else
    log_fail "SourceLocation struct not found"
fi

if [ -f "$FP_DIR/domain/detail.rs" ] && grep -q 'pub struct FailureDetail' "$FP_DIR/domain/detail.rs" 2>/dev/null; then
    log_pass "FailureDetail struct exists"
else
    log_fail "FailureDetail struct not found"
fi

if [ -f "$FP_DIR/domain/detail.rs" ] && grep -q 'pub enum FailureSeverity' "$FP_DIR/domain/detail.rs" 2>/dev/null; then
    log_pass "FailureSeverity enum exists"
else
    log_fail "FailureSeverity enum not found"
fi

if [ -f "$FP_DIR/domain/input.rs" ] && grep -q 'pub struct CompilerOutput' "$FP_DIR/domain/input.rs" 2>/dev/null; then
    log_pass "CompilerOutput struct exists"
else
    log_fail "CompilerOutput struct not found"
fi

if [ -f "$FP_DIR/domain/output.rs" ] && grep -q 'pub struct ParsedFailure' "$FP_DIR/domain/output.rs" 2>/dev/null; then
    log_pass "ParsedFailure struct exists"
else
    log_fail "ParsedFailure struct not found"
fi

if [ -f "$FP_DIR/domain/output.rs" ] && grep -q 'pub struct SourceContext' "$FP_DIR/domain/output.rs" 2>/dev/null; then
    log_pass "SourceContext struct exists"
else
    log_fail "SourceContext struct not found"
fi

if [ -f "$FP_DIR/domain/error.rs" ] && grep -q 'pub enum FailureParserError' "$FP_DIR/domain/error.rs" 2>/dev/null; then
    log_pass "FailureParserError enum exists"
else
    log_fail "FailureParserError enum not found"
fi

if [ -f "$FP_DIR/domain/event/mod.rs" ] && grep -q 'pub enum FailureParserEvent' "$FP_DIR/domain/event/mod.rs" 2>/dev/null; then
    log_pass "FailureParserEvent enum exists"
else
    log_fail "FailureParserEvent enum not found"
fi

if [ -f "$FP_DIR/domain/registry.rs" ] && grep -q 'pub trait LanguageParser' "$FP_DIR/domain/registry.rs" 2>/dev/null; then
    log_pass "LanguageParser trait exists"
else
    log_fail "LanguageParser trait not found"
fi

if [ -f "$FP_DIR/domain/template_service.rs" ] && grep -q 'pub struct TemplateFailureService' "$FP_DIR/domain/template_service.rs" 2>/dev/null; then
    log_pass "TemplateFailureService exists"
else
    log_fail "TemplateFailureService not found"
fi

echo ""

# ---------------------------------------------------------------------------
# Check 2: Application Layer
# ---------------------------------------------------------------------------
echo "--- Application Layer ---"

if [ -f "$FP_DIR/application/service.rs" ] && grep -q 'pub trait FailureParserService' "$FP_DIR/application/service.rs" 2>/dev/null; then
    log_pass "FailureParserService trait exists"
else
    log_fail "FailureParserService trait not found"
fi

if [ -f "$FP_DIR/application/service.rs" ] && grep -q 'pub trait FixSuggestionService' "$FP_DIR/application/service.rs" 2>/dev/null; then
    log_pass "FixSuggestionService trait exists"
else
    log_fail "FixSuggestionService trait not found"
fi

if [ -f "$FP_DIR/application/service_impl.rs" ] && grep -q 'pub struct FailureParserServiceImpl' "$FP_DIR/application/service_impl.rs" 2>/dev/null; then
    log_pass "FailureParserServiceImpl (impl) exists"
else
    log_fail "FailureParserServiceImpl not found"
fi

if [ -f "$FP_DIR/application/fix_suggestion_impl.rs" ] && grep -q 'pub struct FixSuggestionServiceImpl' "$FP_DIR/application/fix_suggestion_impl.rs" 2>/dev/null; then
    log_pass "FixSuggestionServiceImpl (impl) exists"
else
    log_fail "FixSuggestionServiceImpl not found"
fi

if [ -f "$FP_DIR/application/ts_parser.rs" ] && grep -q 'pub struct TypeScriptParser' "$FP_DIR/application/ts_parser.rs" 2>/dev/null; then
    log_pass "TypeScriptParser exists"
else
    log_fail "TypeScriptParser not found"
fi

if [ -f "$FP_DIR/application/factory.rs" ] && grep -q 'pub trait ParserFactory' "$FP_DIR/application/factory.rs" 2>/dev/null; then
    log_pass "ParserFactory trait exists"
else
    log_fail "ParserFactory trait not found"
fi

echo ""

# ---------------------------------------------------------------------------
# Check 3: Infrastructure Layer
# ---------------------------------------------------------------------------
echo "--- Infrastructure Layer ---"

if [ -f "$FP_DIR/infrastructure/repository/mod.rs" ] && grep -q 'pub trait ParserConfigRepository' "$FP_DIR/infrastructure/repository/mod.rs" 2>/dev/null; then
    log_pass "ParserConfigRepository trait exists"
else
    log_fail "ParserConfigRepository trait not found"
fi

if [ -f "$FP_DIR/infrastructure/repository/mod.rs" ] && grep -q 'pub trait FailureLogRepository' "$FP_DIR/infrastructure/repository/mod.rs" 2>/dev/null; then
    log_pass "FailureLogRepository trait exists"
else
    log_fail "FailureLogRepository trait not found"
fi

echo ""

# ---------------------------------------------------------------------------
# Check 4: Interface Layer (HTTP)
# ---------------------------------------------------------------------------
echo "--- Interface Layer ---"

if [ -f "$FP_DIR/interfaces/http/mod.rs" ] && grep -q 'pub const API_BASE_PATH' "$FP_DIR/interfaces/http/mod.rs" 2>/dev/null; then
    log_pass "HTTP API contracts exist"
else
    log_fail "HTTP API contracts not found"
fi

echo ""

# ---------------------------------------------------------------------------
# Check 5: Test Files
# ---------------------------------------------------------------------------
echo "--- Test Files ---"

# Integration tests
INTEGRATION_COUNT=0
for test_file in "failure_parser_template_integration.rs" "failure_parser_service_integration.rs" "failure_parser_typescript_integration.rs" "failure_parser_suggestion_integration.rs"; do
    if [ -f "$SRC_DIR/../tests/$test_file" ] || [ -f "$PI_DIR/../tests/$test_file" ]; then
        log_pass "Integration test: $test_file"
        ((INTEGRATION_COUNT++))
    else
        log_fail "Integration test missing: $test_file"
    fi
done

echo ""

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo "═══ Summary ═══"
echo "  Passed: $PASS"
echo "  Failed: $FAIL"
echo ""

if [ ${#ERRORS[@]} -gt 0 ]; then
    echo "VIOLATIONS:"
    for err in "${ERRORS[@]}"; do
        echo "  - $err"
    done
    echo ""
    echo "Failure-parser contract check FAILED."
    exit 1
fi

echo "Failure-parser contract check PASSED."
exit 0
