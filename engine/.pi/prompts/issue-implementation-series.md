# Issue Implementation Series Workflow

## Overview
Workflow for implementing a series of existing GitHub issues with categorization, batching, and pipeline validation.

## Prerequisites
- GitHub CLI (`gh`) installed and authenticated
- Access to repository issues

## Phases

### Phase 1: Fetch Issues
**Script:** `.pi/scripts/fetch-issues.sh` (or `.pi/scripts/fetch-issues.sh`)

```bash
# Fetch open issues sorted by priority
gh issue list --state open --limit 50 --json number,title,labels,body
```

**Output:** Issues list in `.claude/plans/issues-fetched.json`

### Phase 2: Categorize Issues
**Agent:** `@architecture-coordinator`

1. Analyze fetched issues
2. Group by:
   - **Component:** Same module/files affected
   - **Priority:** Critical > High > Medium > Low
   - **Dependency:** Blocking/Blocked relationships
3. Create batch groups for feature branches

**Output:** `.claude/plans/issue-groups.md`

### Grouping Rules

| Group Type | Criteria | Branch Naming |
|------------|----------|---------------|
| Component batch | Same module (2-5 issues) | `feature/{component}-{issue-range}` |
| Priority batch | All Critical/High | `priority/critical-{date}` |
| Related batch | Dependencies linked | `feature/{feature-name}-issues` |
| Single | No grouping possible | `issue/{issue-number}` |

### Phase 3: Plan Batch Implementation
**Agent:** `@architecture-coordinator`

For each group:
1. Read all issue details
2. Create combined plan in `.claude/plans/{branch-name}.md`
3. Determine implementation order (dependencies first)
4. Define validation requirements per issue

### Phase 4: Create Feature Branch
**Script:** `.pi/scripts/create-feature-branch.sh`

```bash
# Create branch from group name
git checkout -b {branch-name}
```

### Phase 5: Implement Issues (Sequential)
**Agent:** `@code-developer`

For each issue in batch order:
1. Read issue spec from plan
2. Implement changes
3. Run quick validation:
   ```bash
   cargo build && cargo test --lib
   ```
4. Commit with reference:
   ```bash
   git commit -m "feat: implement #123 - {description}"
   ```
5. Mark issue in progress: `gh issue comment {num} --body "Implementing in {branch}"`

### Phase 6: Pre-MR Validation
**Scripts:** Run automated validators

```bash
# Full validation before MR
.pi/scripts/validate-ci.sh
.pi/scripts/validate-tests.sh
.pi/scripts/validate-security.sh
.pi/scripts/validate-operations.sh
```

**Gate:** All scripts must PASS

### Phase 7: Create MR/PR
**Script:** `.pi/scripts/create-mr.sh`

```bash
# Push and create PR
git push -u origin {branch-name}

gh pr create \
  --title "feat: implement issues {range}" \
  --body "$(cat .claude/plans/{branch-name}-pr-body.md)" \
  --base main
```

### Phase 8: MR Validation Workflow
**Agent:** `@architecture-coordinator`

Run MR validation sequence:

1. **Automated CI:** Wait for GitHub Actions to complete
2. **Architecture check:** Run architecture-validator
3. **Security check:** Run security-validator
4. **Integration check:** Run integration-validator

**Gate:** All checks green

### Phase 9: Address MR Feedback
If checks fail:
1. Read failure logs
2. Fix issues (not bypass)
3. Push fixes
4. Re-run validation

**Max retries:** 3

### Phase 10: Merge
**Script:** `.pi/scripts/merge-mr.sh`

```bash
# Merge when green
gh pr merge --squash --delete-branch

# Close implemented issues
for issue in {issue-list}; do
  gh issue close $issue --comment "Implemented in #{pr-number}"
done
```

### Phase 11: Next Batch
Return to Phase 3 for next group.

---

## Scripts Required

| Script | Purpose |
|--------|---------|
| `fetch-issues.sh` | Fetch open issues from GitHub |
| `categorize-issues.sh` | Group issues by component/priority |
| `create-feature-branch.sh` | Create branch from group |
| `create-mr.sh` | Create PR with body template |
| `merge-mr.sh` | Merge PR and close issues |
| `mr-validation.sh` | Run all MR checks |

## Issue Status Tracking

| State | Action |
|-------|--------|
| Open | Fetched, waiting implementation |
| In Progress | Comment added, branch created |
| Implemented | Code done, PR created |
| Closed | Merged, issue resolved |

## Deterministic Checks (Pipeline Must Be Green)

```bash
# Before MR
cargo build
cargo test --all
cargo clippy -- -D warnings
cargo fmt --check
cargo audit

# MR pipeline
.ci/pipeline.sh  # or equivalent
```