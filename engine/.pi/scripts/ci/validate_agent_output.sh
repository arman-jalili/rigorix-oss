#!/usr/bin/env bash
# Guardian — Agent Output Validator
#
# Validates agent-generated output for schema compliance, consistency, and completeness.
#
# Usage:
#   bash validate_agent_output.sh --input=output.md --schema=architecture-validator
#   bash validate_agent_output.sh --input=output.md --json
#   bash validate_agent_output.sh --input=output.md --no-coverage

set -euo pipefail

INPUT=""
SCHEMA="generic"
JSON=false
NO_COVERAGE=false

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

CHECKS_PASSED=0
CHECKS_FAILED=0
RESULTS=()

while [[ $# -gt 0 ]]; do
    case $1 in
        --input) INPUT="$2"; shift 2 ;;
        --schema) SCHEMA="$2"; shift 2 ;;
        --json) JSON=true; shift ;;
        --no-coverage) NO_COVERAGE=true; shift ;;
        *) shift ;;
    esac
done

if [[ -z "$INPUT" || ! -f "$INPUT" ]]; then
    echo "Error: --input <file> is required"
    exit 1
fi

CONTENT=$(cat "$INPUT")

# ── Validation Functions ──

check_section_present() {
    local section="$1"
    if echo "$CONTENT" | grep -qi "$section"; then
        RESULTS+=("{\"check\": \"$section\", \"status\": \"pass\"}")
        ((CHECKS_PASSED++))
        if [[ "$JSON" == "false" ]]; then
            echo -e "  ${GREEN}✓${NC} Section present: $section"
        fi
    else
        RESULTS+=("{\"check\": \"$section\", \"status\": \"fail\", \"message\": \"Section '$section' not found\"}")
        ((CHECKS_FAILED++))
        if [[ "$JSON" == "false" ]]; then
            echo -e "  ${RED}✗${NC} Missing section: $section"
        fi
    fi
}

check_no_contradictions() {
    # Check for common contradiction patterns
    local contradictions=0

    # Check for "must not" followed by "must" about the same topic
    while IFS= read -r line; do
        topic=$(echo "$line" | grep -oP '^\s*[-*]\s+\*\*[^*]+\*\*' | head -1 || echo "")
        if [[ -n "$topic" ]]; then
            # Count must/must not for this topic
            must_count=$(echo "$CONTENT" | grep -ci "${topic}.*must " || echo 0)
            must_not_count=$(echo "$CONTENT" | grep -ci "${topic}.*must not\|${topic}.*must not " || echo 0)
            if [[ $must_count -gt 0 && $must_not_count -gt 0 ]]; then
                ((contradictions++))
            fi
        fi
    done < <(echo "$CONTENT" | grep -E "^\s*[-*]\s+\*\*" || true)

    if [[ $contradictions -eq 0 ]]; then
        RESULTS+=("{\"check\": \"no_contradictions\", \"status\": \"pass\"}")
        ((CHECKS_PASSED++))
        if [[ "$JSON" == "false" ]]; then
            echo -e "  ${GREEN}✓${NC} No internal contradictions"
        fi
    else
        RESULTS+=("{\"check\": \"no_contradictions\", \"status\": \"fail\", \"message\": \"Found $contradictions contradictions\"}")
        ((CHECKS_FAILED++))
        if [[ "$JSON" == "false" ]]; then
            echo -e "  ${RED}✗${NC} Found $contradictions contradictions"
        fi
    fi
}

check_decision_consistency() {
    # Check that all decisions have supporting evidence
    local decisions=$(echo "$CONTENT" | grep -ci "decision\|recommendation\|conclusion" || echo 0)
    local evidence=$(echo "$CONTENT" | grep -ci "evidence\|because\|reason\|rationale\|justification" || echo 0)

    if [[ $decisions -eq 0 ]] || [[ $evidence -gt 0 ]]; then
        RESULTS+=("{\"check\": \"decision_consistency\", \"status\": \"pass\"}")
        ((CHECKS_PASSED++))
        if [[ "$JSON" == "false" ]]; then
            echo -e "  ${GREEN}✓${NC} Decision consistency"
        fi
    else
        RESULTS+=("{\"check\": \"decision_consistency\", \"status\": \"fail\", \"message\": \"Decisions without supporting evidence\"}")
        ((CHECKS_FAILED++))
        if [[ "$JSON" == "false" ]]; then
            echo -e "  ${RED}✗${NC} Decisions without supporting evidence"
        fi
    fi
}

