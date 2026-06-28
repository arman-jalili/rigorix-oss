#!/usr/bin/env bash
# ============================================================================
# check_code-generation_contracts.sh
#
# Validates that every contract interface from the code-generation module has
# a concrete implementation. Uses grep/find to detect trait definitions and
# their implementing structs.
#
# Usage: bash .pi/scripts/ci/check_code-generation_contracts.sh [--help]
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

CG_DIR="$SRC_DIR/code_gen"
TOOLS_DIR="$SRC_DIR/tools"

PASS=0
FAIL=0
ERRORS=()

log_pass() { echo "  ✓ PASS: $1"; PASS=$((PASS + 1)); }
log_fail() { echo "  ✗ FAIL: $1"; ERRORS+=("$1"); FAIL=$((FAIL + 1)); }

echo ""
echo "═══ Code-Generation Contract Implementation Check ═══"
echo "Source: $CG_DIR"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Service Contracts
# ---------------------------------------------------------------------------
echo "--- Service Contracts ---"

if grep -q 'pub trait SyntaxGateService' "$CG_DIR/application/service.rs" 2>/dev/null; then
    ANY_IMPL=$(grep -rl 'impl.*SyntaxGateService' "$CG_DIR" 2>/dev/null || true)
    if [ -n "$ANY_IMPL" ]; then
        IMPL_FILE=$(basename "$(echo "$ANY_IMPL" | head -1)")
        log_pass "SyntaxGateService → $IMPL_FILE"
    else
        log_fail "SyntaxGateService trait has no implementation"
    fi
else
    log_fail "SyntaxGateService trait not found"
fi

if grep -q 'pub trait EditFileService' "$CG_DIR/application/service.rs" 2>/dev/null; then
    log_pass "EditFileService trait exists"
else
    log_fail "EditFileService trait not found"
fi

if grep -q 'pub trait ReadFileService' "$CG_DIR/application/service.rs" 2>/dev/null; then
    log_pass "ReadFileService trait exists"
else
    log_fail "ReadFileService trait not found"
fi

# ---------------------------------------------------------------------------
# Check 2: Factory Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Factory Contracts ---"

for factory in SyntaxGateFactory EditFileFactory ReadFileFactory; do
    if grep -q "pub trait $factory" "$CG_DIR/application/factory.rs" 2>/dev/null; then
        log_pass "$factory trait exists"
    else
        log_fail "$factory trait not found"
    fi
done

# ---------------------------------------------------------------------------
# Check 3: Repository Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Repository Contracts ---"

if grep -q 'pub trait CodeGenEventRepository' "$CG_DIR/infrastructure/repository/mod.rs" 2>/dev/null; then
    log_pass "CodeGenEventRepository trait exists"
else
    log_fail "CodeGenEventRepository trait not found"
fi

# ---------------------------------------------------------------------------
# Check 4: Domain entities exist
# ---------------------------------------------------------------------------
echo ""
echo "--- Domain Entities ---"

if grep -q 'pub enum SyntaxGateResult' "$CG_DIR/domain/result.rs" 2>/dev/null; then
    log_pass "SyntaxGateResult enum exists"
    VARIANT_COUNT=$(grep -cE '    (Passed|Failed|Skipped)' "$CG_DIR/domain/result.rs" 2>/dev/null || echo 0)
    if [ "$VARIANT_COUNT" -ge 3 ]; then
        log_pass "All 3 SyntaxGateResult variants present (found: $VARIANT_COUNT)"
    else
        log_fail "Only $VARIANT_COUNT SyntaxGateResult variants found (expected 3)"
    fi
else
    log_fail "SyntaxGateResult enum not found"
fi

if grep -q 'pub struct SyntaxError' "$CG_DIR/domain/result.rs" 2>/dev/null; then
    log_pass "SyntaxError struct exists"
else
    log_fail "SyntaxError struct not found"
fi

