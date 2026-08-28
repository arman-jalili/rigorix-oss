#!/usr/bin/env bash
# ============================================================================
# check_identity_contracts.sh
#
# Validates that every contract interface from the identity module has a
# concrete implementation. Uses grep/find to detect trait definitions and
# their implementing types.
#
# Usage: bash .pi/scripts/ci/check_identity_contracts.sh [--help]
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
echo "═══ Identity Contract Implementation Check ═══"
echo "Source: $SRC_DIR/identity"
echo ""

# ---------------------------------------------------------------------------
# Check 1: Domain value types (IdentityClaim, IdentitySource, IdentityRef)
# ---------------------------------------------------------------------------
echo "--- Domain Value Types ---"

if grep -q 'pub struct IdentityClaim' "$SRC_DIR/identity/domain/claim.rs" 2>/dev/null; then
    log_pass "IdentityClaim value type defined (claim.rs)"
else
    log_fail "IdentityClaim not found in domain/claim.rs"
fi

if grep -q 'pub enum IdentitySource' "$SRC_DIR/identity/domain/claim.rs" 2>/dev/null; then
    log_pass "IdentitySource enum defined (claim.rs)"
else
    log_fail "IdentitySource not found in domain/claim.rs"
fi

if grep -q 'pub struct IdentityRef' "$SRC_DIR/identity/domain/claim.rs" 2>/dev/null; then
    log_pass "IdentityRef (redacted envelope ref) defined (claim.rs)"
else
    log_fail "IdentityRef not found in domain/claim.rs"
fi

# Claim behavior must be implemented (no todo!() stubs left in claim.rs)
if grep -q 'todo!' "$SRC_DIR/identity/domain/claim.rs" 2>/dev/null; then
    log_fail "IdentityClaim still contains todo!() stubs — is_valid/redacted_summary must be implemented"
else
    log_pass "IdentityClaim behavior implemented (no todo!() stubs)"
fi

# ---------------------------------------------------------------------------
# Check 2: IdentityError
# ---------------------------------------------------------------------------
echo ""
echo "--- IdentityError ---"

if grep -q 'pub enum IdentityError' "$SRC_DIR/identity/domain/error.rs" 2>/dev/null; then
    log_pass "IdentityError enum defined (error.rs)"
else
    log_fail "IdentityError not found in domain/error.rs"
fi

if grep -q 'is_fatal_for_attestation' "$SRC_DIR/identity/domain/error.rs" 2>/dev/null; then
    log_pass "IdentityError recovery semantics implemented (is_fatal_for_attestation)"
else
    log_fail "IdentityError recovery helpers missing (is_fatal_for_attestation)"
fi

# ---------------------------------------------------------------------------
# Check 3: IdentityAttestationService trait → concrete impl
# ---------------------------------------------------------------------------
echo ""
echo "--- IdentityAttestationService ---"

if grep -q 'pub trait IdentityAttestationService' "$SRC_DIR/identity/application/service.rs" 2>/dev/null; then
    log_pass "IdentityAttestationService trait defined (service.rs)"
else
    log_fail "IdentityAttestationService trait not found in application/service.rs"
fi

if grep -q 'impl IdentityAttestationService for IdentityAttestationServiceImpl' "$SRC_DIR/identity/application/service_impl.rs" 2>/dev/null; then
    log_pass "IdentityAttestationService → IdentityAttestationServiceImpl"
else
    log_fail "IdentityAttestationServiceImpl does not implement the service trait"
fi

if grep -q 'todo!' "$SRC_DIR/identity/application/service_impl.rs" 2>/dev/null; then
    log_fail "IdentityAttestationServiceImpl still contains todo!() stubs"
else
    log_pass "IdentityAttestationServiceImpl fully implemented (no todo!() stubs)"
fi

# ---------------------------------------------------------------------------
# Check 4: TokenVerifier trait → NullVerifier + JwksVerifier
# ---------------------------------------------------------------------------
echo ""
echo "--- TokenVerifier ---"

if grep -q 'pub trait TokenVerifier' "$SRC_DIR/identity/infrastructure/verifier.rs" 2>/dev/null; then
    log_pass "TokenVerifier trait defined (verifier.rs)"
else
    log_fail "TokenVerifier trait not found in infrastructure/verifier.rs"
fi

if grep -q 'impl TokenVerifier for NullVerifier' "$SRC_DIR/identity/infrastructure/verifier.rs" 2>/dev/null; then
    log_pass "TokenVerifier → NullVerifier (offline default)"
else
    log_fail "NullVerifier does not implement TokenVerifier"
fi

if grep -q 'impl TokenVerifier for JwksVerifier' "$SRC_DIR/identity/infrastructure/verifier.rs" 2>/dev/null; then
    log_pass "TokenVerifier → JwksVerifier (JWKS-backed)"
else
    log_fail "JwksVerifier does not implement TokenVerifier"
fi

# ---------------------------------------------------------------------------
# Check 5: IdentityRepository trait → FileSystemIdentityRepository
# ---------------------------------------------------------------------------
echo ""
echo "--- IdentityRepository ---"

if grep -q 'pub trait IdentityRepository' "$SRC_DIR/identity/infrastructure/repository/identity_repository.rs" 2>/dev/null; then
    log_pass "IdentityRepository trait defined (identity_repository.rs)"
else
    log_fail "IdentityRepository trait not found in infrastructure/repository/"
fi

if grep -q 'impl IdentityRepository for FileSystemIdentityRepository' "$SRC_DIR/identity/infrastructure/repository/identity_repository_impl.rs" 2>/dev/null; then
    log_pass "IdentityRepository → FileSystemIdentityRepository"
else
    log_fail "FileSystemIdentityRepository does not implement IdentityRepository"
fi

# ---------------------------------------------------------------------------
# Check 6: DTOs (AttestInput / AttestOutput)
# ---------------------------------------------------------------------------
echo ""
echo "--- DTOs ---"

if grep -q 'pub struct AttestInput' "$SRC_DIR/identity/application/dto/mod.rs" 2>/dev/null; then
    log_pass "AttestInput DTO defined"
else
    log_fail "AttestInput DTO not found"
fi

if grep -q 'pub struct AttestOutput' "$SRC_DIR/identity/application/dto/mod.rs" 2>/dev/null; then
    log_pass "AttestOutput DTO defined"
else
    log_fail "AttestOutput DTO not found"
fi

# ---------------------------------------------------------------------------
# Check 7: CoreOrchestratorError integration
# ---------------------------------------------------------------------------
echo ""
echo "--- Error Integration ---"

if grep -qF 'Identity(#[from] IdentityError)' "$SRC_DIR/error.rs" 2>/dev/null; then
    log_pass "IdentityError wired into CoreOrchestratorError (#[from])"
else
    log_fail "CoreOrchestratorError missing Identity(#[from] IdentityError) variant"
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
    echo "Identity contract check FAILED."
    exit 1
fi

echo "Identity contract check PASSED."
exit 0
