#!/usr/bin/env bash
# Git Wrapper: Create Tracking Issue
#
# Creates a tracking issue on GitHub or GitLab that tracks epic progress.
# Posts to the issue body as each step completes.
#
# Usage: bash .pi/scripts/git/create-tracking-issue.sh \
#   --title "Epic: Auth Module v2" \
#   --body "Tracking issue content" \
#   --labels "epic,tracking"
#
# Environment variables:
#   GITHUB_TOKEN / GITLAB_TOKEN — API token
#   GITHUB_REPO / GITLAB_PROJECT_ID — repo/project identifier
#   GIT_PLATFORM — "github" or "gitlab" (auto-detected)

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

# Extract trailing number from a URL or string (macOS-compatible, no -P flag)
extract_number() {
    echo "$1" | grep -o '[0-9][0-9]*$' || echo ""
}

TITLE=""
BODY=""
BODY_FILE=""
LABELS=""
MILESTONE=""
REPO=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --title) TITLE="$2"; shift 2 ;;
        --body) BODY="$2"; shift 2 ;;
        --body-file) BODY_FILE="$2"; shift 2 ;;
        --labels) LABELS="$2"; shift 2 ;;
        --milestone) MILESTONE="$2"; shift 2 ;;
        --repo) REPO="$2"; shift 2 ;;
        *) shift ;;
    esac
done

# If --body-file is provided, read body from file (stripping YAML frontmatter)
if [[ -n "$BODY_FILE" && -f "$BODY_FILE" ]]; then
    # Extract everything after the second --- (YAML frontmatter delimiter)
    BODY=$(awk '/^---$/{n++; next} n>=2{print}' "$BODY_FILE")
    # If stripping produced empty output, use the raw file as fallback
    if [[ -z "$BODY" ]]; then
        BODY=$(cat "$BODY_FILE")
    fi
fi

PLATFORM=$(detect_platform)

if [[ "$PLATFORM" == "none" ]]; then
    echo "No git platform detected. Set GITHUB_TOKEN or GITLAB_TOKEN, or install gh/glab CLI."
    echo "Creating local tracking file instead."
    mkdir -p .pi/.tracking
    TRACKING_FILE=".pi/.tracking/issue-$(date +%s).md"
    cat > "$TRACKING_FILE" << EOF
# $TITLE

$BODY

Labels: $LABELS
Milestone: $MILESTONE
Created: $(date -u +%Y-%m-%dT%H:%M:%SZ)
EOF
    echo "TRACKING_ID=local:$(basename "$TRACKING_FILE")"
    exit 0
fi

case "$PLATFORM" in
    github)
        # Build gh issue create command directly (no eval needed)
        ARGS=()
        [[ -n "$REPO" ]] && ARGS+=(--repo "$REPO")
        [[ -n "$TITLE" ]] && ARGS+=(--title "$TITLE")
        [[ -n "$MILESTONE" ]] && ARGS+=(--milestone "$MILESTONE")

        # Body: strip YAML frontmatter from --body-file, then write to temp file for gh
        if [[ -n "$BODY_FILE" && -f "$BODY_FILE" ]]; then
            STRIPPED_BODY=$(awk '/^---$/{n++; next} n>=2{print}' "$BODY_FILE")
            if [[ -n "$STRIPPED_BODY" ]]; then
                TMP_BODY=$(mktemp /tmp/guardian-issue-XXXXXX.md)
                echo "$STRIPPED_BODY" > "$TMP_BODY"
                ARGS+=(--body-file "$TMP_BODY")
                trap "rm -f '$TMP_BODY'" EXIT
            else
                ARGS+=(--body-file "$BODY_FILE")
            fi
        elif [[ -n "$BODY" ]]; then
            ARGS+=(--body "$BODY")
        fi

        # Create issue (no labels — they may not exist on new repos)
        ISSUE_OUTPUT=$(gh issue create "${ARGS[@]}" 2>&1) || ISSUE_OUTPUT=""

        if [[ -z "$ISSUE_OUTPUT" ]] || [[ "$ISSUE_OUTPUT" == *"could not"* ]] || [[ "$ISSUE_OUTPUT" == *"GraphQL"* ]] || [[ "$ISSUE_OUTPUT" == *"must provide"* ]]; then
            echo "Failed to create issue: $ISSUE_OUTPUT" >&2
            echo "TRACKING_ID="
            exit 1
        fi

        ISSUE_NUMBER=$(extract_number "$ISSUE_OUTPUT")

        # Try to add labels separately (non-fatal)
        if [[ -n "$LABELS" && -n "$ISSUE_NUMBER" ]]; then
            gh issue edit "$ISSUE_NUMBER" --add-label "$LABELS" 2>/dev/null || true
        fi

        echo "TRACKING_ID=$ISSUE_NUMBER"
        echo "TRACKING_URL=$ISSUE_OUTPUT"
        ;;

    gitlab)
        ARGS=()
        [[ -n "$REPO" ]] && ARGS+=(--repo "$REPO")
        [[ -n "$TITLE" ]] && ARGS+=(--title "$TITLE")
        [[ -n "$MILESTONE" ]] && ARGS+=(--milestone "$MILESTONE")

        # Body: strip YAML frontmatter from --body-file, then write to temp file for glab
        if [[ -n "$BODY_FILE" && -f "$BODY_FILE" ]]; then
            STRIPPED_BODY=$(awk '/^---$/{n++; next} n>=2{print}' "$BODY_FILE")
            if [[ -n "$STRIPPED_BODY" ]]; then
                TMP_BODY=$(mktemp /tmp/guardian-issue-XXXXXX.md)
                echo "$STRIPPED_BODY" > "$TMP_BODY"
                ARGS+=(--description-file "$TMP_BODY")
                trap "rm -f '$TMP_BODY'" EXIT
            else
                ARGS+=(--description-file "$BODY_FILE")
            fi
        elif [[ -n "$BODY" ]]; then
            ARGS+=(--description "$BODY")
        fi

        ISSUE_OUTPUT=$(glab issue create "${ARGS[@]}" 2>&1) || ISSUE_OUTPUT=""

        if [[ -z "$ISSUE_OUTPUT" ]]; then
            echo "Failed to create issue" >&2
            echo "TRACKING_ID="
            exit 1
        fi

        ISSUE_NUMBER=$(extract_number "$ISSUE_OUTPUT")

        # Try to add labels separately (non-fatal)
        if [[ -n "$LABELS" && -n "$ISSUE_NUMBER" ]]; then
            glab issue update "$ISSUE_NUMBER" --label "$LABELS" 2>/dev/null || true
        fi

        echo "TRACKING_ID=$ISSUE_NUMBER"
        echo "TRACKING_URL=$ISSUE_OUTPUT"
        ;;
esac
