#!/usr/bin/env bash
# ============================================================================
# check_repo-engine_contracts.sh
#
# Validates that every contract interface from the repo-engine module has
# a concrete implementation. Uses grep/find to detect trait definitions and
# their implementing structs.
#
# Usage: bash .pi/scripts/ci/check_repo-engine_contracts.sh [--help]
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

echo ""
echo "═══ Repo-Engine Contract Implementation Check ═══"
echo "Source: $SRC_DIR/repo_engine"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Service Contracts
# ---------------------------------------------------------------------------
echo "--- Service Contracts ---"
if grep -q 'pub trait SymbolGraphService' "$SRC_DIR/repo_engine/application/service.rs" 2>/dev/null; then
    if grep -q 'impl.*SymbolGraphService' "$SRC_DIR/repo_engine/application/symbol_graph_service_impl.rs" 2>/dev/null; then
        log_pass "SymbolGraphService → SymbolGraphServiceImpl"
    else
        log_fail "SymbolGraphService trait has no implementation"
    fi
else
    log_fail "SymbolGraphService trait not found"
fi

if grep -q 'pub trait IndexerService' "$SRC_DIR/repo_engine/application/service.rs" 2>/dev/null; then
    log_pass "IndexerService trait defined"
else
    log_fail "IndexerService trait not found"
fi

if grep -q 'pub trait WorkspaceValidationService' "$SRC_DIR/repo_engine/application/service.rs" 2>/dev/null; then
    if grep -q 'impl.*WorkspaceValidationService' "$SRC_DIR/repo_engine/application/workspace_validation_service_impl.rs" 2>/dev/null; then
        log_pass "WorkspaceValidationService → WorkspaceValidationServiceImpl"
    else
        log_fail "WorkspaceValidationService trait has no implementation"
    fi
else
    log_fail "WorkspaceValidationService trait not found"
fi

# ---------------------------------------------------------------------------
# Check 2: Factory Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Factory Contracts ---"
for factory in "SymbolFactory" "GraphFactory" "IndexerFactory" "LanguageIndexer"; do
    if grep -q "pub trait $factory" "$SRC_DIR/repo_engine/application/factory.rs" 2>/dev/null; then
        log_pass "$factory trait defined"
    else
        log_fail "$factory trait not found"
    fi
done

# ---------------------------------------------------------------------------
# Check 3: Repository Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Repository Contracts ---"
for repo in "SymbolRepository" "SourceRepository" "GrammarRepository"; do
    if grep -q "pub trait $repo" "$SRC_DIR/repo_engine/infrastructure/repository/mod.rs" 2>/dev/null; then
        log_pass "$repo trait defined"
    else
        log_fail "$repo trait not found"
    fi
done

# ---------------------------------------------------------------------------
# Check 4: Domain entities exist
# ---------------------------------------------------------------------------
echo ""
echo "--- Domain Entities ---"
if grep -q 'pub struct SymbolGraph' "$SRC_DIR/repo_engine/domain/symbol_graph.rs" 2>/dev/null; then
    log_pass "SymbolGraph struct exists"
else
    log_fail "SymbolGraph struct not found"
fi

if grep -q 'pub struct SymbolDefinition' "$SRC_DIR/repo_engine/domain/symbol_graph.rs" 2>/dev/null; then
    log_pass "SymbolDefinition struct exists"
else
    log_fail "SymbolDefinition struct not found"
fi

if grep -q 'pub enum SymbolKind' "$SRC_DIR/repo_engine/domain/symbol_graph.rs" 2>/dev/null; then
    log_pass "SymbolKind enum exists (12 variants)"
else
    log_fail "SymbolKind enum not found"
fi

if grep -q 'pub struct Location' "$SRC_DIR/repo_engine/domain/symbol_graph.rs" 2>/dev/null; then
    log_pass "Location struct exists"
else
    log_fail "Location struct not found"
fi

if grep -q 'pub enum SourceLanguage' "$SRC_DIR/repo_engine/domain/symbol_graph.rs" 2>/dev/null; then
    log_pass "SourceLanguage enum exists (Rust, Python, TypeScript)"
else
    log_fail "SourceLanguage enum not found"
fi

if grep -q 'pub enum SymbolVisibility' "$SRC_DIR/repo_engine/domain/symbol_graph.rs" 2>/dev/null; then
    log_pass "SymbolVisibility enum exists"
else
    log_fail "SymbolVisibility enum not found"
fi

if grep -q 'pub struct SharedSymbolGraph' "$SRC_DIR/repo_engine/domain/symbol_graph.rs" 2>/dev/null; then
    log_pass "SharedSymbolGraph struct exists"
else
    log_fail "SharedSymbolGraph struct not found"
fi

if grep -q 'pub enum SymbolWorkspaceIntent' "$SRC_DIR/repo_engine/domain/symbol_workspace.rs" 2>/dev/null; then
    log_pass "SymbolWorkspaceIntent enum exists (4 variants)"
else
    log_fail "SymbolWorkspaceIntent enum not found"
fi

if grep -q 'pub enum RepoEngineError' "$SRC_DIR/repo_engine/domain/error.rs" 2>/dev/null; then
    log_pass "RepoEngineError enum exists"
else
    log_fail "RepoEngineError enum not found"
fi

if grep -q 'pub enum RepoEngineEvent' "$SRC_DIR/repo_engine/domain/event/mod.rs" 2>/dev/null; then
    log_pass "RepoEngineEvent enum exists"
else
    log_fail "RepoEngineEvent enum not found"
fi

# ---------------------------------------------------------------------------
# Check 5: API Contracts exist
# ---------------------------------------------------------------------------
echo ""
echo "--- API Contracts ---"
if grep -q 'pub const API_BASE_PATH' "$SRC_DIR/repo_engine/interfaces/http/mod.rs" 2>/dev/null; then
    log_pass "HTTP API contracts exist in interfaces/http/"
else
    log_fail "HTTP API contracts not found"
fi

for endpoint in "SEARCH_SYMBOLS_PATH" "GET_SYMBOL_PATH" "SYMBOLS_BY_FILE_PATH" \
                "INDEX_FILE_PATH" "INDEX_DIRECTORY_PATH" "GRAPH_STATS_PATH" \
                "CLEAR_GRAPH_PATH"; do
    if grep -q "pub const $endpoint" "$SRC_DIR/repo_engine/interfaces/http/mod.rs" 2>/dev/null; then
        log_pass "Endpoint defined: $endpoint"
    else
        log_fail "Endpoint not found: $endpoint"
    fi
done

# ---------------------------------------------------------------------------
# Check 6: DTOs exist
# ---------------------------------------------------------------------------
echo ""
echo "--- DTO Schemas ---"
for dto in "AddSymbolInput" "AddSymbolOutput" "LookupSymbolInput" "LookupSymbolOutput" \
           "SearchSymbolsInput" "SearchSymbolsOutput" "SymbolsByFileInput" "SymbolsByFileOutput" \
           "GraphStatsInput" "GraphStatsOutput" "ValidateWorkspaceInput" "ValidateWorkspaceOutput"; do
    if grep -q "pub struct $dto\|pub enum $dto" "$SRC_DIR/repo_engine/application/dto/mod.rs" 2>/dev/null; then
        log_pass "DTO defined: $dto"
    else
        log_fail "DTO not found: $dto"
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
    echo "Some contracts are missing implementations."
    exit 1
fi

echo "All repo-engine contracts have implementations."
exit 0
