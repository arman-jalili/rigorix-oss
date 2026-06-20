#!/usr/bin/env bash
# Create Merge Request / Pull Request
#
# Creates a feature branch, pushes to remote, and opens a PR via GitHub CLI.
# Uses the current issue context to generate PR title and body.
#
# Usage: bash .pi/scripts/create-mr.sh \
#   --issue-id "issue-github-gitlab-connector" \
#   --epic-name "Module 1: Repository Ingestion"

set -euo pipefail

detect_platform() {
    # First: check guardian-manifest.json for repoTool
    if [ -f "guardian-manifest.json" ]; then
        local tool=$(jq -r '.repoTool // ""' guardian-manifest.json 2>/dev/null || echo "")
        if [[ "$tool" == "glab" ]]; then echo "gitlab"; return; fi
        if [[ "$tool" == "gh" ]]; then echo "github"; return; fi
    fi
    # Fallback: check environment or installed CLIs
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

ISSUE_ID=""
EPIC_NAME=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --issue-id) ISSUE_ID="$2"; shift 2 ;;
        --epic-name) EPIC_NAME="$2"; shift 2 ;;
        *) shift ;;
    esac
done

PLATFORM=$(detect_platform)

# Run pre-validation before creating MR
if ! bash .pi/scripts/validate-ci.sh 2>/dev/null; then
    echo "CI validation failed. Aborting MR creation." >&2
    exit 1
fi


if [[ "$PLATFORM" == "none" ]]; then
    echo "No git platform detected. Cannot create MR without gh/glab CLI."
    echo "MR creation skipped — local only."
    exit 0
fi

# Determine branch name from issue ID
BRANCH_NAME="feature/${ISSUE_ID}"

# Ensure we're on the main branch before creating feature branch
git checkout -b "$BRANCH_NAME" 2>/dev/null || git checkout "$BRANCH_NAME" 2>/dev/null || true

# Push branch to remote
git push -u origin "$BRANCH_NAME" 2>/dev/null || git push origin "$BRANCH_NAME" 2>/dev/null || true

# Read issue file for context if available
ISSUE_FILE=".pi/issues/${ISSUE_ID}.md"
PR_BODY=""
if [[ -f "$ISSUE_FILE" ]]; then
    # Extract the human-readable body (after frontmatter)
    PR_BODY=$(awk '/^---$/{n++; next} n>=2{print}' "$ISSUE_FILE")
fi

PR_TITLE="${ISSUE_ID}: Implement ${EPIC_NAME}"

case "$PLATFORM" in
    github)
        # Check if PR already exists
        EXISTING_PR=$(gh pr list --head "$BRANCH_NAME" --json number 2>/dev/null || echo "[]")
        if [[ "$EXISTING_PR" != "[]" ]]; then
            echo "PR already exists for branch $BRANCH_NAME"
            gh pr view --head "$BRANCH_NAME" --json url -q '.url' 2>/dev/null || true
            exit 0
        fi

        if [[ -n "$PR_BODY" ]]; then
            PR_URL=$(gh pr create --head "$BRANCH_NAME" --title "$PR_TITLE" --body "$PR_BODY" 2>&1) || PR_URL=""
        else
            PR_URL=$(gh pr create --head "$BRANCH_NAME" --title "$PR_TITLE" 2>&1) || PR_URL=""
        fi

        if [[ -n "$PR_URL" ]]; then
            echo "MR_URL=$PR_URL"
            echo "MR_BRANCH=$BRANCH_NAME"
        else
            echo "Failed to create PR" >&2
            exit 1
        fi
        ;;

    gitlab)
        if [[ -n "$PR_BODY" ]]; then
            MR_URL=$(glab mr create --source-branch "$BRANCH_NAME" --title "$PR_TITLE" --description "$PR_BODY" 2>&1) || MR_URL=""
        else
            MR_URL=$(glab mr create --source-branch "$BRANCH_NAME" --title "$PR_TITLE" 2>&1) || MR_URL=""
        fi

        if [[ -n "$MR_URL" ]]; then
            echo "MR_URL=$MR_URL"
            echo "MR_BRANCH=$BRANCH_NAME"
        else
            echo "Failed to create MR" >&2
            exit 1
        fi
        ;;
esac
