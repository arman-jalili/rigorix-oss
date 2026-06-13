import type { ArchitectureSlice, ModuleComponent } from "./types.ts";

// ── Issue Generation ──

export function generateIssueMarkdown(
	component: ModuleComponent,
	slice: ArchitectureSlice,
	issueIndex: number,
	totalIssues: number,
): string {
	const moduleId = slice.module.replace(/^module-/, "");
	const componentName = component.name.toLowerCase().replace(/\s+/g, "-");
	const issueId = `ISSUE-${moduleId.toUpperCase()}-${issueIndex + 1}`;

	return `---
guardian_issue:
  id: "${issueId}"
  epic: "TBD"
  component: "${component.name}"
  module: "${slice.module}"
  status: planned
  priority: high
  dependencies:
${component.dependencies.map((d) => `    - "${d}"`).join("\n")}

  in_scope:
    - Implement ${component.name} for the ${slice.module} module
    - Write unit tests for all public interfaces
    - Add integration tests with upstream/downstream components
    - Create API documentation

  out_of_scope:
    - Changes to upstream components (${component.dependencies.join(", ")})
    - UI/frontend changes
    - Deployment pipeline configuration

  affected_layers:
    domain:
      - New domain models for ${componentName}
    application:
      - New service/handler for ${componentName}
    infrastructure:
      - New database tables or external service connections
    api:
      - New endpoints or event handlers

  canonical_references:
    - module: ".pi/architecture/modules/${slice.module}.md#${componentName}"

  acceptance_criteria:
    - "CI pipeline passes (validate-ci.sh)"
    - "All unit tests pass with ≥ 90% coverage"
    - "Integration tests pass with upstream/downstream components"
    - "validate-security.sh passes"
    - "validate-architecture.sh passes"
    - "validate-canonical.sh passes"

  validators:
    - ci
    - tests
    - security
    - architecture
    - canonical

  implementation_notes: |
    ${component.description || "Implement this component according to the architecture module."}

  file_changes:
    - "create: src/${moduleId}/${componentName}/"
    - "create: tests/unit/${moduleId}/${componentName}/"
    - "create: tests/integration/${moduleId}/${componentName}/"
---

# ${issueId}: ${component.name}

## Intent

${component.description || `Implement ${component.name} for the ${slice.module} module.`}

## Architecture Context

- **Module:** ${slice.module}
- **Component:** ${component.name}
- **Status:** ${component.status}
- **Dependencies:** ${component.dependencies.length > 0 ? component.dependencies.join(", ") : "none"}

## Dependencies

\`\`\`
${component.dependencies.map((d) => `  └── ${d}`).join("\n") || "  └── (root component — no dependencies)"}
\`\`\`

## In Scope

- Implement ${component.name} for the ${slice.module} module
- Write unit tests for all public interfaces
- Add integration tests with upstream/downstream components
- Create API documentation

## Out of Scope

- Changes to upstream components
- UI/frontend changes
- Deployment pipeline configuration

## Affected Layers

### Domain
- New domain models for ${componentName}

### Application
- New service/handler for ${componentName}

### Infrastructure
- New database tables or external service connections

### API
- New endpoints or event handlers

## Canonical References

- **Module:** \`.pi/architecture/modules/${slice.module}.md#${componentName}\`

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | CI pipeline passes | \`validate-ci.sh\` |
| 2 | All unit tests pass with ≥ 90% coverage | \`validate-tests.sh\` |
| 3 | Integration tests pass | \`validate-integration.sh\` |
| 4 | Security checks pass | \`validate-security.sh\` |
| 5 | Architecture compliance | \`validate-architecture.sh\` |
| 6 | Canonical references valid | \`validate-canonical.sh\` |

## Implementation

> **Agent:** This is your complete session context. All information you need is above.
> Start by reading the canonical reference files, then implement following the layer structure.

### Steps

1. Read canonical architecture references
2. Create domain entities and interfaces
3. Implement application service/handler
4. Add infrastructure connections
5. Write unit tests (≥ 90% coverage)
6. Write integration tests
7. Run all validators
8. Create MR
`;
}
// ── Contract Freeze Generator ──

