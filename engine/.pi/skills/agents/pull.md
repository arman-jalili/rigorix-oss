---
name: pull
description: Keep branch updated with latest origin/main before handoff.
---

# Pull Skill

Sync with latest origin/main before implementation or handoff.

## Protocol

1. **Fetch** latest from remote
2. **Merge** origin/main into current branch
3. **Record** the result in workpad or notes
4. **Rerun** validations if conflicts were resolved

## Commands

```bash
# Fetch latest
git fetch origin

# Check what would change
git log HEAD..origin/main --oneline

# Merge into current branch
git merge origin/main --no-edit

# Record evidence
# merge source: origin/main
# result: clean / conflicts resolved
# HEAD: <short-sha>
```

## When to Run

- Before starting any implementation
- Before pushing changes
- After being asked to rebase
- When reviewer comments suggest the branch is out of date
