<!--
Canonical Reference: .pi/prompts/plan-to-issues.md
Generated: 2026-06-16T04:28:47.990Z
DO NOT EDIT DIRECTLY - Modify source in .pi/
-->

# Plan to Issues Workflow

**Purpose:** Read a superpowers plan file and convert it to GitHub/GitLab issues with epics and tracking.

---

## Prerequisites

- Superpowers plan file exists in `docs/superpowers/plans/*.md`
- Git repository tool configured (`gh`)
- Repository remote properly configured

---

## Input Format (Superpowers Plan)

Superpowers plans follow this structure:

```markdown
# [Plan Title]

**Date:** YYYY-MM-DD
**Status:** draft/approved/in-progress

---

## Milestone 1: [Milestone Name] (CRITICAL PATH or priority)

### Task 1: [Task Title]

- [ ] **Step 1: [Step description]**
  ```rust
  [code snippet if applicable]
  ```
  - File: `[filename]`

- [ ] **Step 2: [Step description]**
  - Commit message: `[message]`

### Task 2: [Task Title]
...

## Milestone 2: [Milestone Name]
...
```

---

## Workflow Steps

### 1. Locate Plan File

Find the superpowers plan file:

```bash
# List available plan files
ls -la docs/superpowers/plans/

# Or search for specific plan
find . -path "*/superpowers/plans/*.md" -type f
```

Prompt user to select plan if multiple exist.

### 2. Parse Plan Structure

Parse the plan to extract:

**Milestones → Epics:**
- Milestone title → Epic name
- Milestone priority (CRITICAL PATH, HIGH, MEDIUM) → Epic priority
- Tasks count → Scope estimation

**Tasks → Issues:**
- Task title → Issue title
- Task number → Issue ordering
- Steps → Acceptance criteria

**Steps → Checklist Items:**
- Step description → Acceptance criterion
- Step checkbox (- [ ]) → Checklist item
- Code snippets → Implementation hints
- File references → Files affected

### 3. Parse Algorithm

```bash
# Read plan file
PLAN_FILE="docs/superpowers/plans/YYYY-MM-DD-plan-name.md"
cat "$PLAN_FILE"
```

**Parsing Logic:**

1. Find all `## Milestone N:` lines → extract milestone name
2. Within each milestone, find all `### Task N:` lines → extract task title
3. Within each task, find all `- [ ] **Step N:**` lines → extract step description
4. Capture code blocks between triple-backticks as implementation hints
5. Capture file references as affected files

### 4. Determine Scope Classification

For each task, estimate scope based on:

| Factor | Simple | Moderate | Complex | Critical |
|--------|--------|----------|---------|----------|
| **Steps count** | 1-3 | 4-7 | 8-15 | 15+ |
| **Code snippets** | 0-1 | 2-3 | 4-8 | 8+ |
| **Files mentioned** | 1 | 2-3 | 4-8 | 8+ |
| **New dependencies** | No | Maybe | Yes | Multiple |

**Validators Required:**
- Simple: CI + canonical
- Moderate: CI + architecture + canonical
- Complex: CI + architecture + security + canonical
- Critical: All validators + human approval

### 5. Create Issue Drafts Directory

```bash
mkdir -p .pi/context/issue-drafts/
```

### 6. Generate Epic Drafts

For each milestone, create an epic draft:

**Epic Template:**

```markdown
## Epic: [MILESTONE_NAME]

### Source Milestone: Milestone N from [PLAN_FILE]

### Priority: [CRITICAL PATH / HIGH / MEDIUM / LOW]

### Description
[Extract any milestone-level description from plan]

### Goals
- Goal 1: Complete all tasks in milestone
- Goal 2: [Specific goal from plan context]

### Issues Included (Tasks)
| Task | Title | Scope | Validators |
|------|-------|-------|------------|
| Task 1 | [title] | [scope] | [validators] |
| Task 2 | [title] | [scope] | [validators] |

### Dependency Order
[List tasks in order with dependencies noted]

### Estimated Timeline
- Start: [today's date]
- Target: [estimate based on complexity]
```

### 7. Generate Issue Drafts

For each task, create an issue draft:

**Issue Template:**

```markdown
## Issue: [TASK_TITLE]

### Epic: [MILESTONE_NAME]

### Source: Task N from [PLAN_FILE]

### Type: feature

### Priority: [from milestone priority]

### Description
[Task description from plan, or derived from steps]

### Acceptance Criteria (from Steps)
- [ ] **Step 1:** [Step description from plan]
- [ ] **Step 2:** [Step description from plan]
- [ ] **Step 3:** [Step description from plan]
...

### Implementation Notes
[Code snippets from steps, formatted as hints]

**Example from Step 1:**
```rust
[Code snippet from plan]
```

**Files Affected:**
- [File 1 from step references]
- [File 2 from step references]

### Dependencies
- [Depends on Task X (if sequential)]
- [Blocks Task Y (if sequential)]

### Estimated Scope
- Files: [count from file references]
- Lines: [estimate from code snippets]
- Validator Scope: [simple/moderate/complex]

### Testing Requirements
- [ ] Unit tests for [functionality]
- [ ] Integration tests for [feature]

### Documentation Updates
- [ ] Update [file] for [feature]
- [ ] Add [doc] for [API/component]
```

### 8. Generate Tracking Issue Draft

Create a tracking issue that monitors all epics:

**Tracking Issue Template:**

