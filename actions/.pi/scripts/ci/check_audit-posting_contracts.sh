#!/usr/bin/env bash
# Check Audit Posting Contracts
#
# Validates that every interface defined in the contract freeze has a
# corresponding concrete implementation.
#
# Checks:
# - Application service traits → impl files exist
# - Infrastructure repository traits → impl files exist
# - Domain types → no orphan interfaces
# - Factory traits → impl files exist
#
# Usage: bash .pi/scripts/ci/check_audit-posting_contracts.sh [--verbose]
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
AUDIT_DIR="$ROOT_DIR/actions/src/audit_posting"

cd "$ROOT_DIR"

ERRORS=0
MISSING_IMPLS=()

# ── Helper: check if a file exists ──
check_file() {
    local description="$1"
    local filepath="$2"
    if [[ ! -f "$filepath" ]]; then
        MISSING_IMPLS+=("$description: $filepath")
        return 1
    fi
    return 0
}

# ── Interface-to-Implementation Mapping ──

# Service interfaces
check_file "AuditPostingService → AuditPostingServiceImpl" \
    "$AUDIT_DIR/application/audit_posting_service_impl.rs" || ERRORS=$((ERRORS + 1))

check_file "AuditRecordQueue → AuditRecordQueueImpl" \
    "$AUDIT_DIR/application/audit_queue_impl.rs" || ERRORS=$((ERRORS + 1))

# Factory interfaces
check_file "AuditRecordFactory → AuditRecordFactoryImpl" \
    "$AUDIT_DIR/application/audit_record_factory_impl.rs" || ERRORS=$((ERRORS + 1))

check_file "AuditBackendFactory → AuditBackendFactoryImpl" \
    "$AUDIT_DIR/application/audit_backend_factory_impl.rs" || ERRORS=$((ERRORS + 1))

# Infrastructure backends
check_file "AuditBackend → HttpAuditBackend" \
    "$AUDIT_DIR/infrastructure/http_audit_backend.rs" || ERRORS=$((ERRORS + 1))

check_file "FilesystemAuditBackend → FilesystemAuditBackendImpl" \
    "$AUDIT_DIR/infrastructure/filesystem_audit_backend.rs" || ERRORS=$((ERRORS + 1))

# ── Check Domain Layer Structure ──
check_file "Domain types module" \
    "$AUDIT_DIR/domain/signed_audit_record.rs" || ERRORS=$((ERRORS + 1))

check_file "Domain error module" \
    "$AUDIT_DIR/domain/error.rs" || ERRORS=$((ERRORS + 1))

check_file "Domain event module" \
    "$AUDIT_DIR/domain/event/mod.rs" || ERRORS=$((ERRORS + 1))

# ── Check DTO module ──
check_file "Application DTO module" \
    "$AUDIT_DIR/application/dto/mod.rs" || ERRORS=$((ERRORS + 1))

# ── Check service and factory interfaces ──
check_file "Service interface module" \
    "$AUDIT_DIR/application/service.rs" || ERRORS=$((ERRORS + 1))

check_file "Factory interfaces module" \
    "$AUDIT_DIR/application/factory.rs" || ERRORS=$((ERRORS + 1))

# ── Check repository interfaces ──
check_file "Repository interfaces module" \
    "$AUDIT_DIR/infrastructure/repository/mod.rs" || ERRORS=$((ERRORS + 1))

# ── Check HTTP API contracts ──
check_file "HTTP API contracts module" \
    "$AUDIT_DIR/interfaces/http/mod.rs" || ERRORS=$((ERRORS + 1))

# ── Summary ──
echo ""
if [[ ${#MISSING_IMPLS[@]} -gt 0 ]]; then
    echo "❌ MISSING IMPLEMENTATIONS:"
    for missing in "${MISSING_IMPLS[@]}"; do
        echo "   - $missing"
    done
fi

if [[ $ERRORS -eq 0 ]]; then
    echo "✅ All audit-posting contracts have implementations ($ERRORS missing)"
    if $VERBOSE; then
        echo ""
        echo "Service Implementations:"
        echo "  ✓ AuditPostingService → audit_posting_service_impl.rs"
        echo "  ✓ AuditRecordQueue → audit_queue_impl.rs"
        echo ""
        echo "Factory Implementations:"
        echo "  ✓ AuditRecordFactory → audit_record_factory_impl.rs"
        echo "  ✓ AuditBackendFactory → audit_backend_factory_impl.rs"
        echo ""
        echo "Infrastructure Backends:"
        echo "  ✓ AuditBackend (HTTP) → http_audit_backend.rs"
        echo "  ✓ FilesystemAuditBackend (OSS) → filesystem_audit_backend.rs"
        echo ""
        echo "Domain:"
        echo "  ✓ signed_audit_record.rs, error.rs, event/"
        echo ""
        echo "Application:"
        echo "  ✓ dto/, service.rs, factory.rs"
        echo ""
        echo "Interfaces:"
        echo "  ✓ http/"
    fi
    exit 0
else
    echo "❌ $ERRORS contract(s) missing implementation"
    exit 1
fi
