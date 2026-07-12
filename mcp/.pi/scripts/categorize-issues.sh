#!/usr/bin/env bash
# ============================================================================
# categorize-issues.sh — Categorize Issues into Implementation Groups
#
# Run as: bash .pi/scripts/categorize-issues.sh
# Input: .claude/plans/issues-fetched.json
# Output: .claude/plans/issue-groups.md
# ============================================================================
set -euo pipefail

INPUT_FILE=".claude/plans/issues-fetched.json"
OUTPUT_FILE=".claude/plans/issue-groups.md"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo "============================================"
echo "  Categorize Issues into Groups"
echo "============================================"
echo ""

# ---------------------------------------------------------------------------
# Check Input
# ---------------------------------------------------------------------------
if [ ! -f "$INPUT_FILE" ]; then
    echo -e "${RED}❌ No issues file found${NC}"
    echo "  Run: bash .pi/scripts/fetch-issues.sh first"
    exit 1
fi

COUNT=$(jq 'length' "$INPUT_FILE")
echo "Processing $COUNT issues..."

# ---------------------------------------------------------------------------
# Generate Groups
# ---------------------------------------------------------------------------
cat > "$OUTPUT_FILE" << 'EOF'
# Issue Implementation Groups

**Generated:** $(date)
**Total Issues:** $COUNT

---

## Grouping Strategy

Issues are grouped by:
1. **Component** — Same module/files affected
2. **Priority** — Critical > High > Medium > Low
3. **Dependency** — Blocking relationships

---

EOF

# ---------------------------------------------------------------------------
# Priority Groups
# ---------------------------------------------------------------------------
echo ""
echo "--- Priority Groups ---"

# Critical issues
CRITICAL_ISSUES=$(jq -c '[.[] | select(.labels[]?.name == "critical" or .labels[]?.name == "priority/critical")] | sort_by(.number)' "$INPUT_FILE")
CRITICAL_COUNT=$(echo "$CRITICAL_ISSUES" | jq 'length')

if [ "$CRITICAL_COUNT" -gt 0 ]; then
    echo -e "${RED}Critical: $CRITICAL_COUNT issues${NC}"
    cat >> "$OUTPUT_FILE" << EOF

### Group: Critical Priority

**Branch:** \`priority/critical-$(date +%Y%m%d)\`
**Issues:** $CRITICAL_COUNT

EOF
    echo "$CRITICAL_ISSUES" | jq -r '.[] | "- #\(.number): \(.title)"' >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
fi

# High priority
HIGH_ISSUES=$(jq -c '[.[] | select(.labels[]?.name == "high" or .labels[]?.name == "priority/high")]' "$INPUT_FILE")
HIGH_COUNT=$(echo "$HIGH_ISSUES" | jq 'length')

if [ "$HIGH_COUNT" -gt 0 ]; then
    echo -e "${YELLOW}High: $HIGH_COUNT issues${NC}"
    cat >> "$OUTPUT_FILE" << EOF

### Group: High Priority

**Branch:** \`priority/high-$(date +%Y%m%d)\`
**Issues:** $HIGH_COUNT

EOF
    echo "$HIGH_ISSUES" | jq -r '.[] | "- #\(.number): \(.title)"' >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
fi

# ---------------------------------------------------------------------------
# Component Groups (by label patterns)
# ---------------------------------------------------------------------------
echo ""
echo "--- Component Groups ---"

# Extract unique component labels
COMPONENTS=$(jq -r '[.[] | .labels[]?.name | select(. | startswith("component/") or startswith("area/") or startswith("module/"))] | unique | .[]' "$INPUT_FILE" 2>/dev/null || true)

for COMP in $COMPONENTS; do
    COMP_NAME=$(echo "$COMP" | sed 's/component\|area\|module\///')
    COMP_ISSUES=$(jq -c '[.[] | select(.labels[]?.name == "'$COMP'")] | sort_by(.number)' "$INPUT_FILE")
    COMP_COUNT=$(echo "$COMP_ISSUES" | jq 'length')

    if [ "$COMP_COUNT" -ge 2 ]; then
        echo -e "${BLUE}Component '$COMP_NAME': $COMP_COUNT issues${NC}"
        cat >> "$OUTPUT_FILE" << EOF

### Group: $COMP_NAME

**Branch:** \`feature/$COMP_NAME-issues\`
**Component:** $COMP
**Issues:** $COMP_COUNT

EOF
        echo "$COMP_ISSUES" | jq -r '.[] | "- #\(.number): \(.title)"' >> "$OUTPUT_FILE"
        echo "" >> "$OUTPUT_FILE"
    fi
done

# ---------------------------------------------------------------------------
# Individual Issues (no grouping)
# ---------------------------------------------------------------------------
echo ""
echo "--- Individual Issues ---"

SINGLE_ISSUES=$(jq -c '[.[] | select(
    (.labels | length == 0) or
    (.labels[]?.name != "critical" and .labels[]?.name != "high" and
     (.labels[]?.name | startswith("component/") | not) and
     (.labels[]?.name | startswith("area/") | not) and
     (.labels[]?.name | startswith("module/") | not))
)]' "$INPUT_FILE" 2>/dev/null || jq -c '[.[]]' "$INPUT_FILE")
SINGLE_COUNT=$(echo "$SINGLE_ISSUES" | jq 'length')

if [ "$SINGLE_COUNT" -gt 0 ]; then
    echo -e "${NC}Individual: $SINGLE_COUNT issues${NC}"
    cat >> "$OUTPUT_FILE" << EOF

### Individual Issues (No Grouping)

Each requires separate branch: \`issue/{number}\`

EOF
    echo "$SINGLE_ISSUES" | jq -r '.[] | "- #\(.number): \(.title) → branch: \`issue/\(.number)\`"' >> "$OUTPUT_FILE"
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "============================================"
echo "  Summary"
echo "============================================"
echo -e "  Critical groups: ${RED}${CRITICAL_COUNT}${NC}"
echo -e "  High groups:     ${YELLOW}${HIGH_COUNT}${NC}"
echo -e "  Individual:      ${NC}${SINGLE_COUNT}${NC}"
echo ""
echo "Output: $OUTPUT_FILE"

exit 0