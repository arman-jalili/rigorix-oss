#!/usr/bin/env bash
# ============================================================================
# mr-validation.sh — Validate Merge Request / Pull Request (GitHub & GitLab)
#
# Run as: bash .pi/scripts/mr-validation.sh [pr-number]
# Checks: CI status, architecture, security, integration
# ============================================================================
set -euo pipefail

PR_NUMBER="${1:-}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

ERRORS=()
PASS_COUNT=0

pass() { echo -e "${GREEN}✅ PASS${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}❌ FAIL${NC} $1"; ERRORS+=("$1"); }
warn() { echo -e "${YELLOW}⚠️  WARN${NC} $1"; }

detect_platform() {
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
echo "  MR Validation"
echo "============================================"
echo ""

PLATFORM=$(detect_platform)

if [[ "$PLATFORM" == "none" ]]; then
	echo -e "${RED}❌ No git platform detected${NC}"
	echo "  Install and authenticate gh (GitHub) or glab (GitLab) CLI."
	exit 1
fi

# ---------------------------------------------------------------------------
# Get MR/PR Number
# ---------------------------------------------------------------------------
if [ -z "$PR_NUMBER" ]; then
	BRANCH_NAME=$(git branch --show-current)

	case "$PLATFORM" in
		github)
			PR_NUMBER=$(gh pr view --json number -q '.number' 2>/dev/null || echo "")
			;;
		gitlab)
			PR_NUMBER=$(glab mr list --source-branch "$BRANCH_NAME" --json 'iid' -q '.[0].iid' 2>/dev/null || echo "")
			;;
	esac

	if [ -z "$PR_NUMBER" ]; then
		echo -e "${RED}❌ MR/PR number required${NC}"
		echo "Usage: mr-validation.sh [mr-number]"
		echo "  Or run from the MR/PR branch"
		exit 1
	fi
fi

echo "Validating MR/PR #$PR_NUMBER on $PLATFORM..."

# ---------------------------------------------------------------------------
# Get MR/PR Status
# ---------------------------------------------------------------------------
case "$PLATFORM" in
	github)
		PR_INFO=$(gh pr view "$PR_NUMBER" --json title,state,mergeable,statusCheckRollup,headRefName 2>/dev/null || echo "{}")
		PR_STATE=$(echo "$PR_INFO" | jq -r '.state // "unknown"')
		PR_MERGEABLE=$(echo "$PR_INFO" | jq -r '.mergeable // "unknown"')
		PR_BRANCH=$(echo "$PR_INFO" | jq -r '.headRefName // "unknown"')
		echo "Title: $(echo "$PR_INFO" | jq -r '.title')"
		;;
	gitlab)
		MR_INFO=$(glab mr view "$PR_NUMBER" --output json 2>/dev/null || echo "{}")
		PR_STATE=$(echo "$MR_INFO" | jq -r '.state // "unknown"')
		PR_MERGEABLE=$(echo "$MR_INFO" | jq -r '.merge_status // "unknown"')
		PR_BRANCH=$(echo "$MR_INFO" | jq -r '.source_branch // "unknown"')
		echo "Title: $(echo "$MR_INFO" | jq -r '.title')"
		;;
esac

echo "State: $PR_STATE"
echo "Branch: $PR_BRANCH"
echo ""

# ---------------------------------------------------------------------------
# CI Status Check
# ---------------------------------------------------------------------------
echo "--- CI Status ---"

case "$PLATFORM" in
	github)
		STATUS_CHECKS=$(echo "$PR_INFO" | jq -r '.statusCheckRollup[]?.status // "unknown"' 2>/dev/null || echo "unknown")
		;;
	gitlab)
		STATUS_CHECKS=$(echo "$MR_INFO" | jq -r '.head_pipeline.status // "unknown"' 2>/dev/null || echo "unknown")
		;;
esac

if [ "$STATUS_CHECKS" = "SUCCESS" ] || [ "$STATUS_CHECKS" = "success" ] || [ "$STATUS_CHECKS" = "passed" ]; then
	pass "CI checks passed"
