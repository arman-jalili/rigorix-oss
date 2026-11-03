# Issue Merge Workflow

**Purpose:** Merge the MR after CI pipeline passes, close the issue, update the tracking issue, and close the epic if it's the last issue.

---

## Prerequisites

- Compliance MR created (from `/issue-closeout`)
- CI pipeline passed (green)
- MR approved (if required)
- Issue number and MR number known

---

## Workflow Steps

### 1. Verify CI Pipeline Status

Check that all CI checks have passed:

**For GitHub:**
```bash
gh pr checks [MR_NUMBER]

# Expected output:
# ✅ ci/build: passed
# ✅ ci/test: passed
# ✅ ci/lint: passed
# ✅ ci/security: passed
```

**For GitLab:**
```bash
glab mr view [MR_NUMBER]

# Check pipeline status
glab api projects/:id/merge_requests/[mr_iid] --jq '.head_pipeline.status'
# Expected: "success"
```

### 1b. PR Feedback Sweep (Required Before Merge)

Before merging, ensure ALL review comments are resolved:

1. **Gather feedback from all channels:**
   ```bash
   # Top-level PR comments
   gh pr view --comments
   # Inline review comments
   gh api repos/<owner>/<repo>/pulls/<number>/comments
   # Review summaries
   gh pr view --json reviews
   ```

2. **Every actionable reviewer comment (human or bot) is blocking until:**
   - Code/test/docs updated to address it, OR
   - Explicit, justified pushback reply posted on that thread

3. **Re-run validation after feedback-driven changes**

4. **Repeat until no outstanding actionable comments remain**

5. **Confirm PR checks are green after latest changes**

### 2. Merge the MR

**For GitHub (gh):**

```bash
# Merge with squash (recommended for clean history)
gh pr merge [MR_NUMBER] \
  --squash \
  --delete-branch \
  --subject "[Issue #X] Issue Title" \
  --body "Closes #[ISSUE_NUMBER]"

# Or merge with merge commit
gh pr merge [MR_NUMBER] \
  --merge \
  --delete-branch
```

**For GitLab (glab):**

```bash
# Merge with squash
glab mr merge [MR_NUMBER] \
  --squash \
  --squash-message "[Issue #X] Issue Title" \
  --remove-source-branch

# Or merge with merge commit
glab mr merge [MR_NUMBER] \
  --remove-source-branch
```

### 3. Verify Issue Auto-Close

The issue should auto-close due to "Closes #X" in MR body:

**For GitHub:**
```bash
gh issue view [ISSUE_NUMBER] --json state
# Expected: "closed"
```

**For GitLab:**
```bash
glab issue view [ISSUE_NUMBER]
# Expected: Status: closed
```

If not auto-closed, manually close:

```bash
gh issue close [ISSUE_NUMBER]
```

### 4. Add Completion Comment to Issue

Add evidence comment to the closed issue:

**Comment Template:**

```markdown
## Issue Completed ✅

### Merge Details
- MR: #[MR_NUMBER]
- Merged by: [user]
- Merged at: [timestamp]
- Merge method: [squash/merge]

### Validator Evidence
All validators passed before merge:
- ✅ CI: [link to pipeline]
- ✅ Tests: [X] tests passing, [Y]% coverage
- ✅ Security: No vulnerabilities
- ✅ Operations: Production requirements met

### Acceptance Criteria Met
All acceptance criteria verified:
- [x] [Criterion 1]
- [x] [Criterion 2]
- [x] [Criterion 3]

### Changes Deployed
- Commit: [commit_sha]
- Branch merged: [branch_name] → main

---
Issue completed successfully with full compliance verification.
```

**For GitHub:**
```bash
gh issue comment [ISSUE_NUMBER] --body "[COMMENT_BODY]"
```

**For GitLab:**
```bash
glab issue note [ISSUE_NUMBER] --message "[COMMENT_BODY]"
```

### 5. Update Tracking Issue

Update the tracking issue body with progress and comment:

**Get Tracking Issue:**
```bash
gh issue list --label tracking --search "[EPIC_NAME]"
```

**Update Tracking Issue Body:**

Update the main body with progress:

