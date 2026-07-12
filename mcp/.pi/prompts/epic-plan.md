# Epic Plan Workflow

**Purpose:** Plan epics for multi-module projects. Supports cross-module overview planning and module-specific slice planning from architecture documents.

---

## Usage Modes

| Mode | Command | When to Use |
|------|---------|-------------|
| **Overview** | `/epic-plan --overview` | Plan epics across ALL modules (backend + frontend + infra, etc.) |
| **Module Slice** | `/epic-plan --module <name> <input-doc>` | Plan the next slice for a specific module using its architecture doc |
| **Free-Form** | `/epic-plan <feature-description>` | Quick plan for a specific feature (legacy mode) |

---

## Mode 1: Epic Overview (All Modules)

**Command:** `/epic-plan --overview`

### Step 1: Discover Modules

Scan for all architecture documents:

```bash
# Find all architecture docs
find .pi/architecture/modules -name "*.md" -type f 2>/dev/null
find .pi -name "*-architecture.md" -type f 2>/dev/null
ls .pi/architecture/modules/ 2>/dev/null
```

**Identify each module:**
- Backend (e.g., `backend-architecture.md`, API services, data layer)
- Frontend (e.g., `frontend-architecture.md`, UI, client-side logic)
- Infrastructure (e.g., `infra-architecture.md`, deployment, CI/CD, monitoring)
- Shared/Cross-cutting (e.g., auth, config, contracts)

### Step 2: Load Each Module's Architecture

For each discovered module:
1. Read its architecture document
2. Read its module doc in `.pi/architecture/modules/`
3. Check `.pi/architecture/CHANGELOG.md` for pending changes
4. Identify current state: what's built, what's planned, what's blocked

### Step 3: Map Cross-Module Dependencies

Build a dependency matrix:

```markdown
## Cross-Module Dependency Map

| Module | Depends On | Provides To | Shared Contracts |
|--------|-----------|-------------|------------------|
| Backend | — | Frontend (API), Infra (health checks) | API schema, auth tokens |
| Frontend | Backend (API) | — | UI component library |
| Infra | Backend, Frontend | All (deploy, monitor) | Docker configs, CI pipelines |
```

### Step 4: Plan Cross-Module Epics

For each epic, identify which modules are involved and in what order:

```markdown
## Epic Overview: [EPIC_NAME]

### Goal
[What this epic achieves across the full system]

### Module Breakdown

| Module | Scope | Key Changes | Dependencies | Order |
|--------|-------|-------------|--------------|-------|
| Backend | Moderate | New API endpoint, data model | None | 1st |
| Frontend | Simple | UI component for new data | Backend API contract | 2nd |
| Infra | Minimal | Health check update | Backend endpoint | 3rd |

### Cross-Module Risks
- **Contract drift:** Backend and Frontend must agree on API schema before either starts
- **Deployment order:** Backend must deploy before Frontend can use new endpoint
- **Rollback:** All modules must be rolled back together if issues arise

### Epic Sequence
1. **[EPIC-001] Backend API** — Implement data model + endpoint + tests
2. **[EPIC-002] Frontend Integration** — Build UI component, connect to API
3. **[EPIC-003] Infra Hardening** — Update health checks, monitoring, deploy

### Issue Breakdown (All Modules)

#### EPIC-001: Backend API
1. **[Issue]** Define API contract (OpenAPI/schema) — Scope: simple
2. **[Issue]** Implement data model + migration — Scope: moderate
3. **[Issue]** Implement endpoint + validation — Scope: moderate
4. **[Issue]** Write integration tests — Scope: moderate

#### EPIC-002: Frontend Integration
1. **[Issue]** Generate API client from contract — Scope: simple
2. **[Issue]** Build UI component — Scope: moderate
3. **[Issue]** Wire up to backend + error handling — Scope: simple

#### EPIC-003: Infra Hardening
1. **[Issue]** Update health check endpoint — Scope: simple
2. **[Issue]** Add monitoring alert for new endpoint — Scope: simple
```

### Step 5: Deterministic Packet Validation

Before validator review, validate the planning packet structure:

```bash
python scripts/ci/check_planning_packet.py --input=planning_packet.md
```

If packet validation fails, fix the structure before proceeding to validators.

### Step 6: Validator Review (All Modules)

Run validators against the full epic plan:

- **Architecture Validator:** Check cross-module boundaries, dependency direction, contract stability
- **Security Validator:** Check data flow across modules, auth boundaries, external service interactions
- **Operations Validator:** Check deployment order, rollback strategy, observability across modules

### Step 6: Decision

As **architecture-coordinator**:
- If all APPROVED → proceed to `/issue-draft` for each epic in sequence
- If any CONDITIONAL → address recommendations, re-validate
- If any REJECTED → revise epic plan, re-validate

---

## Mode 2: Module Slice Planning

**Command:** `/epic-plan --module <name> <input-doc>`

Example: `/epic-plan --module frontend docs/frontend-architecture.md`

### Step 1: Load the Module Architecture

Read the specified architecture document:

```bash
cat <input-doc>
cat .pi/architecture/modules/<module-name>.md 2>/dev/null
cat .pi/architecture/CHANGELOG.md 2>/dev/null
```

**Extract from the architecture doc:**
- Current state: what exists today
- Planned changes: what the doc says should be built
- Components: the building blocks and their responsibilities
- Dependencies: what this module needs from other modules
- Contracts: APIs, interfaces, data schemas this module exposes or consumes

### Step 2: Assess Implementation Gap

Compare current state vs. planned state:

```markdown
## Implementation Gap: <module-name>

| Component | Current State | Target State | Gap Size |
|-----------|--------------|--------------|----------|
| [Component A] | Partially implemented | Complete | Moderate |
| [Component B] | Not started | Required | Large |
| [Component C] | Complete | Complete | None |
| [Component D] | Needs refactor | Redesigned | Moderate |
```

### Step 3: Check Pending Changes

Check `.pi/architecture/CHANGELOG.md` for any pending architecture changes that affect this module:

- Are there pending ADRs that impact this module?
- Are there recent changes that create follow-up work?
- Are there superseded decisions that need cleanup?

### Step 4: Slice the Next Epic

Based on the gap analysis and pending changes, propose the next slice:

**Slicing Criteria:**
- **Cohesion:** Issues in the epic should touch related components
- **Independence:** Epic should be implementable without waiting on other modules (or have clear contracts)
- **Value:** Epic should deliver measurable progress toward the architecture goal
- **Risk:** Don't cluster high-risk items; spread complexity across epics

**Output Format:**

```markdown
## Epic Slice: [EPIC_NAME] — [Module]

### Source Architecture
[input-doc path] — [section/heading that drives this epic]

### Summary
[2-3 sentence description of what this slice accomplishes for the module]

### Architecture Slice
[Which components/interfaces this epic touches]

### Estimated Scope
- Files: [X]
- Lines: [Y]
- Validators Required: [list]

### Issue Breakdown
1. **[Issue 1 Title]** — [Description] — Scope: [simple/moderate/complex] — Component: [name]
2. **[Issue 2 Title]** — [Description] — Scope: [simple/moderate/complex] — Component: [name]
3. **[Issue 3 Title]** — [Description] — Scope: [simple/moderate/complex] — Component: [name]

### Cross-Module Dependencies
| Dependency | Module | Status | Notes |
|-----------|--------|--------|-------|
| [API contract] | Backend | ✅ Ready / ⏳ Pending / ❌ Blocked | [details] |
| [UI component] | Frontend | ✅ Ready / ⏳ Pending / ❌ Blocked | [details] |

### Risk Assessment
- **Architecture Risk:** [Low/Medium/High] — [reason]
- **Security Risk:** [Low/Medium/High] — [reason]
- **Operations Risk:** [Low/Medium/High] — [reason]

### Pending Architecture Changes
[List any CHANGELOG items that must be addressed in or after this epic]
```

### Step 5: Validator Review

Run validators scoped to this module:

**Architecture Validator:**
```
/architecture-validator

Review epic slice for <module-name>:
1. Compliance with <input-doc> architecture decisions
2. Component boundary correctness
3. Dependency direction (should flow inward)
4. Interface stability with other modules
```

**Security Validator:** (if module touches auth, data, or external services)
```
/security-validator

Review epic slice for <module-name>:
1. Data flow security
2. Authentication/authorization changes
3. External service interactions
4. Sensitive data handling
```

**Operations Validator:** (if module affects deployment, monitoring, or performance)
```
/operations-validator

Review epic slice for <module-name>:
1. Observability impact
2. Deployment complexity
3. Rollback strategy
4. Performance implications
```

### Step 6: Decision

As **architecture-coordinator**:
- If all APPROVED → proceed to `/issue-draft`
- If any CONDITIONAL → address recommendations, re-validate
- If any REJECTED → revise epic slice, re-validate

---

## Mode 3: Free-Form (Legacy)

**Command:** `/epic-plan <feature-description>`

Quick planning for a specific feature without module context. Use only for simple, single-module features.

### Steps
1. Analyze the feature description against current architecture
2. Identify which module(s) are affected
3. Propose an epic with issue breakdown
4. Run validators
5. Decision: approve, condition, or reject

---

## Validator Response Format

All validators output:

```markdown
## [Validator Name] Review

### Scope
[Modules/components reviewed]

### Status: ✅ APPROVED / ⚠️ CONDITIONAL / ❌ REJECTED

### Findings
- [Finding 1]
- [Finding 2]

### Recommendations
- [Recommendation 1]
- [Recommendation 2]

### Required Changes (if CONDITIONAL/REJECTED)
- [Change 1]
- [Change 2]
```

---

## Acceptance Criteria

### For Overview Mode
- [ ] All modules discovered and architecture docs loaded
- [ ] Cross-module dependency map documented
- [ ] Epic sequence ordered by dependency
- [ ] All validators reviewed cross-module risks
- [ ] Epic overview ready for `/issue-draft` (sequential epics)

### For Module Slice Mode
- [ ] Input architecture document loaded and analyzed
- [ ] Implementation gap assessed
- [ ] Pending architecture changes identified
- [ ] Epic slice proposed with issue breakdown
- [ ] Cross-module dependencies documented
- [ ] All validators reviewed module-specific risks
- [ ] Epic slice ready for `/issue-draft`

### For Free-Form Mode
- [ ] Feature analyzed against architecture
- [ ] Affected modules identified
- [ ] Epic proposed with issue breakdown
- [ ] Validators reviewed
- [ ] Epic ready for `/issue-draft`

---

## Git Repository Tool

Use the configured repository tool (`gh`):

- **gh** (GitHub): `gh issue list`, `gh issue create`, `gh issue view`, `gh epic list`
- **glab** (GitLab): `glab issue list`, `glab issue create`, `glab issue view`, `glab epic list`

Check existing epics/issues before proposing new ones to avoid duplicates.

---

## Next Workflow

After approval, run: `/issue-draft`