elif [ "$STATUS_CHECKS" = "IN_PROGRESS" ] || [ "$STATUS_CHECKS" = "pending" ] || [ "$STATUS_CHECKS" = "running" ]; then
	warn "CI checks in progress"
	echo "  Wait for completion and re-run"
else
	fail "CI checks failed or unknown: $STATUS_CHECKS"
fi

# ---------------------------------------------------------------------------
# Mergeable Check
# ---------------------------------------------------------------------------
echo ""
echo "--- Mergeable Status ---"

if [ "$PR_MERGEABLE" = "MERGEABLE" ] || [ "$PR_MERGEABLE" = "true" ] || [ "$PR_MERGEABLE" = "can_be_merged" ]; then
	pass "MR/PR is mergeable"
elif [ "$PR_MERGEABLE" = "CONFLICTING" ] || [ "$PR_MERGEABLE" = "false" ] || [ "$PR_MERGEABLE" = "cannot_be_merged" ]; then
	fail "MR/PR has merge conflicts"
	echo "  Resolve conflicts before merge"
else
	warn "Mergeable status unknown: $PR_MERGEABLE"
fi

# ---------------------------------------------------------------------------
# Run Local Validators
# ---------------------------------------------------------------------------
echo ""
echo "--- Local Validation ---"

# Checkout MR/PR branch
git checkout "$PR_BRANCH" 2>/dev/null || {
	warn "Could not checkout branch $PR_BRANCH"
}

# Run validation scripts
if [ -f ".pi/scripts/validate-ci.sh" ]; then
	bash .pi/scripts/validate-ci.sh 2>&1 | tail -5
fi

if [ -f ".pi/scripts/validate-security.sh" ]; then
	bash .pi/scripts/validate-security.sh 2>&1 | tail -5
fi

# ---------------------------------------------------------------------------
# Architecture Check
# ---------------------------------------------------------------------------
echo ""
echo "--- Architecture Check ---"

case "$PLATFORM" in
	github)
		CHANGED_FILES=$(gh pr diff "$PR_NUMBER" --name-only 2>/dev/null || git diff --name-only "origin/main"..HEAD)
		;;
	gitlab)
		CHANGED_FILES=$(glab mr diff "$PR_NUMBER" --name-only 2>/dev/null || git diff --name-only "origin/main"..HEAD)
		;;
esac

for FILE in $CHANGED_FILES; do
	if [ -f "$FILE" ] && [[ "$FILE" == *.rs ]]; then
		NEW_FNS=$(git diff "origin/main"..HEAD -- "$FILE" | grep "^[+]pub fn\|^[+]pub async fn" || true)
		if [ -n "$NEW_FNS" ]; then
			echo -e "${BLUE}ℹ️  New public functions in $FILE:${NC}"
			echo "$NEW_FNS" | head -5
		fi
	fi
done

# ---------------------------------------------------------------------------
# Check for Tests
# ---------------------------------------------------------------------------
echo ""
echo "--- Test Coverage Check ---"

TEST_FILES=$(echo "$CHANGED_FILES" | grep -E "test.*\.|\.test\.|tests/" || true)
SRC_FILES=$(echo "$CHANGED_FILES" | grep -E "src/|lib/" | grep -v test || true)

if [ -n "$SRC_FILES" ] && [ -z "$TEST_FILES" ]; then
	warn "Source files changed but no test files"
	echo "  Consider adding tests for:"
	echo "$SRC_FILES" | head -5
else
	pass "Test coverage present"
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "============================================"
echo "  Summary"
echo "============================================"
echo -e "  Passed:   ${GREEN}${PASS_COUNT}${NC}"
echo -e "  Failed:   ${RED}${#ERRORS[@]}${NC}"
echo ""

if [ ${#ERRORS[@]} -gt 0 ]; then
	echo "BLOCKERS:"
	for err in "${ERRORS[@]}"; do
		echo "  - $err"
	done
	echo ""
	echo "Fix blockers before merging"
	exit 1
fi

echo -e "${GREEN}MR validation passed. Ready for merge.${NC}"
echo ""

exit 0
