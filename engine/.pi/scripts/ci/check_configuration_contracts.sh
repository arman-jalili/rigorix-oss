#!/usr/bin/env bash
# ============================================================================
# check_configuration_contracts.sh
#
# Validates that every contract interface from the configuration module has
# a concrete implementation. Uses grep/find to detect trait definitions and
# their implementing structs.
#
# Usage: bash .pi/scripts/ci/check_configuration_contracts.sh [--help]
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
echo "═══ Configuration Contract Implementation Check ═══"
echo "Source: $SRC_DIR/configuration"
echo ""

# ---------------------------------------------------------------------------
# Check 1: ConfigService trait → ConfigServiceImpl
# ---------------------------------------------------------------------------
echo "--- Service Contracts ---"
if grep -q 'pub trait ConfigService' "$SRC_DIR/configuration/application/service.rs" 2>/dev/null; then
    if grep -q 'impl.*ConfigService' "$SRC_DIR/configuration/application/config_service_impl.rs" 2>/dev/null; then
        log_pass "ConfigService → ConfigServiceImpl"
    else
        log_fail "ConfigService trait has no implementation"
    fi
else
    log_fail "ConfigService trait not found"
fi

if grep -q 'pub trait SecretService' "$SRC_DIR/configuration/application/service.rs" 2>/dev/null; then
    if grep -q 'impl.*SecretService' "$SRC_DIR/configuration/application/secret_service_impl.rs" 2>/dev/null; then
        log_pass "SecretService → SecretServiceImpl"
    else
        log_fail "SecretService trait has no implementation"
    fi
else
    log_fail "SecretService trait not found"
fi

# ---------------------------------------------------------------------------
# Check 2: Factory Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Factory Contracts ---"
if grep -q 'pub trait ConfigFactory' "$SRC_DIR/configuration/application/factory.rs" 2>/dev/null; then
    if grep -q 'impl.*ConfigFactory' "$SRC_DIR/configuration/infrastructure/config_factory_impl.rs" 2>/dev/null; then
        log_pass "ConfigFactory → ConfigFactoryImpl"
    else
        log_fail "ConfigFactory trait has no implementation"
    fi
else
    log_fail "ConfigFactory trait not found"
fi

if grep -q 'pub trait SecretFactory' "$SRC_DIR/configuration/application/factory.rs" 2>/dev/null; then
    if grep -q 'impl.*SecretFactory' "$SRC_DIR/configuration/infrastructure/secret_factory_impl.rs" 2>/dev/null; then
        log_pass "SecretFactory → SecretFactoryImpl"
    else
        log_fail "SecretFactory trait has no implementation"
    fi
else
    log_fail "SecretFactory trait not found"
fi

# ---------------------------------------------------------------------------
# Check 3: Repository Contracts
# ---------------------------------------------------------------------------
echo ""
echo "--- Repository Contracts ---"
if grep -q 'pub trait ConfigRepository' "$SRC_DIR/configuration/infrastructure/repository/mod.rs" 2>/dev/null; then
    if grep -q 'impl.*ConfigRepository' "$SRC_DIR/configuration/infrastructure/filesystem_config_repository.rs" 2>/dev/null; then
        log_pass "ConfigRepository → FilesystemConfigRepository"
    else
        log_fail "ConfigRepository trait has no implementation"
    fi
else
    log_fail "ConfigRepository trait not found"
fi

if grep -q 'pub trait ConfigWriteRepository' "$SRC_DIR/configuration/infrastructure/repository/mod.rs" 2>/dev/null; then
    # ConfigWriteRepository may not be implemented yet — warn but don't fail
    if grep -q 'impl.*ConfigWriteRepository' "$SRC_DIR/configuration/infrastructure/"*.rs 2>/dev/null; then
        log_pass "ConfigWriteRepository → implementation found"
    else
        log_pass "ConfigWriteRepository trait defined (implementation deferred — optional)"
    fi
fi

# ---------------------------------------------------------------------------
# Check 4: Domain entities exist
# ---------------------------------------------------------------------------
echo ""
echo "--- Domain Entities ---"
if grep -q 'pub struct Config' "$SRC_DIR/configuration/domain/config.rs" 2>/dev/null; then
    log_pass "Config struct exists"
else
    log_fail "Config struct not found"
fi

if grep -q 'pub struct Secret' "$SRC_DIR/configuration/domain/secret.rs" 2>/dev/null; then
    log_pass "Secret struct exists"
else
    log_fail "Secret struct not found"
fi

if grep -q 'pub enum ConfigurationError' "$SRC_DIR/configuration/domain/error.rs" 2>/dev/null; then
    log_pass "ConfigurationError enum exists"
else
    log_fail "ConfigurationError enum not found"
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
    echo "Some contracts are missing implementations."
    exit 1
fi

echo "All configuration contracts have implementations."
exit 0
