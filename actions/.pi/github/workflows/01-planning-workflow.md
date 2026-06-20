# Phase A: Planning Workflow

## Purpose
Turn a goal into a bounded execution packet with validators and CI gates identified. This is the first workflow for every new epic.

## Primary Agent
- **Architecture Coordinator** (owner)
- **Architecture Validator** (mandatory review)
- **Security Validator** (conditional review)
- **Operations Validator** (conditional review)

## Workflow Steps

### Step 1: Stream Classification
Classify the goal into one of: **feature**, **hardening**, **migration**, or **control**.

### Step 2: Scope Boundaries
Define explicit in-scope and out-of-scope boundaries. List impacted layers/modules.

### Step 3: Dependency Graph
Map execution order. Identify blocked work, parallel-safe work, and the first issue.

### Step 4: Risk Classification
Assign low/medium/high with justification.

### Step 5: Validator Assignment
- Architecture Validator: always
- Security Validator: if security-impacting
- Operations Validator: if operations-impacting

### Step 6: CI Gate Selection
Select applicable gates: docs_policy, architecture_conformance, lint, static_analysis, unit, integration, security, migration_verify, release_readiness.

## Output: Planning Packet

```markdown
## Stream classification
## Scope summary
## In scope
## Out of scope
## Impacted layers
## Risk classification
## Dependency graph
## Mandatory validators
## Mandatory CI gates
## Forbidden shortcuts
## First implementation issue recommendation
## Open questions / escalation items
```

## Deterministic Validation
```bash
python scripts/ci/check_planning_packet.py --input=planning_packet.md
```

## Done Criteria
- [ ] Scope bounded and explicit
- [ ] All impacted layers identified
- [ ] Risk classification explicit
- [ ] Dependency graph clear
- [ ] Validators assigned
- [ ] CI gates identified
- [ ] First issue clear
- [ ] Deterministic validation passes
- [ ] Architecture validation passes

## Next Phase
**→ Phase B: Issue Generation** (Issue Factory)

## Related Documents
- `.pi/agents/architecture-coordinator.md`
- `.pi/agents/architecture-validator.md`
- `.pi/agents/security-validator.md`
- `.pi/agents/operations-validator.md`
- `.pi/context/domain-workflow.md`
