#!/usr/bin/env bash
# ============================================================================
# validate-code-graph-contracts.sh
#
# Verifies that every frozen contract from the code-graph contract freeze
# has a corresponding implementation. Checks domain entities, service traits,
# factory traits, and repository interfaces.
#
# Usage: .pi/scripts/validate-code-graph-contracts.sh
#
# Exit codes:
#   0 — All contracts have implementations
#   1 — One or more contracts missing implementation
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ENGINE_DIR="$SCRIPT_DIR/../../src/code_graph"

PASS=0
FAIL=0
FAILURES=""

check_impl() {
    local name="$1"
    local file="$2"
    if [ -f "$ENGINE_DIR/$file" ]; then
        echo "  ✅ $name ($file)"
        PASS=$((PASS + 1))
    else
        echo "  ❌ $name — missing: $file"
        FAIL=$((FAIL + 1))
        FAILURES="$FAILURES\n    - $name ($file)"
    fi
}

check_contains() {
    local name="$1"
    local file="$2"
    local pattern="$3"
    if [ -f "$ENGINE_DIR/$file" ] && grep -q "$pattern" "$ENGINE_DIR/$file" 2>/dev/null; then
        echo "  ✅ $name ($file matches '$pattern')"
        PASS=$((PASS + 1))
    else
        echo "  ❌ $name — $file missing pattern: $pattern"
        FAIL=$((FAIL + 1))
        FAILURES="$FAILURES\n    - $name ($file: $pattern)"
    fi
}

echo ""
echo "============================================"
echo " Code-Graph Contract Implementation Check"
echo "============================================"
echo ""

echo "--- Domain Entities ---"
check_impl "CodeGraph struct"         "domain/graph.rs"
check_impl "ModuleNode struct"        "domain/graph.rs"
check_impl "ModuleEdge struct"        "domain/graph.rs"
check_impl "NodeKind enum"            "domain/graph.rs"
check_impl "EdgeKind enum"            "domain/graph.rs"
check_impl "GraphMetadata struct"     "domain/graph.rs"
check_impl "CodeGraphError enum"      "domain/error.rs"
check_impl "CodeGraphEvent enum"      "domain/event/mod.rs"

echo ""
echo "--- Service Traits ---"
check_impl "CodeGraphService trait"   "application/service.rs"
check_impl "CodeGraphAnalyzer trait"  "application/service.rs"
check_impl "CodeGraphFormatter trait" "application/service.rs"
check_impl "CodeGraphImporter trait"  "application/service.rs"
check_impl "CodeGraphServiceImpl"     "application/service_impl.rs"
check_impl "CodeGraphAnalyzerImpl"    "application/service_impl.rs"
check_impl "CodeGraphFormatterImpl"   "application/service_impl.rs"
check_impl "CodeGraphImporterImpl"    "application/service_impl.rs"

echo ""
echo "--- Builder ---"
check_impl "CodeGraphBuilder"         "application/builder.rs"

echo ""
echo "--- DTOs ---"
check_impl "DTO module"              "application/dto/mod.rs"
check_impl "ConstructGraphInput"     "application/dto/mod.rs"
check_impl "AddNodeInput/Output"     "application/dto/mod.rs"
check_impl "AddEdgeInput/Output"     "application/dto/mod.rs"
check_impl "SealGraphInput/Output"   "application/dto/mod.rs"
check_impl "OutputFormat enum"       "application/dto/mod.rs"

echo ""
echo "--- Factories ---"
check_impl "CodeGraphServiceFactory"  "application/factory.rs"
check_impl "CodeGraphAnalyzerFactory" "application/factory.rs"
check_impl "CodeGraphFormatterFactory" "application/factory.rs"
check_impl "CodeGraphImporterFactory" "application/factory.rs"

echo ""
echo "--- Persistence ---"
check_impl "CodeGraphRepository trait"    "infrastructure/repository/mod.rs"
check_impl "InMemoryCodeGraphRepository"  "infrastructure/repository/memory_repository.rs"
check_impl "FilesystemCodeGraphRepository" "infrastructure/repository/filesystem_repository.rs"

echo ""
echo "--- HTTP API Contracts ---"
check_impl "HTTP API contracts"      "interfaces/http/mod.rs"
check_impl "ApiErrorResponse"        "interfaces/http/mod.rs"
check_contains "Error codes"         "interfaces/http/mod.rs" "error_codes"
check_contains "Status codes"        "interfaces/http/mod.rs" "status_codes"

echo ""
echo "--- Tests ---"
check_impl "Test suite"              "tests.rs"
check_contains "Domain tests"        "tests.rs" "test_codegraph_new"
check_contains "Service tests"       "tests.rs" "test_service_construct_graph"
check_contains "Formatter tests"     "tests.rs" "test_formatter_mermaid"

echo ""
echo "============================================"
echo " Results: $PASS passed, $FAIL failed"
echo "============================================"

if [ "$FAIL" -gt 0 ]; then
    echo -e "Missing implementations:$FAILURES"
    echo ""
    exit 1
fi

echo ""
exit 0
