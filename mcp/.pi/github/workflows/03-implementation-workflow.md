# Phase C: Implementation Workflow

## Purpose
Execute ONE issue at a time with strict acceptance-criteria closure and minimal drift.

## Primary Agent
- **Bootstrap Implementer** (owner)
- **Architecture Validator** (post-implementation validation)

## Input
- Single assigned issue from Phase B
- Planning packet (scope context)
- Dependency status

## Workflow Steps

### Step 1: Readiness Check
Verify: epic exists, issue exists, dependencies satisfied, issue is implementation-ready.

### Step 2: Acceptance Criteria Trace Map
Before coding, create a trace map: each criterion → files to change → evidence → validation command.

### Step 3: Validation Path Selection
Classify the issue:
- **Config-only** — changes limited to config, CI wiring, or docs
- **Application** — any change to source code, contracts, or runtime behavior
- **Mixed** — both

Run the applicable validation path:
```bash
# Config-only path
bash scripts/ci/check_config.sh

# Application path
./scripts/ci/run_preflight.sh --staged --json
```

### Step 4: Implementation
For each acceptance criterion:
1. Create feature branch
2. Read canonical references
3. Implement minimum to satisfy AC
4. Write tests
5. Run validation command

### Step 5: Toolchain Validation
Run the blocking validation path selected in Step 3.

### Step 6: Evidence Collection
For each AC, collect: file paths changed, tests written/passed, commands run, output.

### Step 7: Implementation Report
Produce report with: readiness check, AC trace map, files changed, validation results, done/not-done per AC.

## Output: Implementation Report
- Readiness check
- Acceptance criteria trace map
- Files changed
- Tests and CI impacts
- Toolchain validation results
- Done / not done against acceptance criteria
- Evidence produced

## Done Criteria
- [ ] All acceptance criteria met
- [ ] Evidence collected for each AC
- [ ] Correct validation path selected
- [ ] Toolchain validation passes
- [ ] Tests written and passing
- [ ] Documentation updated
- [ ] Implementation report ready for validator

## Next Phase
**→ Phase D: Validation** (Architecture Validator)
**→ Next Issue** (if more issues in epic)

## Related Documents
- `.pi/agents/bootstrap-implementer.md`
- `.pi/agents/architecture-validator.md`
- `.pi/prompts/issue-template-set.md`
