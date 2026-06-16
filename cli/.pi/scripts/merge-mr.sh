#!/usr/bin/env bash
# Merge Merge Request / Pull Request
#
# Merges the current PR via GitHub CLI when CI checks pass.
#
# Usage: bash .pi/scripts/merge-mr.sh --issue-id "issue-github-gitlab-connector"

set -euo pipefail

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

ISSUE_ID=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --issue-id) ISSUE_ID="$2"; shift 2 ;;
        *) shift ;;
    esac
done

PLATFORM=$(detect_platform)
BRANCH_NAME="feature/${ISSUE_ID}"

if [[ "$PLATFORM" == "none" ]]; then
    echo "No git platform detected. Cannot merge without gh/glab CLI."
    echo "Merge skipped — local only."
    exit 0
fi

case "$PLATFORM" in
    github)
        # Find the PR for this branch
        PR_NUMBER=$(gh pr list --head "$BRANCH_NAME" --json number -q '.[0].number' 2>/dev/null || echo "")

        if [[ -z "$PR_NUMBER" ]]; then
            echo "No PR found for branch $BRANCH_NAME"
            exit 0
        fi

        # Check PR mergeability
        PR_STATE=$(gh pr view "$PR_NUMBER" --json state -q '.state' 2>/dev/null || echo "unknown")
        if [[ "$PR_STATE" == "MERGED" ]]; then
            echo "PR #$PR_NUMBER already merged"
            exit 0
        fi

        if [[ "$PR_STATE" == "CLOSED" ]]; then
            echo "PR #$PR_NUMBER is closed"
            exit 0
        fi

        # Attempt merge with squash
        MERGE_RESULT=$(gh pr merge "$PR_NUMBER" --squash --delete-branch 2>&1) || MERGE_RESULT=""

        if [[ -n "$MERGE_RESULT" ]]; then
            echo "Merged PR #$PR_NUMBER: $MERGE_RESULT"
            # Switch back to main branch
            git checkout main 2>/dev/null || git checkout master 2>/dev/null || true
            git pull origin main 2>/dev/null || git pull origin master 2>/dev/null || true
        else
            echo "Merge failed for PR #$PR_NUMBER" >&2
            exit 1
        fi
        ;;

    gitlab)
        MR_IID=$(glab mr list --source-branch "$BRANCH_NAME" --json iid -q '.[0].iid' 2>/dev/null || echo "")

        if [[ -z "$MR_IID" ]]; then
            echo "No MR found for branch $BRANCH_NAME"
            exit 0
        fi

        glab mr merge "$MR_IID" --when-pipeline-succeeds 2>&1 || true
        git checkout main 2>/dev/null || git checkout master 2>/dev/null || true
        git pull origin main 2>/dev/null || git pull origin master 2>/dev/null || true
        ;;
esac
