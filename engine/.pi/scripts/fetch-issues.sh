#!/usr/bin/env bash
# ============================================================================
# fetch-issues.sh — Fetch GitHub Issues for Implementation
#
# Run as: bash .pi/scripts/fetch-issues.sh [limit] [state]
# Output: .claude/plans/issues-fetched.json
# ============================================================================
set -euo pipefail

LIMIT="${1:-50}"
STATE="${2:-open}"
OUTPUT_DIR=".claude/plans"
OUTPUT_FILE="$OUTPUT_DIR/issues-fetched.json"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo "============================================"
echo "  Fetch GitHub Issues"
echo "============================================"
echo ""

# Ensure output directory exists
mkdir -p "$OUTPUT_DIR"

# ---------------------------------------------------------------------------
# Check GitHub CLI
# ---------------------------------------------------------------------------
if ! command -v gh &> /dev/null; then
    echo -e "${RED}❌ GitHub CLI (gh) not installed${NC}"
    echo "  Install: brew install gh"
    exit 1
fi

# Check authentication
if ! gh auth status &> /dev/null; then
    echo -e "${RED}❌ GitHub CLI not authenticated${NC}"
    echo "  Run: gh auth login"
    exit 1
fi

# ---------------------------------------------------------------------------
# Fetch Issues
# ---------------------------------------------------------------------------
echo "Fetching $STATE issues (limit: $LIMIT)..."

gh issue list \
    --state "$STATE" \
    --limit "$LIMIT" \
    --json number,title,labels,body,state,createdAt,updatedAt,assignees \
    > "$OUTPUT_FILE"

COUNT=$(jq 'length' "$OUTPUT_FILE")
echo -e "${GREEN}✅ Fetched $COUNT issues${NC}"

# ---------------------------------------------------------------------------
# Categorize by Priority
# ---------------------------------------------------------------------------
echo ""
echo "--- Priority Summary ---"

CRITICAL=$(jq '[.[] | select(.labels[]?.name == "critical" or .labels[]?.name == "priority/critical")] | length' "$OUTPUT_FILE")
HIGH=$(jq '[.[] | select(.labels[]?.name == "high" or .labels[]?.name == "priority/high")] | length' "$OUTPUT_FILE")
MEDIUM=$(jq '[.[] | select(.labels[]?.name == "medium" or .labels[]?.name == "priority/medium")] | length' "$OUTPUT_FILE")
LOW=$(jq '[.[] | select(.labels[]?.name == "low" or .labels[]?.name == "priority/low")] | length' "$OUTPUT_FILE")
UNLABEL=$(jq '[.[] | select(.labels | length == 0)] | length' "$OUTPUT_FILE")

echo -e "  Critical: ${RED}${CRITICAL}${NC}"
echo -e "  High:     ${YELLOW}${HIGH}${NC}"
echo -e "  Medium:   ${GREEN}${MEDIUM}${NC}"
echo -e "  Low:      ${GREEN}${LOW}${NC}"
echo -e "  No label: ${NC}${UNLABEL}${NC}"

# ---------------------------------------------------------------------------
# List Issues
# ---------------------------------------------------------------------------
echo ""
echo "--- Issue List ---"
jq -r '.[] | "#\(.number) - \(.title) [\(.labels[]?.name // "none")]"' "$OUTPUT_FILE" | head -20

if [ "$COUNT" -gt 20 ]; then
    echo "... and $((COUNT - 20)) more"
fi

echo ""
echo "============================================"
echo "  Output: $OUTPUT_FILE"
echo "============================================"

exit 0