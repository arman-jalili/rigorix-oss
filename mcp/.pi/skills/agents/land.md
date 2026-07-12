---
name: land
description: Execute the PR merge loop with full validation before merging.
---

# Land Skill

When a ticket reaches `Merging` state, execute the land loop: validate, merge, and move to Done.

**NEVER call `gh pr merge` or `glab mr merge` directly.** Use this skill.

## Protocol

1. **Verify PR is approved** — check review status
2. **Verify all checks pass** — CI, lint, tests
3. **Verify workpad is complete** — all acceptance criteria checked
4. **Verify no outstanding review comments** — run feedback sweep
5. **Merge** the PR using the configured repository tool
6. **Delete** the feature branch (remote and local)
7. **Move ticket** to Done state
8. **Update tracking** — update changelog if needed

## GitHub

```bash
# Check PR status
gh pr view <number> --json state,mergeStateStatus,reviewDecision,statusCheckRollup

# Verify checks pass
gh pr checks <number>

# Verify reviews
gh pr view <number> --json reviews

# Merge (squash for clean history, or merge for preserving commits)
gh pr merge <number> --squash --delete-branch

# Or rebase merge
gh pr merge <number> --rebase --delete-branch
```

## GitLab

```bash
# Check MR status
glab mr view <iid>

# Merge MR
glab mr merge <iid> --when-pipeline-succeeds --should-remove-source-branch
```

## Rules

- NEVER merge if any check is failing
- NEVER merge if review is not approved
- NEVER merge if acceptance criteria are incomplete
- ALWAYS delete the feature branch after merge
- ALWAYS move the ticket to Done after merge
- If merge fails, document the failure in the workpad and notify
