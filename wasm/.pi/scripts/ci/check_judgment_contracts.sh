#!/usr/bin/env bash
# ============================================================================
# check_judgment_contracts.sh — proofing for ADR-008 P1 (judgment-aware
# modeling). Deterministic proof that each frozen contract (§1–§7 of
# .pi/architecture/contracts/judgment-aware-modeling-p1.md) is implemented
# in the framework sources.
#
# Checks:
#   1. exploration prompt emits ADR-008 schema markers (src)
#   2. validate-exploration-depth.sh exists + executable
#   3. java model-judgment gate exists + executable
#   4. module-doc scaffold emits 'Modeling Depth' header (src)
#   5. java-spring codegen skill carries 'Judgment Tables (ADR-008)'
#   6. architecture-validator skill carries Model Judgment Review R1-R4
#   7. modeling contracts: ModuleComponent type carries the ADR-008 fields
#   8. exploration.md template carries the Modeling Judgment section
#
# Exit 0 = all contracts proven; 1 = at least one missing.
# ============================================================================
set -uo pipefail

ROOT="$(pwd)"
if [ "${1:-}" = "--root" ] && [ -n "${2:-}" ]; then ROOT="$2"; fi
cd "$ROOT" || exit 1

FAILED=0
PASSED=0

check() { # label, file-or-dir, grep pattern
    local label="$1" path="$2" pattern="$3"
    if [ -n "$path" ] && [ -e "$path" ] && grep -qE "$pattern" "$path" 2>/dev/null; then
        echo "  PASS: $label"
        PASSED=$((PASSED + 1))
    else
        echo "  FAIL: $label (missing: $path / $pattern)" >&2
        FAILED=$((FAILED + 1))
    fi
}

check_file() { # label file
    if [ -f "$2" ] && [ -x "$2" ]; then
        echo "  PASS: $1"
        PASSED=$((PASSED + 1))
    else
        echo "  FAIL: $1 (not found/executable: $2)" >&2
        FAILED=$((FAILED + 1))
    fi
}

echo "Judgment contracts proofing (ADR-008 P1)"
echo "----------------------------------------"

# 1. exploration prompt schema markers
check "exploration prompt: subdomainType/invariants/triage/contextMap/conflicts" \
    "src/lib/domain-explorer.ts" 'subdomainType|"invariants"|"triage"|"contextMap"|"conflicts"'

# 2. depth validator present + executable
check_file "validate-exploration-depth.sh executable" ".pi/scripts/validate-exploration-depth.sh"
check_file "validate-exploration-depth.sh (templates copy)" "templates/pi/scripts/validate-exploration-depth.sh"

# 3. java model-judgment gate present + executable
check_file "java model-judgment gate" ".pi/scripts/languages/java/check_model_judgment.sh"
check_file "java model-judgment gate (templates copy)" "templates/pi/scripts/languages/java/check_model_judgment.sh"

# 4. scaffold emits Modeling Depth header + invariants + depth note
check "scaffold: Modeling Depth header" "src/lib/domain-explorer.ts" 'Modeling Depth'
check "scaffold: depth note for service/adapter" "src/lib/domain-explorer.ts" 'Modeling Depth Note \(ADR-008\)'

# 5. java-spring codegen skill: judgment tables
check "java-spring skill: Judgment Tables (ADR-008)" \
    ".pi/skills/java-spring-enterprise-codegen.md" 'Judgment Tables \(ADR-008\)'

# 6. architecture-validator skill: R1-R4 review questions
check "architecture-validator skill: Model Judgment Review" \
    ".pi/skills/agents/architecture-validator.md" 'Model Judgment Review'

# 7. ModuleComponent modeling-contract fields
check "architect-lib types: modelingDepth/aggregateRoot/invariants/events" \
    ".pi/extensions/architect-lib/types.ts" 'modelingDepth|aggregateRoot|invariants|publishes'
check "generators: Modeling Contract emission" \
    ".pi/extensions/architect-lib/generators.ts" 'Modeling Contract \(ADR-008\)'

# 8. exploration.md template: Modeling Judgment section
check "exploration.md template: Modeling Judgment section" \
    "templates/pi/domain/exploration.md" 'Modeling Judgment \(ADR-008\)'

echo ""
echo "  Passed: $PASSED   Failed: $FAILED"
if [ "$FAILED" -gt 0 ]; then
    exit 1
fi
echo "All judgment contracts proven."
exit 0
