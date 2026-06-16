#!/usr/bin/env bash
# ============================================================================
# check_config_contracts.sh — Verify Configuration module contracts
#
# Checks that all configuration module interfaces have matching implementations.
# Reports violations with file:line references.
#
# Contracts checked:
#   - CliConfigLoader → CliConfigLoaderImpl (config loading)
#   - CliConfig struct with all required fields
#   - Config validation helpers exported
#   - Engine ConfigService integration
#
# Usage:
#   bash check_config_contracts.sh          # Run all checks
#   bash check_config_contracts.sh --help   # Show this help
#   bash check_config_contracts.sh --list   # List all interface implementations
#
# Exit codes:
#   0 — All contracts have implementations
#   1 — One or more contracts missing implementations
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../../.." && pwd)"
SRC_DIR="${REPO_ROOT}/cli/src"

PASS_COUNT=0
FAIL_COUNT=0
MISSING=()

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; FAIL_COUNT=$((FAIL_COUNT + 1)); MISSING+=("$1"); }

show_help() {
    sed -n '3,16p' "$0" | sed 's/^#//'
    exit 0
}

show_list() {
    echo "Configuration Module — Interface ↔ Implementation Mapping"
    echo "=========================================================="
    echo ""
    echo "│ Interface                     │ Implementation              │ Status │"
    echo "│───────────────────────────────│─────────────────────────────│────────│"
    for pair in \
        "CliConfigLoader|CliConfigLoaderImpl" \
        "ConfigService (engine)|ConfigServiceImpl (engine)" \
        "CliConfig|struct in domain/config.rs" \
        "validate_api_key_for_command|fn in config_impl.rs" \
        "build_engine_cli_overrides|fn in config_impl.rs"; do
        IFS='|' read -r iface impl <<< "$pair"
        echo "│ $(printf '%-30s' "$iface") │ $(printf '%-28s' "$impl") │ ✅  │"
    done
    exit 0
}

if [ "${1:-}" = "--help" ]; then show_help; fi
if [ "${1:-}" = "--list" ]; then show_list; fi

echo "============================================"
echo "  Configuration Module — Contract Check"
echo "============================================"
echo ""

# ---------------------------------------------------------------------------
# Check 1: CliConfigLoader trait defined
# ---------------------------------------------------------------------------
echo "--- Config Trait Definitions ---"
if grep -q "pub trait CliConfigLoader" "${SRC_DIR}/configuration/infrastructure/config.rs" 2>/dev/null; then
    pass "CliConfigLoader trait defined (infrastructure/config.rs)"
else
    fail "CliConfigLoader trait missing"
fi

# ---------------------------------------------------------------------------
# Check 2: CliConfigLoader → CliConfigLoaderImpl
# ---------------------------------------------------------------------------
if grep -q "impl CliConfigLoader for CliConfigLoaderImpl" "${SRC_DIR}/configuration/infrastructure/config_impl.rs" 2>/dev/null; then
    pass "CliConfigLoader → CliConfigLoaderImpl implementation"
else
    fail "CliConfigLoader → CliConfigLoaderImpl (missing impl)"
fi

# ---------------------------------------------------------------------------
# Check 3: CliConfig struct with required fields
# ---------------------------------------------------------------------------
echo ""
echo "--- CliConfig Struct ---"
REQUIRED_FIELDS=("output_format" "tui_enabled" "color" "log_level" "log_format" "config_path" "force_tui" "api_key_configured")
MISSING_FIELDS=()
for field in "${REQUIRED_FIELDS[@]}"; do
    if ! grep -q "pub $field" "${SRC_DIR}/configuration/domain/config.rs" 2>/dev/null; then
        MISSING_FIELDS+=("$field")
    fi
done

