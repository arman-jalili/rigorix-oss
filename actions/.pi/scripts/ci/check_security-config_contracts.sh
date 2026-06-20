#!/usr/bin/env bash
# Check Security Config Contracts
#
# Validates that every interface defined in the contract freeze has a
# corresponding concrete implementation.
#
# Usage: bash .pi/scripts/ci/check_security-config_contracts.sh [--verbose]
#
# Exit codes:
#   0 — All contracts have implementations
#   1 — One or more contracts are missing implementations

set -euo pipefail

VERBOSE=false
if [[ "${1:-}" == "--verbose" ]]; then
    VERBOSE=true
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../../../.." && pwd)"
SECURITY_DIR="$ROOT_DIR/actions/src/security_config"

cd "$ROOT_DIR"

ERRORS=0

# ── Helper ──
check_file() {
    local description="$1"
    local filepath="$2"
    if [[ ! -f "$filepath" ]]; then
        echo "   ❌ MISSING: $description — $filepath"
        return 1
    fi
    return 0
}

# ── Service Implementations ──
check_file "ForkDetectionService → ForkDetectorImpl" \
    "$SECURITY_DIR/application/fork_detector_impl.rs" || ERRORS=$((ERRORS + 1))

check_file "SecretMaskingService → SecretMaskerImpl" \
    "$SECURITY_DIR/application/secret_masker_impl.rs" || ERRORS=$((ERRORS + 1))

check_file "TokenValidationService → TokenValidatorImpl" \
    "$SECURITY_DIR/application/token_validator_impl.rs" || ERRORS=$((ERRORS + 1))

check_file "UrlAllowlistService → UrlAllowlistImpl" \
    "$SECURITY_DIR/application/url_allowlist_impl.rs" || ERRORS=$((ERRORS + 1))

check_file "HmacSigningService → HmacSignerImpl" \
    "$SECURITY_DIR/application/hmac_signer_impl.rs" || ERRORS=$((ERRORS + 1))

# ── Repository Implementations ──
check_file "ForkRepository → EnvForkRepository" \
    "$SECURITY_DIR/infrastructure/env_fork_repository_impl.rs" || ERRORS=$((ERRORS + 1))

# ── Domain Layer ──
check_file "Domain types module" \
    "$SECURITY_DIR/domain/types.rs" || ERRORS=$((ERRORS + 1))

check_file "Domain error module" \
    "$SECURITY_DIR/domain/error.rs" || ERRORS=$((ERRORS + 1))

check_file "Domain event module" \
    "$SECURITY_DIR/domain/event/mod.rs" || ERRORS=$((ERRORS + 1))

# ── Application Layer ──
check_file "Application DTO module" \
    "$SECURITY_DIR/application/dto/mod.rs" || ERRORS=$((ERRORS + 1))

check_file "Factory interfaces module" \
    "$SECURITY_DIR/application/factory.rs" || ERRORS=$((ERRORS + 1))

# ── API Contracts ──
check_file "HTTP API contracts module" \
    "$SECURITY_DIR/interfaces/http/mod.rs" || ERRORS=$((ERRORS + 1))

# ── Summary ──
if $VERBOSE && [[ $ERRORS -eq 0 ]]; then
    echo ""
    echo "Service Implementations:"
    echo "  ✓ ForkDetectionService → fork_detector_impl.rs"
    echo "  ✓ SecretMaskingService → secret_masker_impl.rs"
    echo "  ✓ TokenValidationService → token_validator_impl.rs"
    echo "  ✓ UrlAllowlistService → url_allowlist_impl.rs"
    echo "  ✓ HmacSigningService → hmac_signer_impl.rs"
    echo ""
    echo "Repository Implementations:"
    echo "  ✓ ForkRepository → env_fork_repository_impl.rs"
fi

if [[ $ERRORS -eq 0 ]]; then
    echo "✅ All security-config contracts have implementations"
    exit 0
else
    echo "❌ $ERRORS contract(s) missing implementation"
    exit 1
fi
