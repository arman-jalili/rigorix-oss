#!/usr/bin/env bash
# Architecture Readiness Validator
#
# Validates that the epic slice is architecturally complete before closing.
# Checks: runbook, DR plan, docs update, canonical refs synced, observability.
#
# Usage: bash .pi/scripts/validate-architecture-readiness.sh
#
# Exit codes:
#   0 — All readiness checks passed
#   1 — One or more readiness checks failed

set -euo pipefail

PI_DIR=".pi"
ARCH_DIR="${PI_DIR}/architecture"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

PASS=0
FAIL=0

log_pass() { echo -e "  ${GREEN}✓ PASS${NC} $1"; ((PASS++)); }
log_fail() { echo -e "  ${RED}✗ FAIL${NC} $1 — $2"; ((FAIL++)); }
log_skip() { echo -e "  ${YELLOW}⊘ SKIP${NC} $1 — $2"; ((PASS++)); }

echo "═══ Architecture Readiness Checks ═══"
echo ""

# 1. Runbook readiness
echo "Checking runbook readiness..."
if [[ -f "docs/runbook.md" || -f "docs/RUNBOOK.md" || -f "RUNBOOK.md" ]]; then
    # Check runbook has required sections
    RUNBOOK=$(find . -name "runbook.md" -o -name "RUNBOOK.md" 2>/dev/null | head -1)
    if grep -qiE "(incident|escalation|rollback|recovery|on.call)" "$RUNBOOK" 2>/dev/null; then
        log_pass "Runbook readiness (runbook exists with incident/rollback sections)"
    else
        log_fail "Runbook readiness" "Runbook exists but missing incident/rollback sections"
    fi
else
    log_fail "Architecture readiness" "No runbook.md found. Create docs/runbook.md with incident procedures."
fi

# 2. DR plan
echo "Checking DR plan..."
if [[ -f "docs/dr-plan.md" || -f "docs/disaster-recovery.md" || -f "docs/DR.md" ]]; then
    DR_FILE=$(find . -name "dr-plan.md" -o -name "disaster-recovery.md" -o -name "DR.md" 2>/dev/null | head -1)
    if grep -qiE "(rto|rpo|backup|restore|failover|recovery)" "$DR_FILE" 2>/dev/null; then
        log_pass "DR plan readiness (DR plan exists with RTO/RPO sections)"
    else
        log_fail "DR plan readiness" "DR plan exists but missing RTO/RPO/recovery sections"
    fi
else
    log_fail "Architecture readiness" "No dr-plan.md found. Create docs/dr-plan.md with RTO/RPO targets."
fi

# 3. Docs updated
echo "Checking documentation sync..."
if command -v git &>/dev/null; then
    # Check if any architecture module was recently updated
    recently_updated=false
    if [[ -d "$ARCH_DIR/modules" ]]; then
        for module_file in "${ARCH_DIR}"/modules/*.md; do
            [[ -f "$module_file" ]] || continue
            if git log -1 --since="1 day ago" -- "$module_file" 2>/dev/null | grep -q .; then
                recently_updated=true
                break
            fi
        done
    fi
    if [[ "$recently_updated" == "true" ]]; then
        log_pass "Documentation sync (architecture docs recently updated)"
    else
        log_skip "Documentation sync" "Architecture docs not recently updated (may be fine for small slices)"
    fi
else
    log_pass "Documentation sync (git not available, skipping)"
fi

# 4. Canonical refs synced
echo "Checking canonical references..."
if [[ -f "${PI_DIR}/scripts/validate-canonical.sh" ]]; then
    if bash "${PI_DIR}/scripts/validate-canonical.sh" >/dev/null 2>&1; then
        log_pass "Canonical references (validate-canonical.sh passed)"
    else
        log_fail "Canonical references" "validate-canonical.sh failed — some files lack canonical refs"
    fi
else
    log_skip "Canonical references" "validate-canonical.sh not found"
fi

# 5. Observability readiness
echo "Checking observability readiness..."
HAS_TRACING=false
HAS_METRICS=false
HAS_LOGGING=false

for f in $(find . -name "*.py" -o -name "*.ts" -o -name "*.rs" -o -name "*.go" 2>/dev/null | head -30); do
    grep -qiE "(opentelemetry|jaeger|zipkin|tracing\.)" "$f" 2>/dev/null && HAS_TRACING=true
    grep -qiE "(prometheus|datadog|metrics\.|counter|histogram)" "$f" 2>/dev/null && HAS_METRICS=true
    grep -qiE "(structured.log|json.log|log\.info|log\.error|logger\.)" "$f" 2>/dev/null && HAS_LOGGING=true
done

if [[ "$HAS_TRACING" == "true" ]]; then
    log_pass "Observability: tracing detected"
else
    log_skip "Observability: tracing" "No tracing framework detected"
fi

if [[ "$HAS_METRICS" == "true" ]]; then
    log_pass "Observability: metrics detected"
else
    log_skip "Observability: metrics" "No metrics framework detected"
fi

if [[ "$HAS_LOGGING" == "true" ]]; then
    log_pass "Observability: structured logging detected"
else
    log_skip "Observability: logging" "No structured logging detected"
fi

# 6. Architecture conformance (re-run to verify)
echo "Re-running architecture conformance..."
if [[ -f "${PI_DIR}/scripts/ci/check_architecture_conformance.sh" ]]; then
    if bash "${PI_DIR}/scripts/ci/check_architecture_conformance.sh" >/dev/null 2>&1; then
        log_pass "Architecture conformance (all checks pass)"
    else
        log_fail "Architecture conformance" "Some conformance checks failed — fix before closing epic"
    fi
else
    log_skip "Architecture conformance" "check_architecture_conformance.sh not found"
fi

# ── Summary ──

echo ""
echo "═══ Architecture Readiness Summary ═══"
echo -e "  ${GREEN}Pass: ${PASS}${NC}"
echo -e "  ${RED}Fail: ${FAIL}${NC}"
echo "  Total: $((PASS + FAIL))"

if [[ $FAIL -gt 0 ]]; then
    echo ""
    echo -e "${RED}✗ Architecture readiness FAILED. Fix the issues above before closing the epic.${NC}"
    exit 1
fi

echo ""
echo -e "${GREEN}✓ Architecture readiness complete. Epic can be closed.${NC}"
exit 0