export function generateContractFreezeMarkdown(
	slice: ArchitectureSlice,
	epicName: string,
): string {
	const moduleId = slice.module.replace(/^module-/, "");

	return `---
guardian_issue:
  id: "ISSUE-CONTRACT-FREEZE"
  epic: "${epicName}"
  component: "Contract Freeze"
  module: "${slice.module}"
  status: planned
  priority: critical
  dependencies: []

  in_scope:
    - Define public interfaces for all components in this epic
    - Define DTOs, schemas, and API contracts
    - Document event payloads and topics
    - Create interface stubs with no implementation
    - Freeze: no implementation changes without contract change

  out_of_scope:
    - Any implementation logic
    - Database schema changes
    - Infrastructure setup

  affected_layers:
    domain:
      - Interface definitions for domain services
    application:
      - Input/output DTO definitions
    api:
      - REST/event contracts

  canonical_references:
    - module: ".pi/architecture/modules/${slice.module}.md"

  acceptance_criteria:
    - "All component interfaces defined as interfaces/types"
    - "DTO schemas documented"
    - "API contracts frozen and reviewed"
    - "Implementation PRs reference these contracts"

  validators:
    - architecture
    - canonical

  implementation_notes: |
    Define the contract before any implementation. Every implementation issue
    depends on this contract being frozen first. The contract should include:
    interfaces, types, DTOs, event schemas, API paths, error formats.

  file_changes:
    - "create: src/${moduleId}/contracts/"
    - "create: src/${moduleId}/contracts/dtos/"
    - "create: src/${moduleId}/contracts/events/"
---

# Contract Freeze: ${slice.module}

## Intent

Define and freeze all public interfaces, contracts, and schemas for the ${slice.module}
epic before any implementation begins. This prevents architecture drift — implementation
must satisfy contracts, not the other way around.

## Included Components

${slice.nextLogicalSlice.map((c: { name: string }) => `- ${c.name}`).join("\n")}

## What Must Be Frozen

### Interfaces
- Service interfaces for every component
- Repository/DAO interfaces
- Factory interfaces

### Contracts
- Input/output DTO schemas
- API endpoint contracts (method, path, request/response)
- Event payload schemas
- Error response formats

### Out of Bounds (no contracts needed)
- Internal implementation details
- Database column names (hidden behind repository)
- Framework-specific annotations

## Acceptance Criteria

| # | Criterion | How to Verify |
|---|-----------|---------------|
| 1 | All component interfaces defined | Check src/<group>/<module>/domain/ and application/ |
| 2 | Contracts reviewed and frozen | PR approval |
| 3 | DTO schemas documented | OpenAPI / TypeSpec / equivalent |
| 4 | Implementation depends on contracts | No implementation without interface |

## Implementation

> **Agent:** Create interface-only files. No implementation. Use Clean Architecture layers:
> 1. Read the architecture module to understand each component's role
> 2. Place domain interfaces in domain/, service interfaces in application/, API contracts in interfaces/http/
> 3. DTOs with proper validation decorators go in application/
> 4. Event schemas go in domain/event/
> 5. Repository interfaces go in infrastructure/repository/
>
> The goal is a reviewed, frozen contract that implementation issues can depend on.
`;
}

// ── Proofing Issue Generator ──

export function generateProofingMarkdown(
	slice: ArchitectureSlice,
	epicName: string,
): string {
	const moduleId = slice.module.replace(/^module-/, "");

	return `---
guardian_issue:
  id: "ISSUE-PROOFING"
  epic: "${epicName}"
  component: "Proofing & CI Enforcement"
  module: "${slice.module}"
  status: planned
  priority: critical
  dependencies: []

  in_scope:
    - Create deterministic validation scripts for each contract
    - Verify all interfaces have matching implementations
    - Check test coverage meets thresholds
    - Integrate proofing scripts into .pi/scripts/ci/
    - Scripts must be self-contained shell scripts (zero token cost)

  out_of_scope:
    - Implementation changes
    - New features
    - Production deployment

  affected_layers:
    ci:
      - New proofing scripts in .pi/scripts/ci/
      - Updated CI stage configuration

  canonical_references:
    - module: ".pi/architecture/modules/${slice.module}.md"

  acceptance_criteria:
    - "All proofing scripts created and executable"
    - "Each contract has at least one validation check"
    - "Scripts pass on current implementation"
    - "Scripts fail if implementation is removed"
    - "Scripts integrated into CI pipeline (stage in run_hardening_stages.sh)"

  validators:
    - ci
    - tests
    - canonical

  implementation_notes: |
    Create deterministic shell scripts that validate: each defined interface has an
    implementation, each implementation has tests, test coverage meets threshold,
    contracts are not violated. These escape the LLM ad-hoc check trap — they run
    every build for zero token cost.

  file_changes:
    - "create: .pi/scripts/ci/check_${moduleId}_contracts.sh"
    - "create: .pi/scripts/ci/check_${moduleId}_coverage.sh"
    - "modify: .pi/scripts/ci/run_hardening_stages.sh"
---

# Proofing & CI Enforcement: ${slice.module}

## Intent

Create deterministic, automated validation scripts that prove every contract from the
freeze phase is correctly implemented and tested. These scripts make compliance
automatic — no human review needed for routine checks.

## What Each Script Does

### Contract Implementation Check
- Reads each interface from the contract freeze
- Verifies a concrete implementation class exists
- Verifies all interface methods are implemented
- Reports violations with file:line references

### Coverage Threshold Check
- Runs the project's coverage tool
- Asserts each module meets minimum coverage (default 80%)
- Fails the build if coverage drops

### CI Integration
Each check becomes a CI stage in the hardening pipeline — it runs automatically
on every PR. No LLM cost. No human review. Just pass or fail.

## Scripts To Create

| Script | Purpose | Location |
|--------|---------|----------|
| check_${moduleId}_contracts.sh | Validate contract implementation | .pi/scripts/ci/ |
| check_${moduleId}_coverage.sh | Enforce coverage thresholds | .pi/scripts/ci/ |
| stage_${moduleId}_proofing.sh | CI stage wrapper | .pi/scripts/ci/ |

## CI Pipeline Update

Add the new stage to \`run_hardening_stages.sh\`:

\`\`\`bash
run_stage "11" "${moduleId}_proofing" \\
    "\${SCRIPTS_DIR}/stage_${moduleId}_proofing.sh" \\
    "always"
\`\`\`

## Acceptance Criteria

| # | Criterion | Script |
|---|-----------|--------|
| 1 | All interfaces have implementations | check_contracts.sh |
| 2 | Coverage ≥ 80% per module | check_coverage.sh |
| 3 | CI runs checks on every PR | run_hardening_stages.sh |
| 4 | All scripts exit 0 on pass, 1 on fail | self-validating |

## Implementation

> **Agent:** Create shell scripts. Keep them simple — grep, find, awk.
> No frameworks, no dependencies. Each script should be:
> 1. Runnable standalone (bash script.sh)
> 2. Runnable as a CI stage
> 3. Self-documenting with --help
> 4. Exit 0 for pass, 1 for fail
>
> End by running the full CI pipeline to verify integration:
> \`bash .pi/scripts/ci/run_hardening_stages.sh\`
`;
}

