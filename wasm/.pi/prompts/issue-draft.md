# Issue Draft Workflow

**Purpose:** Issue-creator agent reads the approved epic and creates draft GitHub/GitLab issues for review before publishing.

---

## Prerequisites

- Epic proposal approved by all validators (from `/epic-plan`)
- Epic name and issue breakdown defined

---

## Workflow Steps

### 1. Read Epic Proposal

Load the approved epic proposal from previous workflow:

```bash
# Check for epic proposal file (if saved)
cat .pi/context/epic-proposal.md  # if exists
```

Or use the epic details from the `/epic-plan` output.

### 2. Create Issue Drafts

For each issue in the epic breakdown, create a detailed draft:

**Issue Template:**

```markdown
## Issue: [ISSUE_TITLE]

### Epic: [EPIC_NAME]

### Type: [feature/bug/refactor/docs]

### Priority: [high/medium/low]

### Description
[Clear description of what needs to be done]

### Acceptance Criteria
- [ ] [Criterion 1 - specific and testable]
- [ ] [Criterion 2 - specific and testable]
- [ ] [Criterion 3 - specific and testable]

### Implementation Notes
- [Technical approach hints]
- [Files likely affected]
- [Patterns to follow from .pi/context/patterns.md]

### Dependencies
- [Depends on issue #X]
- [Blocks issue #Y]

### Estimated Scope
- Files: [X]
- Lines: [Y]
- Validator Scope: [simple/moderate/complex]

### Testing Requirements
- [Unit tests required for X]
- [Integration tests required for Y]

### Documentation Updates
- [API docs for X]
- [README section for Y]
```

### 3. Epic Draft (GitHub/GitLab Milestone)

Create the epic/milestone draft:

**Epic Template (GitHub):**
```markdown
## Epic: [EPIC_NAME]

### Milestone Title: [EPIC_NAME]

### Description
[Summary from epic proposal]

### Goals
- [Goal 1]
- [Goal 2]
- [Goal 3]

### Issues Included
1. #[issue_number] - [Issue title]
2. #[issue_number] - [Issue title]
3. #[issue_number] - [Issue title]

### Tracking Issue
[Reference to tracking issue #X]

### Timeline
- Start: [date]
- Target Completion: [date]

### Success Metrics
- [Metric 1]
- [Metric 2]
```

**Epic Template (GitLab):**
```markdown
## Epic: [EPIC_NAME]

### Labels: [epic, scope:X]

### Description
[Summary from epic proposal]

### Child Issues
- #[issue_number] - [Issue title]
- #[issue_number] - [Issue title]

### Related Epics
- #[epic_number] - [Related epic]

### Milestone
[Milestone name]
```

### 4. Tracking Issue Draft

Create a tracking issue that monitors epic progress:

**Tracking Issue Template:**

```markdown
## Tracking: [EPIC_NAME]

### Type: tracking

### Purpose
This issue tracks the overall progress of the [EPIC_NAME] epic.

### Issues Checklist
- [ ] #[issue_1] - [title] - Status: [open/in-progress/review/merged]
- [ ] #[issue_2] - [title] - Status: [open/in-progress/review/merged]
- [ ] #[issue_3] - [title] - Status: [open/in-progress/review/merged]

### Progress
- Total Issues: [N]
- Completed: 0/N (0%)
- In Progress: 0/N (0%)

### Dependencies
- [External dependency 1]
- [External dependency 2]

### Timeline
- Start: [date]
- Current: [date]
- Target: [date]

### Notes
- [Any important notes about epic execution]
```

### 5. Review Drafts

Before creating in git, review all drafts:

**Review Checklist:**
- [ ] All issues have clear acceptance criteria
- [ ] Dependencies correctly mapped
- [ ] Scope estimates reasonable
- [ ] Testing requirements specified
- [ ] Documentation updates noted
- [ ] Tracking issue includes all issues

---

## Output Format

Save drafts for review:

```
.pi/context/issue-drafts/
├── epic-draft.md
├── tracking-issue-draft.md
├── issue-1-draft.md
├── issue-2-draft.md
├── issue-3-draft.md
└── review-checklist.md
```

---

## Git Repository Tool

Using `gh`:

| Tool | Command Preview |
|------|-----------------|
| **gh** | `gh issue create --title "[TITLE]" --body "[BODY]" --label "[LABELS]"` |
| **glab** | `glab issue create --title "[TITLE]" --description "[BODY]" --label "[LABELS]"` |

---

## Acceptance Criteria

- [ ] All issue drafts created with full details
- [ ] Epic/milestone draft created
- [ ] Tracking issue draft created
- [ ] All drafts reviewed and approved
- [ ] Ready for `/git-issues`

---

## Next Workflow

After draft approval, run: `/git-issues` to create in repository.