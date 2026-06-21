#!/usr/bin/env bash
# Git Wrapper: Update Tracking Issue
#
# Posts a progress update comment to a tracking issue.
#
# Usage: bash .pi/scripts/git/update-tracking-issue.sh \
#   --id 100 \
#   --comment "✓ Issue #102 complete (CI green, tests pass)"

set -euo pipefail

detect_platform() {
    if [ -f "guardian-manifest.json" ]; then
        local tool=$(jq -r '.repoTool // ""' guardian-manifest.json 2>/dev/null || echo "")
        if [[ "$tool" == "glab" ]]; then echo "gitlab"; return; fi
        if [[ "$tool" == "gh" ]]; then echo "github"; return; fi
    fi
    if [[ -n "${GIT_PLATFORM:-}" ]]; then echo "$GIT_PLATFORM"
    elif command -v gh &>/dev/null && gh auth status &>/dev/null 2>&1; then echo "github"
    elif command -v glab &>/dev/null && glab auth status &>/dev/null 2>&1; then echo "gitlab"
    else echo "none"; fi
}

ISSUE_ID=""
COMMENT=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --id) ISSUE_ID="$2"; shift 2 ;;
        --comment) COMMENT="$2"; shift 2 ;;
        *) shift ;;
    esac
done

PLATFORM=$(detect_platform)

if [[ -z "$ISSUE_ID" || -z "$COMMENT" ]]; then
    echo "Usage: $0 --id <issue_number> --comment <comment_text>"
    exit 1
fi

# Handle local tracking files
if [[ "$ISSUE_ID" == local:* ]]; then
    TRACKING_FILE=".pi/.tracking/$(echo "$ISSUE_ID" | cut -d: -f2)"
    if [[ -f "$TRACKING_FILE" ]]; then
        echo "" >> "$TRACKING_FILE"
        echo "---" >> "$TRACKING_FILE"
        echo "$COMMENT" >> "$TRACKING_FILE"
        echo "Updated local tracking file: $TRACKING_FILE"
    fi
    exit 0
fi

case "$PLATFORM" in
    github)
        gh issue comment "$ISSUE_ID" --body "$COMMENT" 2>/dev/null && echo "Comment posted to GitHub issue #$ISSUE_ID"
        ;;
    gitlab)
        glab issue note "$ISSUE_ID" --message "$COMMENT" 2>/dev/null && echo "Comment posted to GitLab issue #$ISSUE_ID"
        ;;
    none)
        echo "No git platform detected. Comment not posted."
        echo "  Issue: #$ISSUE_ID"
        echo "  Comment: $COMMENT"
        ;;
esac
