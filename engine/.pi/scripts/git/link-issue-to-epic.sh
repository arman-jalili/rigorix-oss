#!/usr/bin/env bash
# Git Wrapper: Link Issue to Epic
#
# Usage: bash .pi/scripts/git/link-issue-to-epic.sh \
#   --issue-id 102 \
#   --epic-id 101 \
#   [--repo "group/project"]

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
EPIC_ID=""
REPO=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --issue-id) ISSUE_ID="$2"; shift 2 ;;
        --epic-id) EPIC_ID="$2"; shift 2 ;;
        --repo) REPO="$2"; shift 2 ;;
        *) shift ;;
    esac
done

[[ -z "$ISSUE_ID" || -z "$EPIC_ID" ]] && { echo "Usage: $0 --issue-id <issue> --epic-id <epic> [--repo <repo>]"; exit 1; }
PLATFORM=$(detect_platform)
[[ -z "$REPO" ]] && REPO=$(read_repository)

case "$PLATFORM" in
    github)
        gh issue edit "$ISSUE_ID" ${REPO:+--repo "$REPO"} --body "$(gh issue view "$ISSUE_ID" --json body -q .body 2>/dev/null || true)

Relates to #$EPIC_ID" 2>/dev/null
        echo "Linked GitHub issue #$ISSUE_ID to #$EPIC_ID"
        ;;
    gitlab)
        # GitLab: link issue to epic via API
        # API: POST /groups/:group/epics/:epic_iid/issues/:issue_iid
        # Derive group from repository path: "group/subgroup/project" -> "group/subgroup"
        if [[ -n "$REPO" ]]; then
            group_path=$(echo "$REPO" | rev | cut -d'/' -f2- | rev)
            if [[ -n "$group_path" ]]; then
                glab api --method POST "groups/$group_path/epics/$EPIC_ID/issues/$ISSUE_ID" 2>/dev/null && {
                    echo "Linked GitLab issue #$ISSUE_ID to epic #$EPIC_ID"
                    exit 0
                }
            fi
        fi
        # Fallback: add a note referencing the epic
        glab issue note "$ISSUE_ID" ${REPO:+--repo "$REPO"} --message "Part of epic #$EPIC_ID" 2>/dev/null || true
        echo "Linked GitLab issue #$ISSUE_ID to epic #$EPIC_ID"
        ;;
    *)
        echo "Linked local issue #$ISSUE_ID to epic #$EPIC_ID (no platform)"
        ;;
esac
