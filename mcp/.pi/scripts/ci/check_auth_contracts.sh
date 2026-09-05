#!/usr/bin/env bash
# ============================================================================
# check_auth_contracts.sh
#
# Validates that every interface frozen in the auth contract freeze (#820) has
# a concrete implementation, that each frozen trait method is implemented, and
# that the module is registered. Exits 0 if all contracts are satisfied,
# 1 otherwise.
#
# Usage: bash check_auth_contracts.sh [--help|--verbose]
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# SCRIPT_DIR = <repo>/mcp/.pi/scripts/ci, so ../../../ = <repo>/mcp
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../../../" && pwd)"

VERBOSE=false
if [[ "${1:-}" == "--verbose" ]]; then VERBOSE=true; fi
if [[ "${1:-}" == "--help" ]]; then
    sed -n '/^# =*/,/^\$/p' "${BASH_SOURCE[0]}" | grep -v '^# =' | sed 's/^# //'
    exit 0
fi

PASS=0
FAIL=0

pass() { echo -e "  \e[32m✅ PASS\e[0m $1"; PASS=$((PASS + 1)); }
fail() { echo -e "  \e[31m❌ FAIL\e[0m $1"; FAIL=$((FAIL + 1)); }

MODULE_SRC="${PROJECT_ROOT}/src/auth"

echo "============================================"
echo "  Auth Contract Implementation Check"
echo "============================================"
echo ""

# ---------------------------------------------------------------------------
# Module registration
# ---------------------------------------------------------------------------
echo "--- Module Registration ---"
if grep -q '^pub mod auth;' "${PROJECT_ROOT}/src/lib.rs"; then
    pass "auth module registered in src/lib.rs"
else
    fail "auth module missing from src/lib.rs (pub mod auth;)"
fi

# ---------------------------------------------------------------------------
# Helper: verify a trait has an implementation and implements each method
# ---------------------------------------------------------------------------
# usage: check_impl <TraitName> <ImplName> <impl-file-glob> <method-name...>
check_impl() {
    local trait_name="$1"
    local impl_name="$2"
    local search_path="$3"
    shift 3

    echo "--- ${trait_name} → ${impl_name} ---"

    local impl_line
    impl_line=$(grep -rn "impl ${trait_name} for ${impl_name}" "${MODULE_SRC}/" 2>/dev/null || true)
    if [ -n "$impl_line" ]; then
        pass "${trait_name} trait → ${impl_name} ($(echo "$impl_line" | head -1 | cut -d: -f1,2))"
    else
        fail "${trait_name}: no 'impl ${trait_name} for ${impl_name}' found under src/auth/"
        return
    fi

    local impl_file
    impl_file=$(echo "$impl_line" | head -1 | cut -d: -f1)
    if [ ! -f "$impl_file" ]; then
        # Search-based resolution failed; fall back to the provided glob.
        impl_file=$(ls ${search_path} 2>/dev/null | head -1 || true)
    fi

    local missing=0
    for method in "$@"; do
        if grep -qE "(async )?fn ${method}\(" "$impl_file"; then
            if [[ "$VERBOSE" == "true" ]]; then
                pass "  ${impl_name}::${method} implemented"
            fi
        else
            fail "  ${trait_name}::${method} NOT implemented in $(basename "$impl_file")"
            missing=1
        fi
    done
    if [ "$missing" -eq 0 ]; then
        pass "${trait_name}: all $(($#)) methods implemented in $(basename "$impl_file")"
    fi
}

# ---------------------------------------------------------------------------
# Application contracts
# ---------------------------------------------------------------------------
check_impl "AuthService" "AuthServiceImpl" \
    "${MODULE_SRC}/application/service_impl.rs" \
    login poll status refresh logout attest

check_impl "AuthServiceFactory" "AuthServiceFactoryImpl" \
    "${MODULE_SRC}/application/factory.rs" \
    create

# ---------------------------------------------------------------------------
# Infrastructure contracts
# ---------------------------------------------------------------------------
check_impl "IdpClient" "HttpIdpClient" \
    "${MODULE_SRC}/infrastructure/idp_client_impl.rs" \
    discover device_authorization poll_token refresh_token revoke_token

check_impl "KeychainStore" "KeychainStoreImpl" \
    "${MODULE_SRC}/infrastructure/keychain_store_impl.rs" \
    store_refresh_token get_refresh_token delete_refresh_token uses_plaintext_fallback

check_impl "TokenProvider" "InMemoryTokenProvider" \
    "${MODULE_SRC}/infrastructure/token_provider_impl.rs" \
    current_access_token set_access_token access_token_expires_at clear

# ---------------------------------------------------------------------------
# Interfaces contracts
# ---------------------------------------------------------------------------
check_impl "AuthToolHandler" "AuthToolHandlerImpl" \
    "${MODULE_SRC}/interfaces/mcp/handler_impl.rs" \
    handle_auth_login handle_auth_status handle_auth_logout

check_impl "SseAuthGate" "SseAuthGateImpl" \
    "${MODULE_SRC}/interfaces/sse_auth_impl.rs" \
    mode authorize

# ---------------------------------------------------------------------------
# Frozen MCP tool surface
# ---------------------------------------------------------------------------
echo "--- MCP Tool Surface ---"
for tool in rigorix_auth_login rigorix_auth_status rigorix_auth_logout; do
    if grep -rq "${tool}" "${MODULE_SRC}/interfaces/mcp/mod.rs"; then
        pass "tool name ${tool} frozen in interfaces/mcp"
    else
        fail "tool name ${tool} missing from interfaces/mcp/mod.rs"
    fi
done

# ---------------------------------------------------------------------------
# No interface without an implementation (each frozen trait covered above)
# ---------------------------------------------------------------------------
echo "--- Interface/Implementation Coverage ---"
TRAITS=$(grep -rhoE '^\s*pub trait [A-Za-z0-9_]+' "${MODULE_SRC}/" 2>/dev/null | sed 's/.*trait //' | sort -u || true)
UNIMPLEMENTED=0
for trait_name in $TRAITS; do
    if grep -rnq "impl ${trait_name} for" "${MODULE_SRC}/" 2>/dev/null; then
        continue
    fi
    # Marker traits / type-alias bundles are not impl contracts.
    case "$trait_name" in
        Secret*|Shared*) continue ;;
    esac
    fail "trait ${trait_name} has no 'impl ${trait_name} for' anywhere in src/auth/"
    UNIMPLEMENTED=1
done
if [ "$UNIMPLEMENTED" -eq 0 ]; then
    pass "every auth trait has a concrete implementation"
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "============================================"
echo "  Summary"
echo "============================================"
echo -e "  Passed:   ${PASS}"
echo -e "  Failed:   ${FAIL}"
echo ""

if [ "${FAIL}" -gt 0 ]; then
    echo "FAILURES: ${FAIL} contract violation(s)"
    exit 1
fi

echo -e "\e[32mAll auth contracts implemented. Freeze (#820) satisfied.\e[0m"
exit 0
