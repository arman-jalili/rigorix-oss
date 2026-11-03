# Output Formats

> **Purpose:** Standardized report templates. All validators use these. Referenced, not duplicated.
> **Generic:** Adjust severity levels and fields to your project.

---

## Validation Report (All Validators)

```markdown
## [Validator Name] Report

**Task:** [task-id or description]
**Scope:** [Simple/Moderate/Complex/Critical]
**Files Reviewed:** [list]

### Results

| Check | Status | Notes |
|-------|--------|-------|
| [Check 1] | ✅/❌ | [notes] |
| [Check 2] | ✅/❌ | [notes] |

### Issues

| Severity | File:Line | Description | Fix |
|----------|-----------|-------------|-----|
| [Critical/High/Medium/Low] | [file:line] | [description] | [fix] |

### Verdict

- [ ] APPROVED
- [ ] APPROVED WITH CONDITIONS
- [ ] REQUIRES CHANGES
- [ ] REJECTED

### Conditions (if any)
- [condition 1]
```

## Design Proposal

```markdown
## Design Proposal: [Name]

### Summary
[1-2 paragraphs]

### Approach
[Detailed approach]

### Affected Components
- [Component]: [Changes]

### Tradeoffs
| Option | Pros | Cons | Decision |
|--------|------|------|----------|
| A | ... | ... | Chosen |
| B | ... | ... | Rejected |

### Risks
| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| [Risk] | [H/M/L] | [H/M/L] | [Mitigation] |
```

## Implementation Plan

```markdown
## Implementation Plan: [Name]

### Scope
- **Files:** [count]
- **Estimated lines:** [count]
- **Complexity:** [Simple/Moderate/Complex/Critical]

### Steps
1. [Step 1] — [file(s)]
2. [Step 2] — [file(s)]
3. [Step 3] — [file(s)]

### Dependencies
- [Dependency 1]
- [Dependency 2]

### Validation Contract
> Items pre-validated at plan time. Post-code review checks ONLY wiring + build.
- [ ] Architecture: [approved]
- [ ] Security: [approved]
- [ ] Operations: [approved]
```

## Epic Draft

```markdown
## Epic: [Name]

### Goal
[One sentence]

### Issues
- [ ] #[issue] — [title]
- [ ] #[issue] — [title]

### Acceptance Criteria
- [ ] [Criterion 1]
- [ ] [Criterion 2]

### Validation
- [ ] All issues merged
- [ ] All CI green
- [ ] Post-merge validation passed
```

## CI/MR Report

```markdown
## CI/MR Validation

**PR:** #[number]
**Branch:** [branch-name]

### CI Status
| Check | Status | Duration |
|-------|--------|----------|
| Build | ✅/❌ | [time] |
| Test | ✅/❌ | [time] |
| Lint | ✅/❌ | [time] |
| Security | ✅/❌ | [time] |

### Verdict
- [ ] READY TO MERGE
- [ ] NEEDS CHANGES
- [ ] BLOCKED
```
