# Git Issues Workflow

**Purpose:** Create epics/milestones and all issues in GitHub or GitLab, including the tracking issue.

---

## Prerequisites

- Issue drafts approved (from `/issue-draft`)
- Git repository tool configured (`gh`)
- Repository remote properly configured

---

## Workflow Steps

### 1. Verify Repository Access

Check that the repository tool is configured and authenticated:

```bash
# For GitHub (gh)
gh auth status
gh repo view arman-jalili/rigorix-oss

# For GitLab (glab)
glab auth status
glab repo view arman-jalili/rigorix-oss
```

### 2. Create Epic/Milestone

**For GitHub (using gh):**

```bash
# Create milestone
gh api repos/arman-jalili/rigorix-oss/milestones \
  -f title="[EPIC_NAME]" \
  -f description="[EPIC_DESCRIPTION]" \
  -f state="open"

# Get milestone number
gh api repos/arman-jalili/rigorix-oss/milestones --jq '.[] | select(.title=="[EPIC_NAME]") | .number'
```

**For GitLab (using glab):**

```bash
# Create epic (GitLab Premium/Ultimate)
glab api projects/:id/epics \
  -f title="[EPIC_NAME]" \
  -f description="[EPIC_DESCRIPTION]" \
  -f labels="epic"

# Or create milestone for GitLab Free
glab api projects/:id/milestones \
  -f title="[EPIC_NAME]" \
  -f description="[EPIC_DESCRIPTION]"
```

### 3. Create Individual Issues

**For GitHub (gh):**

```bash
gh issue create \
  --title "[ISSUE_TITLE]" \
  --body "[ISSUE_BODY_FROM_DRAFT]" \
  --label "type:[feature/bug/refactor],scope:[simple/moderate/complex]" \
  --milestone "[MILESTONE_NUMBER]"
```

**For GitLab (glab):**

```bash
glab issue create \
  --title "[ISSUE_TITLE]" \
  --description "[ISSUE_BODY_FROM_DRAFT]" \
  --label "type:[feature/bug/refactor],scope:[simple/moderate/complex]" \
  --milestone "[MILESTONE_ID]"
```

### 4. Record Issue Numbers

After creating each issue, record the issue number for the tracking issue:

```markdown
### Issue Numbers Created
- Issue 1: #[number]
- Issue 2: #[number]
- Issue 3: #[number]
- Milestone/Epic: #[number]
```

### 5. Create Tracking Issue

Create the tracking issue with links to all created issues:

**GitHub Tracking Issue:**

```bash
gh issue create \
  --title "Tracking: [EPIC_NAME]" \
  --body "$(cat <<'EOF'
## Epic Progress Tracking

### Milestone: #[milestone_number]

### Issues Checklist
- [ ] #[issue_1] - [Issue title]
- [ ] #[issue_2] - [Issue title]
- [ ] #[issue_3] - [Issue title]

### Progress
- Total Issues: [N]
- Completed: 0/[N] (0%)
- In Progress: 0/[N] (0%)

### Timeline
- Start: [date]
- Target: [date]

---
*This issue will be updated as epic progresses*
EOF
)" \
  --label "tracking,epic"
```

**GitLab Tracking Issue:**

```bash
glab issue create \
  --title "Tracking: [EPIC_NAME]" \
  --description "$(cat <<'EOF'
## Epic Progress Tracking

### Epic: #[epic_number]

### Child Issues Checklist
- [ ] #[issue_1] - [Issue title]
- [ ] #[issue_2] - [Issue title]
- [ ] #[issue_3] - [Issue title]

### Progress
- Total Issues: [N]
- Completed: 0/[N] (0%)

---
*This issue will be updated as epic progresses*
EOF
)" \
  --label "tracking,epic"
```

### 6. Link Issues to Epic/Milestone

**For GitHub:**
```bash
# Issues automatically linked to milestone via --milestone flag
# Verify linking
gh issue view [issue_number] --json milestone
```

**For GitLab:**
```bash
# Link issues to epic (Premium/Ultimate)
glab api projects/:id/issues/[issue_iid] \
  -f epic_id="[epic_id]"
```

### 7. Verify Creation

Verify all issues and tracking are created correctly:

```bash
# For GitHub
gh issue list --milestone "[MILESTONE_TITLE]" --limit 50

# For GitLab
glab issue list --milestone "[MILESTONE_TITLE]"
```

---

## Output Summary

After completion, provide:

```markdown
## Git Issues Created

### Repository: arman-jalili/rigorix-oss
### Tool: gh

### Epic/Milestone
- Title: [EPIC_NAME]
- Number/ID: #[number]

### Issues Created
| # | Title | Scope | Status |
|---|-------|-------|--------|
| #[n] | [title] | [scope] | open |
| #[n] | [title] | [scope] | open |
| #[n] | [title] | [scope] | open |

### Tracking Issue
- Number: #[number]
- Title: Tracking: [EPIC_NAME]

### Next Steps
1. Start implementation on first issue
2. Use `/issue-closeout` when issue complete
3. Update tracking issue after each merge
```

---

## Error Handling

| Error | Solution |
|-------|----------|
| Authentication failed | Run `gh auth login` or `glab auth login` |
| Repository not found | Check `arman-jalili/rigorix-oss` format (owner/repo) |
| Milestone creation failed | Use project ID instead of name |
| Rate limited | Wait and retry, or use API token |

---

## Acceptance Criteria

- [ ] Milestone/Epic created successfully
- [ ] All individual issues created
- [ ] Tracking issue created
- [ ] Issues linked to milestone/epic
- [ ] Issue numbers recorded
- [ ] Verification confirms all issues visible

---

## Next Workflow

When ready to close out an issue after implementation, run: `/issue-closeout`