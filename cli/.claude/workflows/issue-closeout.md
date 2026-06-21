<!--
Canonical Reference: .pi/prompts/issue-closeout.md
Generated: 2026-06-16T04:28:47.990Z
DO NOT EDIT DIRECTLY - Modify source in .pi/
-->

# Issue Closeout Workflow

<!--
Canonical Reference: .pi/prompts/issue-closeout.md
Blueprint Source: Guardian Framework v1.2
-->

**Purpose:** Verify all acceptance criteria are met, run validators (including canonical reference check), and create a compliance merge request (MR) with complete documentation.

---

## Canonical Reference Requirement

**Before closeout, verify all implementation files reference architecture:**

```bash
bash .pi/scripts/validate-canonical.sh
```

**Files must include:**
```typescript
/**
 * Canonical Reference: .pi/architecture/modules/[module].md#[section]
 * Implements: [issue acceptance criteria]
 * Issue: #[number]
 */
```

**Architecture Sync Check:**
1. Check `.pi/architecture/CHANGELOG.md` for pending changes
2. Verify implementation matches current architecture spec
3. If CHANGELOG has pending changes for this module, note in MR

---

## Prerequisites

- Issue implementation completed
- All code changes committed to feature branch
- Issue number known
- Canonical references added to all modified files

---

## Workflow Steps

### 1. Verify Acceptance Criteria

Review the original issue and check each acceptance criterion:

```bash
# View original issue
gh issue view [ISSUE_NUMBER]
```

**Acceptance Criteria Checklist:**

| Criterion | Status | Evidence |
|-----------|--------|----------|
| [Criterion 1] | ✅/❌ | [How verified] |
| [Criterion 2] | ✅/❌ | [How verified] |
| [Criterion 3] | ✅/❌ | [How verified] |

**Verification Methods:**
- Manual testing: Describe test steps performed
- Automated tests: Reference passing test output
- Code review: Reference specific files changed
- Documentation: Reference docs updated

### 2. Run Validators

Execute all required validators based on issue scope:

**CI Validator (always required):**

```bash
bash .pi/scripts/validate-ci.sh
```

Expected output:
```
✅ Build: Passed
✅ Tests: All passing
✅ Lint: No errors
✅ Format: Correct
✅ Audit: No vulnerabilities
```

**Test Validator (for moderate+ scope):**

```bash
bash .pi/scripts/validate-tests.sh
```

Expected output:
```
✅ Unit tests: [X] tests passing
✅ Integration tests: [Y] tests passing
✅ Coverage: [Z]% (above threshold)
```

**Security Validator (for complex+ scope):**

```bash
bash .pi/scripts/validate-security.sh
```

Expected output:
```
✅ No hardcoded secrets
✅ No SQL injection patterns
✅ No path traversal vulnerabilities
✅ Input validation present
```

**Operations Validator (for production changes):**

```bash
bash .pi/scripts/validate-operations.sh
```

Expected output:
```
✅ Tracing implemented
✅ Cancellation handling correct
✅ Atomic writes used
✅ Error handling complete
```

**Canonical Reference Validator (always required):**

```bash
bash .pi/scripts/validate-canonical.sh
```

Expected output:
```
✅ Implementation files have canonical references
✅ References point to valid blueprint sections
✅ Coverage ≥ 50%
```

### 3. Validator Results Recording

Document all validator results:

```markdown
## Validator Results

### CI Validation
- Status: ✅ PASSED
- Build: Success (output snippet)
- Tests: [N] tests passed
- Lint: 0 errors, 0 warnings

### Test Validation
- Status: ✅ PASSED
- Unit Tests: [N] passed
- Integration Tests: [N] passed
- Coverage: [X]%

### Security Validation
- Status: ✅ PASSED
- Secrets Check: No issues
- Injection Check: No issues
- Path Traversal: No issues

### Operations Validation
- Status: ✅ PASSED
- Tracing: Implemented in [files]
- Cancellation: Handled in [files]
- Atomic Writes: Used in [files]

### Canonical Reference Validation
- Status: ✅ PASSED
- Files with references: [X]/[Y]
- Coverage: [Z]% (above 50% threshold)
- All references valid

### Overall Validation Status: ✅ ALL PASSED
```

