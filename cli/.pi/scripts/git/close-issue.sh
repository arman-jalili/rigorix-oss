#!/usr/bin/env bash
# Git Wrapper: Close Issue
#
# Usage: bash .pi/scripts/git/close-issue.sh --id 101 [--repo "group/project"]

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

read_repository() {
    if [ -f "guardian-manifest.json" ]; then
        jq -r '.repository // (.templateContext.repository // "")' guardian-manifest.json 2>/dev/null || echo ""
    else
        echo ""
    fi
}

ISSUE_ID=""
REPO=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --id) ISSUE_ID="$2"; shift 2 ;;
        --repo) REPO="$2"; shift 2 ;;
        *) shift ;;
    esac
done

[[ -z "$ISSUE_ID" ]] && { echo "Usage: $0 --id <issue_number> [--repo <repo>]"; exit 1; }
PLATFORM=$(detect_platform)
[[ -z "$REPO" ]] && REPO=$(read_repository)

case "$PLATFORM" in
    github) gh issue close "$ISSUE_ID" ${REPO:+--repo "$REPO"} 2>/dev/null && echo "Closed GitHub issue #$ISSUE_ID" ;;
    gitlab) glab issue update "$ISSUE_ID" --state-event close ${REPO:+--repo "$REPO"} 2>/dev/null && echo "Closed GitLab issue #$ISSUE_ID" ;;
    *) echo "Local issue #$ISSUE_ID marked as closed (no platform)" ;;
esac
