#!/usr/bin/env bash
# ============================================================================
# create-feature-branch.sh — Create Branch for Issue Group
#
# Run as: bash .pi/scripts/create-feature-branch.sh <branch-name> [base-branch]
# ============================================================================
set -euo pipefail

BRANCH_NAME="${1:-}"
BASE_BRANCH="${2:-main}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo "============================================"
echo "  Create Feature Branch"
echo "============================================"
echo ""

# ---------------------------------------------------------------------------
# Validate Input
# ---------------------------------------------------------------------------
if [ -z "$BRANCH_NAME" ]; then
    echo -e "${RED}❌ Branch name required${NC}"
    echo "Usage: create-feature-branch.sh <branch-name> [base-branch]"
    echo ""
    echo "Branch naming conventions:"
    echo "  feature/{component}-issues    — Component batch"
    echo "  priority/{level}-{date}       — Priority batch"
    echo "  issue/{number}                — Single issue"
    exit 1
fi

# ---------------------------------------------------------------------------
# Check Git State
# ---------------------------------------------------------------------------
if ! git rev-parse --is-inside-work-tree &> /dev/null; then
    echo -e "${RED}❌ Not in a git repository${NC}"
    exit 1
fi

# Check for uncommitted changes
CHANGES=$(git status --porcelain | wc -l | tr -d ' ')
if [ "$CHANGES" -gt 0 ]; then
    echo -e "${YELLOW}⚠️  Uncommitted changes detected${NC}"
    git status --short
    echo ""
    echo "Options:"
    echo "  1. Commit changes first"
    echo "  2. Stash: git stash"
    echo "  3. Force (not recommended): git checkout -f"
    exit 1
fi

# ---------------------------------------------------------------------------
# Fetch Latest
# ---------------------------------------------------------------------------
echo "Fetching latest from origin..."
git fetch origin "$BASE_BRANCH" 2>/dev/null || true

# ---------------------------------------------------------------------------
# Create Branch
# ---------------------------------------------------------------------------
echo "Creating branch: $BRANCH_NAME"
echo "Base: $BASE_BRANCH"

# Check if branch exists
if git show-ref --verify --quiet refs/heads/"$BRANCH_NAME"; then
    echo -e "${YELLOW}⚠️  Branch already exists${NC}"
    echo "Options:"
    echo "  1. Use existing: git checkout $BRANCH_NAME"
    echo "  2. Delete and recreate: git branch -D $BRANCH_NAME"
    exit 1
fi

# Check if remote branch exists
if git show-ref --verify --quiet refs/remotes/origin/"$BRANCH_NAME"; then
    echo -e "${YELLOW}⚠️  Remote branch exists${NC}"
    echo "  Use: git checkout -b $BRANCH_NAME origin/$BRANCH_NAME"
    exit 1
fi

# Create branch
git checkout -b "$BRANCH_NAME" "origin/$BASE_BRANCH" 2>/dev/null || git checkout -b "$BRANCH_NAME" "$BASE_BRANCH"

echo -e "${GREEN}✅ Branch created: $BRANCH_NAME${NC}"
echo ""
echo "Current state:"
git log --oneline -3
echo ""

# ---------------------------------------------------------------------------
# Track in Plan File
# ---------------------------------------------------------------------------
PLAN_FILE=".claude/plans/$BRANCH_NAME.md"
mkdir -p ".claude/plans"

cat > "$PLAN_FILE" << EOF
# Implementation Plan: $BRANCH_NAME

**Branch:** $BRANCH_NAME
**Base:** $BASE_BRANCH
**Created:** $(date)

## Status

- [ ] Issues identified
- [ ] Implementation plan created
- [ ] Code implemented
- [ ] Tests passing
- [ ] MR created
- [ ] Pipeline green
- [ ] Merged

## Issues

<!-- Add issue numbers and descriptions -->

## Implementation Steps

<!-- Auto-generated from issue analysis -->

## Validation Checklist

- [ ] cargo build
- [ ] cargo test --all
- [ ] cargo clippy -- -D warnings
- [ ] cargo fmt --check
- [ ] cargo audit
EOF

echo "Plan file created: $PLAN_FILE"

exit 0