```markdown
## Tracking: [PLAN_NAME] Implementation

### Source Plan: [PLAN_FILE]

### Plan Date: [date from plan]

### Status: [draft/approved/in-progress]

### Epics Checklist (Milestones)
- [ ] Epic 1: [Milestone 1 name] - [N] tasks
- [ ] Epic 2: [Milestone 2 name] - [N] tasks
- [ ] Epic 3: [Milestone 3 name] - [N] tasks

### Issues Checklist (All Tasks)
**Epic 1: [Milestone 1]**
- [ ] Task 1: [title] - Status: pending
- [ ] Task 2: [title] - Status: pending

**Epic 2: [Milestone 2]**
- [ ] Task 1: [title] - Status: pending
- [ ] Task 2: [title] - Status: pending

### Overall Progress
- Total Epics: [N]
- Total Issues: [N]
- Completed: 0/[N] (0%)
- In Progress: 0/[N] (0%)

### Critical Path
[List milestones marked CRITICAL PATH]

### Timeline
- Plan Date: [date]
- Started: [today or pending]
- Target Completion: [estimate]

### Notes
[Any plan-level notes or context]
```

### 9. Save All Drafts

```bash
# Save to drafts directory
cat > .pi/context/issue-drafts/epic-1-draft.md << 'EOF'
[Epic 1 content]
EOF

cat > .pi/context/issue-drafts/issue-1-1-draft.md << 'EOF'
[Issue 1 of Epic 1 content]
EOF

cat > .pi/context/issue-drafts/tracking-issue-draft.md << 'EOF'
[Tracking issue content]
EOF

# Create summary
cat > .pi/context/issue-drafts/summary.md << 'EOF'
## Plan-to-Issues Conversion Summary

### Source Plan: [PLAN_FILE]
### Conversion Date: [today]

### Epics Created
| Epic | Milestone | Tasks | Scope |
|------|-----------|-------|-------|
| Epic 1 | [name] | [N] | [priority] |
| Epic 2 | [name] | [N] | [priority] |

### Issues Created
| Epic | Task | Title | Scope | Validators |
|------|------|-------|-------|------------|
| 1 | 1 | [title] | simple | ci,canonical |
| 1 | 2 | [title] | moderate | ci,architecture,canonical |
...

### Next Steps
1. Review drafts in .pi/context/issue-drafts/
2. Edit any issue details as needed
3. Run `/git-issues` to create in repository
EOF
```

### 10. Review Drafts

Before proceeding, review all drafts:

**Review Checklist:**
- [ ] All milestones converted to epics
- [ ] All tasks converted to issues
- [ ] All steps converted to acceptance criteria
- [ ] Code snippets preserved as implementation hints
- [ ] File references captured as affected files
- [ ] Scope classification reasonable
- [ ] Validators assigned appropriately
- [ ] Dependencies mapped correctly
- [ ] Tracking issue includes all epics/issues

---

## Output Format

```
.pi/context/issue-drafts/
├── summary.md              # Conversion summary
├── tracking-issue-draft.md # Overall tracking
├── epic-1-draft.md         # Epic for Milestone 1
├── epic-2-draft.md         # Epic for Milestone 2
├── issue-1-1-draft.md      # Issue for Epic 1, Task 1
├── issue-1-2-draft.md      # Issue for Epic 1, Task 2
├── issue-2-1-draft.md      # Issue for Epic 2, Task 1
└── review-checklist.md     # Review checklist
```

---

## Git Repository Tool

Using `gh`:

| Tool | Preview Command |
|------|-----------------|
| **gh** | `gh issue create --title "[TITLE]" --body "[BODY]"` |
| **glab** | `glab issue create --title "[TITLE]" --description "[BODY]"` |

---

## Acceptance Criteria

- [ ] Plan file located and parsed successfully
- [ ] All milestones converted to epic drafts
- [ ] All tasks converted to issue drafts
- [ ] All steps converted to acceptance criteria
- [ ] Code snippets preserved as implementation hints
- [ ] Scope classification applied to each issue
- [ ] Validators assigned based on scope
- [ ] Tracking issue draft created
- [ ] All drafts saved to .pi/context/issue-drafts/
- [ ] Summary generated
- [ ] Ready for `/git-issues`

---

## Example Conversion

**Input (Plan):**
```markdown
## Milestone 1: L402 Payment End-to-End (CRITICAL PATH)

### Task 1: Project Scaffold

- [ ] **Step 1: Create autonomics package.json**
  ```json
  {
    "name": "autonomics",
    "version": "0.1.0"
  }
  ```
  - File: `package.json`

- [ ] **Step 2: Initialize TypeScript config**
  - Commit message: `chore: init tsconfig`
```

**Output (Issue Draft):**
```markdown
## Issue: Project Scaffold

### Epic: L402 Payment End-to-End

### Type: feature

### Priority: high (CRITICAL PATH)

### Description
Initialize project scaffold with package.json and TypeScript configuration.

### Acceptance Criteria
- [ ] **Step 1:** Create autonomics package.json
- [ ] **Step 2:** Initialize TypeScript config

### Implementation Notes

**Example for Step 1:**
```json
{
  "name": "autonomics",
  "version": "0.1.0"
}
```

**Files Affected:**
- package.json

**Commit Message:** `chore: init tsconfig`

### Estimated Scope
- Files: 1
- Lines: < 50
- Validator Scope: simple

### Validators Required
- ci, canonical
```

---

## Next Workflow

After draft review and approval, run: `/git-issues` to create in repository.