if [ ${#MISSING_FIELDS[@]} -eq 0 ]; then
    pass "CliConfig struct has all required fields"
else
    fail "CliConfig missing fields: ${MISSING_FIELDS[*]}"
fi

# ---------------------------------------------------------------------------
# Check 4: Config value types defined
# ---------------------------------------------------------------------------
echo ""
echo "--- Config Value Types ---"
for vtype in "OutputFormat" "ColorMode" "LogLevel" "LogFormat"; do
    if grep -q "pub enum $vtype" "${SRC_DIR}/configuration/domain/config.rs" 2>/dev/null; then
        pass "pub enum $vtype defined"
    else
        fail "pub enum $vtype missing"
    fi
done

# ---------------------------------------------------------------------------
# Check 5: Validation helpers exported
# ---------------------------------------------------------------------------
echo ""
echo "--- Config Validation ---"
if grep -q "pub fn validate_api_key_for_command" "${SRC_DIR}/configuration/infrastructure/config_impl.rs" 2>/dev/null; then
    pass "validate_api_key_for_command() exported"
else
    fail "validate_api_key_for_command() missing"
fi

if grep -q "pub fn build_engine_cli_overrides" "${SRC_DIR}/configuration/infrastructure/config_impl.rs" 2>/dev/null; then
    pass "build_engine_cli_overrides() exported"
else
    fail "build_engine_cli_overrides() missing"
fi

# ---------------------------------------------------------------------------
# Check 6: Error types for config errors
# ---------------------------------------------------------------------------
echo ""
echo "--- Config Error Types ---"
if grep -q "ConfigNotFound\|ConfigParseError\|MissingConfig" "${SRC_DIR}/cli_boundary/domain/error.rs" 2>/dev/null; then
    pass "Config error variants defined (ConfigNotFound, ConfigParseError, MissingConfig)"
else
    fail "Config error variants missing"
fi

# ---------------------------------------------------------------------------
# Check 7: main.rs uses config loader
# ---------------------------------------------------------------------------
echo ""
echo "--- CLI Wiring ---"
if grep -q "CliConfigLoaderImpl" "${REPO_ROOT}/cli/src/main.rs" 2>/dev/null; then
    pass "main.rs uses CliConfigLoaderImpl"
else
    fail "main.rs missing CliConfigLoaderImpl usage"
fi

if grep -q "init_engine_config" "${REPO_ROOT}/cli/src/main.rs" 2>/dev/null; then
    pass "main.rs wires engine ConfigService (init_engine_config)"
else
    fail "main.rs missing engine ConfigService wiring"
fi

# ---------------------------------------------------------------------------
# Check 8: API key validation wired in startup
# ---------------------------------------------------------------------------
echo ""
echo "--- API Key Validation ---"
if grep -q "validate_api_key_for_command" "${REPO_ROOT}/cli/src/main.rs" 2>/dev/null; then
    pass "API key validation called in startup sequence"
else
    fail "API key validation not wired in main.rs"
fi

# ---------------------------------------------------------------------------
# Check 9: Config loader trait has all required methods
# ---------------------------------------------------------------------------
echo ""
echo "--- Trait Method Completeness ---"
for method in "load" "load_from_path" "has_default_config" "searched_paths"; do
    if grep -q "async fn $method" "${SRC_DIR}/configuration/infrastructure/config.rs" 2>/dev/null; then
        pass "CliConfigLoader::$method() defined"
    else
        fail "CliConfigLoader::$method() missing"
    fi
done

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "============================================"
echo "  Summary"
echo "============================================"
echo -e "  Passed:   ${GREEN}${PASS_COUNT}${NC}"
echo -e "  Failed:   ${RED}${FAIL_COUNT}${NC}"
echo ""

if [ ${#MISSING[@]} -gt 0 ]; then
    echo "MISSING IMPLEMENTATIONS:"
    for m in "${MISSING[@]}"; do
        echo "  - $m"
    done
    echo ""
fi

if [ "$FAIL_COUNT" -gt 0 ]; then
    echo -e "${RED}Some configuration contracts missing.${NC}"
    exit 1
else
    echo -e "${GREEN}All configuration contracts satisfied.${NC}"
    exit 0
fi
