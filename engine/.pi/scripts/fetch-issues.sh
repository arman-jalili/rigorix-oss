#!/usr/bin/env bash
# ============================================================================
# fetch-issues.sh — Fetch Issues from GitHub or GitLab for Implementation
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

detect_platform() {
	if [ -f "guardian-manifest.json" ]; then
		local tool=$(jq -r '.repoTool // ""' guardian-manifest.json 2>/dev/null || echo "")
		if [[ "$tool" == "glab" ]]; then echo "gitlab"; return; fi
		if [[ "$tool" == "gh" ]]; then echo "github"; return; fi
	fi
	if [[ -n "${GIT_PLATFORM:-}" ]]; then
		echo "$GIT_PLATFORM"
	elif command -v gh &>/dev/null && gh auth status &>/dev/null 2>&1; then
		echo "github"
	elif command -v glab &>/dev/null && glab auth status &>/dev/null 2>&1; then
		echo "gitlab"
	else
		echo "none"
	fi
}

echo "============================================"
echo "  Fetch Issues"
echo "============================================"
echo ""

PLATFORM=$(detect_platform)

if [[ "$PLATFORM" == "none" ]]; then
	echo -e "${RED}❌ No git platform detected${NC}"
	echo "  Install and authenticate gh (GitHub) or glab (GitLab) CLI."
	exit 1
fi

# Ensure output directory exists
mkdir -p "$OUTPUT_DIR"

# ---------------------------------------------------------------------------
# Fetch Issues
# ---------------------------------------------------------------------------
echo "Fetching $STATE issues from $PLATFORM (limit: $LIMIT)..."

case "$PLATFORM" in
	github)
		gh issue list \
			--state "$STATE" \
			--limit "$LIMIT" \
			--json number,title,labels,body,state,createdAt,updatedAt,assignees \
			> "$OUTPUT_FILE"
		;;

	gitlab)
		# glab does not export the same rich JSON schema as gh, so we build it
		glab issue list \
			--state "$STATE" \
			--per-page "$LIMIT" \
			--output json 2>/dev/null > "$OUTPUT_FILE.tmp" || true

		# Convert glab output to match gh schema (if possible)
		if [[ -s "$OUTPUT_FILE.tmp" ]]; then
			jq '[.[] | {
				number: (.iid // .id),
				title,
				labels: (.labels // []),
				body: (.description // ""),
				state: (.state | ascii_downcase),
				createdAt: (.created_at // ""),
				updatedAt: (.updated_at // ""),
				assignees: ([.assignee? | select(. != null) | {login: .username, name: .name}] // [])
			}]' "$OUTPUT_FILE.tmp" > "$OUTPUT_FILE" 2>/dev/null || mv "$OUTPUT_FILE.tmp" "$OUTPUT_FILE"
			rm -f "$OUTPUT_FILE.tmp"
		else
			echo "[]" > "$OUTPUT_FILE"
			rm -f "$OUTPUT_FILE.tmp"
		fi
		;;
esac

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