// ── Architecture Readiness Generator (expanded) ──

export function generateArchitectureReadinessMarkdown(
	slice: ArchitectureSlice,
	epicName: string,
): string {
	const moduleId = slice.module.replace(/^module-/, "");

	return `---
guardian_issue:
  id: "ISSUE-READINESS"
  epic: "${epicName}"
  component: "Architecture Readiness"
  module: "${slice.module}"
  status: planned
  priority: critical
  dependencies: []

  in_scope:
    - Create runbook (startup, shutdown, recovery procedures)
    - Create DR plan (backup, restore, failover)
    - Add observability (metrics, tracing, structured logging)
    - Add health check endpoints
    - Update architecture documentation
    - Sync canonical references
    - Verify CI enforces all the above

  out_of_scope:
    - New feature work
    - Implementation changes

  affected_layers:
    domain:
      - Architecture documentation updates
    application:
      - Observability hooks
    infrastructure:
      - Health checks, monitoring config
    ci:
      - Verify proofing scripts + validators in CI

  canonical_references:
    - module: ".pi/architecture/modules/${slice.module}.md"

  acceptance_criteria:
    - "Runbook created and reviewed"
    - "DR plan documented"
    - "Observability patterns in place (tracing, metrics, logging)"
    - "Health check endpoint responds"
    - "Architecture docs synced with implementation"
    - "Canonical references verified (validate-canonical.sh passes)"
    - "Proofing scripts integrated in CI and passing"
    - "All validators pass: ci, tests, security, architecture, canonical, operations"

  validators:
    - ci
    - tests
    - security
    - architecture
    - canonical
    - operations

  implementation_notes: |
    The final issue in every epic. Production readiness means: the team can operate it
    (runbook), recover from failure (DR plan), observe it (metrics/tracing/logging),
    and CI will catch regressions (proofing scripts + validators).

  file_changes:
    - "create: docs/runbook-${moduleId}.md"
    - "create: docs/dr-plan-${moduleId}.md"
    - "modify: .pi/architecture/CHANGELOG.md"
    - "modify: .pi/architecture/modules/${slice.module}.md"
---

# Architecture Readiness: ${slice.module}

## Intent

Make the ${slice.module} module production-ready. This is the final issue in every epic
— it closes the loop between implementation and operability.

## Deliverables

### Runbook
\`docs/runbook-${moduleId}.md\` covering:
- Startup sequence and dependencies
- Graceful shutdown procedure
- Common failure modes and recovery
- Configuration reference

### DR Plan
\`docs/dr-plan-${moduleId}.md\` covering:
- Backup strategy and schedule
- Restore procedure
- Failover plan
- RTO/RPO targets

### Observability
- Metrics: key business and technical metrics exposed
- Tracing: distributed tracing context propagated
- Logging: structured logging with correlation IDs
- Health: /health endpoint with dependency checks

### CI Enforcement
Verify that:
- Proofing scripts from the proofing issue are in CI
- All validators (ci, tests, security, architecture, canonical, operations) pass
- A CI pipeline run against this state succeeds

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Runbook exists | manual review |
| 2 | DR plan exists | manual review |
| 3 | Observability patterns present | validate-operations.sh |
| 4 | Canonical references synced | validate-canonical.sh |
| 5 | CI enforce validators | validate-ci.sh |
| 6 | All proofing scripts pass | run_hardening_stages.sh |
| 7 | Architecture docs updated | validate-architecture.sh |

## Implementation

> **Agent:** Close out the epic properly:
> 1. Write runbook and DR plan docs
> 2. Add observability instrumentation
> 3. Update architecture module docs with final implementation details
> 4. Sync CHANGE LOG
> 5. Verify proofing scripts from the proofing issue pass
> 6. Run full validation suite
> 7. Architecture readiness validator: bash .pi/scripts/validate-architecture-readiness.sh
> 8. Create final MR
`;
}