### 4. Create Compliance MR

Create a merge request with complete compliance documentation:

**MR Title Format:**
```
[Issue #X] Issue Title - [brief summary]
```

**MR Body Template:**

```markdown
## Issue Closeout

Closes #[ISSUE_NUMBER]

### Summary
[Brief description of changes made]

### Acceptance Criteria Verification

| Criterion | Status | Evidence |
|-----------|--------|----------|
| [AC 1] | ✅ | [Evidence link/description] |
| [AC 2] | ✅ | [Evidence link/description] |
| [AC 3] | ✅ | [Evidence link/description] |

### Validator Results

| Validator | Status | Details |
|-----------|--------|---------|
| CI | ✅ PASSED | Build, tests, lint all passing |
| Test | ✅ PASSED | [N] tests, [X]% coverage |
| Security | ✅ PASSED | No vulnerabilities detected |
| Operations | ✅ PASSED | Production requirements met |
| Canonical | ✅ PASSED | [X]% coverage, all refs valid |

### Files Changed

| File | Change Type | Reason |
|------|-------------|--------|
| src/foo.ts | modified | [Reason] |
| tests/foo.test.ts | added | [Reason] |
| docs/foo.md | updated | [Reason] |

### Testing Evidence

**Unit Tests:**
```bash
[Test command output showing passes]
```

**Integration Tests:**
```bash
[Test command output showing passes]
```

### Manual Testing Performed

- [Test case 1]: [Result]
- [Test case 2]: [Result]

### Documentation Updates

- [ ] API documentation updated
- [ ] README updated (if applicable)
- [ ] Inline comments added for complex logic

### Deployment Notes

- [Any special deployment considerations]
- [Database migrations required: Yes/No]
- [Configuration changes required: Yes/No]

### Rollback Plan

[How to rollback if issues arise]

---

## Compliance Checklist

- [ ] All acceptance criteria verified
- [ ] All validators passed
- [ ] Tests cover new functionality
- [ ] Documentation updated
- [ ] No secrets or sensitive data in code
- [ ] Branch up-to-date with main
- [ ] MR ready for review

---

🤖 Generated with Guardian compliance workflow
```

### 5. Create MR in Repository

**For GitHub (gh):**

```bash
gh pr create \
  --title "[Issue #X] Issue Title" \
  --body "[MR_BODY_FROM_TEMPLATE]" \
  --base main \
  --head [feature-branch] \
  --assignee @me \
  --label "ready-for-review"
```

**For GitLab (glab):**

```bash
glab mr create \
  --title "[Issue #X] Issue Title" \
  --description "[MR_BODY_FROM_TEMPLATE]" \
  --target-branch main \
  --source-branch [feature-branch] \
  --assignee @me \
  --label "ready-for-review"
```

### 6. Link MR to Issue

**For GitHub:**
The MR body includes "Closes #X" which auto-links.

**For GitLab:**
```bash
glab api projects/:id/issues/[issue_iid] \
  -f merge_request_id="[mr_id]"
```

---

## Error Handling

| Error | Solution |
|-------|----------|
| Validator failed | Fix issues, re-run validators |
| Acceptance criteria not met | Complete implementation, re-verify |
| Tests failing | Debug and fix tests |
| MR creation failed | Check branch exists and is pushed |

---

## Acceptance Criteria

- [ ] All original issue acceptance criteria verified
- [ ] All required validators passed
- [ ] Canonical reference validator passed (≥50% coverage)
- [ ] Compliance MR created with full documentation
- [ ] MR linked to original issue
- [ ] MR status: ready for review
- [ ] Ready for `/issue-merge` after CI pipeline passes

---

## Next Workflow

After CI pipeline is green and MR approved, run: `/issue-merge`