```markdown
## Epic Progress Tracking

### Milestone/Epic: #[epic_number]

### Issues Checklist
- [x] #[issue_1] - [title] - ✅ Completed (MR #[mr_number])
- [ ] #[issue_2] - [title] - Status: [open/in-progress]
- [ ] #[issue_3] - [title] - Status: [open/in-progress]

### Progress
- Total Issues: [N]
- Completed: 1/[N] (updated percentage)
- In Progress: [X]/[N]

### Dependencies Completed
- ✅ [Dependency 1] - Issue #[number] merged

### Timeline
- Start: [date]
- Current: [date]
- Target: [date]
- Last Update: [timestamp]

---
Updated after #[ISSUE_NUMBER] merge
```

**For GitHub:**
```bash
# GitHub doesn't support editing issue body via CLI directly
# Use API
gh api repos/arman-jalili/rigorix-oss/issues/[TRACKING_NUMBER] \
  -X PATCH \
  -f body="[UPDATED_BODY]"
```

**For GitLab:**
```bash
glab api projects/:id/issues/[tracking_iid] \
  -X PUT \
  -f description="[UPDATED_BODY]"
```

**Add Progress Comment:**

```bash
gh issue comment [TRACKING_NUMBER] --body "
## Issue #[ISSUE_NUMBER] Completed ✅

- Title: [Issue title]
- MR: #[MR_NUMBER]
- Merged: [timestamp]
- Validators: All passed

Progress: 1/[N] issues completed
"
```

### 6. Check If Epic Complete

Check if this was the last issue in the epic:

```bash
gh issue list --milestone "[EPIC_NAME]" --state open
```

**If no remaining open issues:**

#### Close the Milestone/Epic

**For GitHub:**
```bash
gh api repos/arman-jalili/rigorix-oss/milestones/[MILESTONE_NUMBER] \
  -X PATCH \
  -f state="closed"
```

**For GitLab:**
```bash
glab api projects/:id/milestones/[milestone_id] \
  -X PUT \
  -f state_event="close"

# For epics (Premium/Ultimate)
glab api projects/:id/epics/[epic_id] \
  -X PUT \
  -f state_event="close"
```

#### Update Tracking Issue for Epic Completion

```markdown
## Epic Completed ✅

### Summary
All [N] issues completed successfully.

### Issues Completed
| # | Title | MR | Merged |
|---|-------|----|----|
| #[n1] | [title] | #[mr1] | [date] |
| #[n2] | [title] | #[mr2] | [date] |
| #[n3] | [title] | #[mr3] | [date] |

### Epic Statistics
- Total Issues: [N]
- Completed: [N]/[N] (100%)
- Total MRs: [N]
- Duration: [start] to [end]

### Validators Summary
All issues passed required validators before merge.

---
Epic completed successfully. Tracking issue closed.
```

#### Close Tracking Issue

```bash
gh issue close [TRACKING_NUMBER]
gh issue comment [TRACKING_NUMBER] --body "
## Epic Complete ✅

All [N] issues merged successfully.
Epic [EPIC_NAME] closed.

Duration: [X days]
Success rate: 100%

---
Epic tracking complete.
"
```

---

## Output Summary

After completion, provide:

```markdown
## Issue Merge Complete

### Issue #[ISSUE_NUMBER]
- Status: ✅ Closed
- MR #[MR_NUMBER]: Merged
- Merge Method: [squash/merge]

### Tracking Issue #[TRACKING_NUMBER]
- Status: Updated
- Progress: [X]/[N] completed

### Epic Status
- Remaining Issues: [N-X]
- Epic Status: [open/closed]

### Next Action
- [If remaining issues]: Proceed to next issue implementation
- [If epic complete]: Ready for next `/epic-plan`
```

---

## Error Handling

| Error | Solution |
|-------|----------|
| CI pipeline failed | Fix issues, re-push, wait for green |
| MR not approved | Request review, wait for approval |
| Merge conflict | Rebase onto main, resolve conflicts |
| Issue not auto-closed | Manually close with comment |
| Tracking update failed | Retry with correct issue number |

---

## Acceptance Criteria

- [ ] CI pipeline verified green
- [ ] MR merged successfully
- [ ] Branch deleted
- [ ] Issue closed with evidence comment
- [ ] Tracking issue body updated
- [ ] Tracking issue progress comment added
- [ ] Epic/milestone closed if last issue
- [ ] Tracking issue closed if epic complete

---

## Next Workflow

- If more issues in epic: Implement next issue, then `/issue-closeout`
- If epic complete: Run `/epic-plan` for next epic