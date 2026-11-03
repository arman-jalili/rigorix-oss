---
name: push
description: Keep remote branch current and publish updates.
---

# Push Skill

Publish local changes to remote and keep branch updated.

## Protocol

1. **Before push:** run all validations for the scope
2. **Merge latest origin/main** into the branch and resolve conflicts
3. **Rerun validations** after merge
4. **Push** to remote

## Commands

```bash
# Fetch latest
git fetch origin

# Merge main into current branch
git merge origin/main

# If conflicts: resolve, then
git add <resolved-files>
git commit -m "fix: resolve merge conflicts"

# Push branch (create if needed)
git push -u origin <branch-name>

# Force push only with --force-with-lease (never --force)
git push --force-with-lease
```

## Rules

- ALWAYS merge origin/main before pushing
- NEVER use `git push --force` — use `--force-with-lease` only when necessary
- ALWAYS re-run validations after resolving conflicts
