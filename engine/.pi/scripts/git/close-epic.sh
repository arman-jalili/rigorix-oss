#!/usr/bin/env bash
# Git Wrapper: Close Epic
#
# Closes the epic and all linked issues when the epic is complete.
#
# Usage: bash .pi/scripts/git/close-epic.sh \
#   --epic-id 101 \
#   --tracking-id 100 \
#   --comment "Epic complete. All 5 issues done." \
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

EPIC_ID=""
TRACKING_ID=""
COMMENT="Epic complete."
REPO=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --epic-id) EPIC_ID="$2"; shift 2 ;;
        --tracking-id) TRACKING_ID="$2"; shift 2 ;;
        --comment) COMMENT="$2"; shift 2 ;;
        --repo) REPO="$2"; shift 2 ;;
        *) shift ;;
    esac
done

PLATFORM=$(detect_platform)
[[ -z "$REPO" ]] && REPO=$(read_repository)

if [[ -z "$EPIC_ID" ]]; then
    echo "Usage: $0 --epic-id <epic_number> [--tracking-id <tracking_id>] [--comment <comment>] [--repo <repo>]"
    exit 1
fi

# Close tracking issue first
if [[ -n "$TRACKING_ID" ]]; then
    bash "$(dirname "$0")/close-issue.sh" --id "$TRACKING_ID" ${REPO:+--repo "$REPO"}
fi

# Close the epic
case "$PLATFORM" in
    github)
        gh issue close "$EPIC_ID" ${REPO:+--repo "$REPO"} 2>/dev/null
        if [[ -n "$COMMENT" ]]; then
            gh issue comment "$EPIC_ID" ${REPO:+--repo "$REPO"} --body "$COMMENT" 2>/dev/null
        fi
        echo "Closed GitHub epic #$EPIC_ID"
        ;;
    gitlab)
        # If EPIC_ID looks numeric, use the GitLab Epic API
        if [[ "$EPIC_ID" =~ ^[0-9]+$ ]]; then
            # Try epic close first, fallback to issue close
            glab api --method PUT "epics/$EPIC_ID" --body '{"state_event":"close"}' 2>/dev/null && \
                echo "Closed GitLab epic #$EPIC_ID" || \
                glab issue update "$EPIC_ID" --state-event close ${REPO:+--repo "$REPO"} 2>/dev/null
        else
            glab issue update "$EPIC_ID" --state-event close ${REPO:+--repo "$REPO"} 2>/dev/null
        fi
        if [[ -n "$COMMENT" ]]; then
            glab issue note "$EPIC_ID" ${REPO:+--repo "$REPO"} --message "$COMMENT" 2>/dev/null || true
        fi
        echo "Closed GitLab epic #$EPIC_ID"
        ;;
    *)
        echo "Local epic #$EPIC_ID closed (no platform)"
        ;;
esac
