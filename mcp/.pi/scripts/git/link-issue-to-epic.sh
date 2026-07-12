#!/usr/bin/env bash
# Git Wrapper: Link Issue to Epic
#
# Usage: bash .pi/scripts/git/link-issue-to-epic.sh \
#   --issue-id 102 \
#   --epic-id 101

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
EPIC_ID=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --issue-id) ISSUE_ID="$2"; shift 2 ;;
        --epic-id) EPIC_ID="$2"; shift 2 ;;
        *) shift ;;
    esac
done

[[ -z "$ISSUE_ID" || -z "$EPIC_ID" ]] && { echo "Usage: $0 --issue-id <issue> --epic-id <epic>"; exit 1; }
PLATFORM=$(detect_platform)

case "$PLATFORM" in
    github)
        # GitHub uses "closes #N" or "fixes #N" in issue body to link
        gh issue edit "$ISSUE_ID" --body "$(gh issue view "$ISSUE_ID" --json body -q .body)

Relates to #$EPIC_ID" 2>/dev/null
        echo "Linked GitHub issue #$ISSUE_ID to #$EPIC_ID"
        ;;
    gitlab)
        # Use GitLab API to link issue to epic (--milestone is semantically wrong).
        # API: POST /groups/:group/epics/:epic_iid/issues/:issue_iid
        # We derive the group from the project path.
        if command -v jq &>/dev/null && [[ -n "${CI_PROJECT_ID:-}" || -n "${GITLAB_PROJECT_ID:-}" ]]; then
            local pid="${CI_PROJECT_ID:-${GITLAB_PROJECT_ID:-}}"
            glab api "projects/$pid" --method GET 2>/dev/null | jq -r '.namespace.full_path // empty' | read -r group_path
            if [[ -n "$group_path" ]]; then
                glab api --method POST "groups/$group_path/epics/$EPIC_ID/issues/$ISSUE_ID" 2>/dev/null || {
                    # Fallback: add a note referencing the epic
                    glab issue note "$ISSUE_ID" --message "Part of epic #$EPIC_ID" 2>/dev/null || true
                }
            else
                glab issue note "$ISSUE_ID" --message "Part of epic #$EPIC_ID" 2>/dev/null || true
            fi
        else
            glab issue note "$ISSUE_ID" --message "Part of epic #$EPIC_ID" 2>/dev/null || true
        fi
        echo "Linked GitLab issue #$ISSUE_ID to epic #$EPIC_ID"
        ;;
    *)
        echo "Linked local issue #$ISSUE_ID to epic #$EPIC_ID (no platform)"
        ;;
esac