check_acceptance_criteria() {
    # Check that acceptance criteria are present and evaluable
    local has_criteria=$(echo "$CONTENT" | grep -ci "acceptance criteria\|criteria\|must\|should\|required" || echo 0)
    local has_checklist=$(echo "$CONTENT" | grep -c "\- \[ \]" || echo 0)

    if [[ $has_criteria -gt 0 ]]; then
        RESULTS+=("{\"check\": \"acceptance_criteria\", \"status\": \"pass\"}")
        ((CHECKS_PASSED++))
        if [[ "$JSON" == "false" ]]; then
            echo -e "  ${GREEN}✓${NC} Acceptance criteria present"
        fi
    else
        RESULTS+=("{\"check\": \"acceptance_criteria\", \"status\": \"fail\", \"message\": \"No acceptance criteria found\"}")
        ((CHECKS_FAILED++))
        if [[ "$JSON" == "false" ]]; then
            echo -e "  ${RED}✗${NC} No acceptance criteria found"
        fi
    fi
}

check_canonical_references() {
    local has_refs=$(echo "$CONTENT" | grep -c "Canonical Reference\|canonical.*reference\|\.pi/architecture" || echo 0)

    if [[ $has_refs -gt 0 ]]; then
        RESULTS+=("{\"check\": \"canonical_references\", \"status\": \"pass\"}")
        ((CHECKS_PASSED++))
        if [[ "$JSON" == "false" ]]; then
            echo -e "  ${GREEN}✓${NC} Canonical references present"
        fi
    else
        RESULTS+=("{\"check\": \"canonical_references\", \"status\": \"fail\", \"message\": \"No canonical references found\"}")
        ((CHECKS_FAILED++))
        if [[ "$JSON" == "false" ]]; then
            echo -e "  ${RED}✗${NC} No canonical references found"
        fi
    fi
}

# ── Schema-Specific Checks ──

case "$SCHEMA" in
    architecture-validator)
        check_section_present "## Summary"
        check_section_present "## Findings"
        check_section_present "## Recommendations"
        check_section_present "## Acceptance Criteria"
        check_section_present "## Canonical References"
        check_no_contradictions
        check_decision_consistency
        check_acceptance_criteria
        check_canonical_references
        ;;

    epic-plan)
        check_section_present "## Why"
        check_section_present "## Target Outcome"
        check_section_present "## Scope"
        check_section_present "## In Scope"
        check_section_present "## Out of Scope"
        check_section_present "## Impacted Layers"
        check_section_present "## Architecture Constraints"
        check_section_present "## Dependency Map"
        check_section_present "## Canonical References"
        check_acceptance_criteria
        check_canonical_references
        ;;

    issue-draft)
        check_section_present "## Intent"
        check_section_present "## Scope"
        check_section_present "## In Scope"
        check_section_present "## Out of Scope"
        check_section_present "## Dependencies"
        check_section_present "## Acceptance Criteria"
        check_section_present "## Canonical References"
        check_acceptance_criteria
        check_canonical_references
        ;;

    *)
        # Generic validation
        check_section_present "## Summary"
        check_no_contradictions
        check_decision_consistency
        check_acceptance_criteria
        ;;
esac

# ── Summary ──

TOTAL=$((CHECKS_PASSED + CHECKS_FAILED))

if [[ "$JSON" == "true" ]]; then
    RESULTS_JSON=$(printf '%s\n' "${RESULTS[@]}" | jq -s .)
    cat << EOF
{
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "input": "$INPUT",
  "schema": "$SCHEMA",
  "summary": {
    "total": $TOTAL,
    "passed": $CHECKS_PASSED,
    "failed": $CHECKS_FAILED
  },
  "checks": ${RESULTS_JSON},
  "status": "$([ $CHECKS_FAILED -gt 0 ] && echo "fail" || echo "pass")"
}
EOF
else
    echo ""
    echo "================================"
    echo "AGENT OUTPUT VALIDATION SUMMARY"
    echo "================================"
    echo "Input:       $INPUT"
    echo "Schema:      $SCHEMA"
    echo "Total checks: $TOTAL"
    echo -e "Passed:      ${GREEN}${CHECKS_PASSED}${NC}"
    echo -e "Failed:      ${RED}${CHECKS_FAILED}${NC}"
    echo "================================"

    if [[ $CHECKS_FAILED -eq 0 ]]; then
        echo -e "${GREEN}✅ Agent output validation passed${NC}"
    else
        echo -e "${RED}❌ Agent output validation failed (${CHECKS_FAILED} check(s))${NC}"
    fi
fi

exit $([ $CHECKS_FAILED -gt 0 ] && echo 1 || echo 0)