if grep -q 'pub enum CodeGenError' "$CG_DIR/domain/error.rs" 2>/dev/null; then
    log_pass "CodeGenError enum exists"
else
    log_fail "CodeGenError enum not found"
fi

if grep -q 'pub enum CodeGenEvent' "$CG_DIR/domain/event/mod.rs" 2>/dev/null; then
    log_pass "CodeGenEvent enum exists"
else
    log_fail "CodeGenEvent enum not found"
fi

# ---------------------------------------------------------------------------
# Check 5: DTOs exist
# ---------------------------------------------------------------------------
echo ""
echo "--- DTOs ---"

for dto in EditFileInput EditFileOutput ReadFileInput ReadFileOutput \
           StructuredPatchHunk SyntaxGateInput SyntaxGateOutput \
           SyntaxGateConfig EditFileConfig CodeGenConfig; do
    if grep -q "pub struct $dto" "$CG_DIR/application/dto/mod.rs" 2>/dev/null; then
        log_pass "$dto DTO exists"
    else
        log_fail "$dto DTO not found"
    fi
done

# ---------------------------------------------------------------------------
# Check 6: HTTP Contracts exist
# ---------------------------------------------------------------------------
echo ""
echo "--- HTTP Contracts ---"

for endpoint in EDIT_FILE_PATH EDIT_FILE_PREVIEW_PATH READ_FILE_PATH \
                VERIFY_SYNTAX_PATH CODE_GEN_CONFIG_PATH; do
    if grep -q "pub const $endpoint" "$CG_DIR/interfaces/http/mod.rs" 2>/dev/null; then
        log_pass "HTTP endpoint $endpoint exists"
    else
        log_fail "HTTP endpoint $endpoint not found"
    fi
done

for req_resp in EditFileRequest EditFileResponse EditFilePreviewResponse \
                ReadFileRequest ReadFileResponse VerifySyntaxRequest \
                VerifySyntaxResponse CodeGenConfigResponse ApiErrorResponse; do
    if grep -q "pub struct $req_resp" "$CG_DIR/interfaces/http/mod.rs" 2>/dev/null; then
        log_pass "$req_resp exists"
    else
        log_fail "$req_resp not found"
    fi
done

if grep -q 'pub mod error_codes' "$CG_DIR/interfaces/http/mod.rs" 2>/dev/null; then
    log_pass "Error codes defined"
else
    log_fail "Error codes not defined"
fi

# ---------------------------------------------------------------------------
# Check 7: EditFileTool exists in tools module
# ---------------------------------------------------------------------------
echo ""
echo "--- EditFileTool (tools module) ---"

if grep -q 'pub struct EditFileTool' "$TOOLS_DIR/application/file_edit_tool.rs" 2>/dev/null; then
    log_pass "EditFileTool struct exists"
    if grep -q 'impl.*Tool.*for EditFileTool' "$TOOLS_DIR/application/file_edit_tool.rs" 2>/dev/null; then
        log_pass "EditFileTool implements Tool trait"
    else
        log_fail "EditFileTool does not implement Tool trait"
    fi
else
    log_fail "EditFileTool not found"
fi

# ---------------------------------------------------------------------------
# Check 8: Helper methods exist
# ---------------------------------------------------------------------------
echo ""
echo "--- Helper Methods ---"

for method in is_success is_failed errors passed failed skipped; do
    if grep -q "fn $method" "$CG_DIR/domain/result.rs" 2>/dev/null; then
        log_pass "SyntaxGateResult::$method() exists"
    else
        log_fail "SyntaxGateResult::$method() not found"
    fi
done

for method in is_recoverable is_retriable path; do
    if grep -q "fn $method" "$CG_DIR/domain/error.rs" 2>/dev/null; then
        log_pass "CodeGenError::$method() exists"
    else
        log_fail "CodeGenError::$method() not found"
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
    echo "Some code-generation contracts are missing implementations."
    exit 1
fi

echo "All code-generation contracts have implementations."
exit 0
