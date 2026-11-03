# Phase D: Validation Workflow

## Purpose
Verify issue/epic output against architecture and release policy before merge. Runs after each implemented issue and before epic close.

## Primary Agents
- **Architecture Validator** (owner)
- **Security Validator** (if security-impacting)
- **Operations Validator** (if operations-impacting)

## Workflow Steps

### Step 1: Validation Path Selection
Classify the issue (config-only / application / mixed) and run the applicable validation:
```bash
python scripts/ci/validate_agent_output.py --input=implementation_report.md --schema=architecture-validator
```

### Step 2: Read Implementation
Review: implementation diff, implementation report, acceptance criteria, canonical references, validation path evidence.

### Step 3: Architecture Validation
Check: artifact classification, section references, scope-fit, evidence precision, acceptance criteria evaluation, contradiction detection.

### Step 4: Security Validation (if applicable)
Check: tenant isolation preserved, auth boundaries maintained, secrets handled correctly, no security regressions.

### Step 5: Operations Validation (if applicable)
Check: observability added, runbooks updated, rollback path clear, SLOs defined.

### Step 6: CI Verification
Run applicable pipeline stages.

## Output: Validation Summary

```markdown
## Overall Validation Status
- pass | pass_with_recommendations | fail

## Validator Decisions
- Architecture: <decision>
- Security: <decision or N/A>
- Operations: <decision or N/A>

## Blocking Findings
## Non-Blocking Recommendations
## Required Follow-up
## Merge Authorization
- [ ] Architecture validator: APPROVED
- [ ] Security validator: APPROVED (if applicable)
- [ ] Operations validator: APPROVED (if applicable)
- [ ] CI pipeline: PASSED
- [ ] Ready to merge
```

## Done Criteria
- [ ] All validators pass
- [ ] Correct validation path selected
- [ ] All CI gates pass
- [ ] No blocking findings
- [ ] Follow-up issues created (if needed)
- [ ] Ready to merge

## Related Documents
- `.pi/agents/architecture-validator.md`
- `.pi/agents/security-validator.md`
- `.pi/agents/operations-validator.md`
- `scripts/ci/validate_agent_output.py`
