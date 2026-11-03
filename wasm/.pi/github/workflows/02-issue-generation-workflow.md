# Phase B: Issue Generation Workflow

## Purpose
Convert an approved planning packet into an epic + issue set that are independently reviewable.

## Primary Agent
- **Issue Factory** (owner)

## Input
- Approved planning packet from Phase A
- Valid planning packet check: pass
- Architecture validation: pass

## Workflow Steps

### Step 1: Validate Planning Packet
Check scope is bounded, dependencies are clear, validators assigned, CI gates identified. If contradictions exist, return to Architecture Coordinator.

### Step 2: Create Issues
For each work unit, produce one issue with:
- Single outcome and owner
- Clear acceptance criteria
- Verification criteria
- Canonical references

**Issue breakdown pattern (feature stream):**
1. Contract issue — domain/API contracts first
2. Schema/Config issue — migrations, indexes, config
3. Service issue — repositories, adapters
4. Handler/Runtime issue — business logic, workers
5. Verification issue — tests, conformance
6. Rollout/Runbook issue — ops, monitoring

### Step 3: Dependency Ordering
Order by: contract → schema → service → handler → verification → rollout.

### Step 4: Issue Boundary Check
Each issue must have: one primary outcome, one owner, clear ACs, out-of-scope explicit, canonical references, independently reviewable.

## Output: Issue Set

```markdown
## Issues (in dependency order)
- Issue 1: <title>
- Issue 2: <title>
...

## Labels
- risk::<level>
- layer::<layers>
- type::<feature|hardening|migration>

## First implementation issue
- Issue 1: <title>
```

## Done Criteria
- [ ] All issues independently reviewable
- [ ] Acceptance criteria clear
- [ ] Verification criteria clear
- [ ] Dependency order correct
- [ ] First implementation issue identified

## Next Phase
**→ Phase C: Implementation** (Bootstrap Implementer)

## Related Documents
- `.pi/agents/issue-factory.md`
- `.pi/prompts/issue-template-set.md`
