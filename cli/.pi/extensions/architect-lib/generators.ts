// Canonical Reference: .pi/extensions/architect-lib/generators.ts (guardian proofing/issue generator — vendored)
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import type { ArchitectureSlice, ModuleComponent } from "./types.ts";

// ── Module Doc Readers ───────────────────────────────────────────────────────

/** Read the raw module architecture doc, or empty string if missing. */
function readModuleDoc(cwd: string, moduleId: string): string {
	try {
		return readFileSync(
			join(cwd, ".pi", "architecture", "modules", `${moduleId}.md`),
			"utf-8",
		);
	} catch {
		return "";
	}
}

/** Extract a named section's body (between ## heading and next ## / --- / EOF). */
function extractSection(doc: string, heading: string): string {
	const m = doc.match(new RegExp(`## ${heading}\n([\\s\\S]*?)(?=\n##|\n---|$)`));
	return m ? m[1].trim() : "";
}

interface AcRow { num: string; component?: string; criterion: string; verifyIn: string; }

/**
 * Parse every AC row from ## Acceptance Criteria.
 * Supports:
 *   4-column: `| # | Component | Criterion | Verify In |`
 *   3-column: `| # | Criterion | Verify In |`
 * Returns rows with component populated when the 4-col format is detected.
 */
function parseAcTable(doc: string): AcRow[] {
	const section = extractSection(doc, "Acceptance Criteria");
	if (!section) return [];
	const rows = section.match(/^\| [\d✅☐][^|]*\|[^|]+\|[^|]+\|[^|]*\|$/gm) ?? [];
	return rows.map((row) => {
		const parts = row.split("|").map((p) => p.trim()).filter(Boolean);
		// 4-col: [num, component, criterion, verifyIn]
		// 3-col: [num, criterion, verifyIn]
		const isFourCol = parts.length >= 4 && parts[1] !== "Criterion";
		if (isFourCol) {
			return { num: parts[0] ?? "", component: parts[1], criterion: parts[2] ?? "", verifyIn: parts[3] ?? "" };
		}
		return { num: parts[0] ?? "", criterion: parts[1] ?? "", verifyIn: parts[2] ?? "" };
	});
}

/** Parse numbered steps from ## Implementation Sequence. */
function parseImplSequence(doc: string): string[] {
	const section = extractSection(doc, "Implementation Sequence");
	if (!section) return [];
	return (section.match(/^\d+\. .+$/gm) ?? []).map((s) => s.trim());
}

/**
 * Filter ACs that belong to a specific component.
 * 
 * When the AC table has a Component column (4-col format), ACs are matched
 * by component name. Falls back to keyword matching on criterion text when
 * no component column is present.
 */
function primaryAcs(acs: AcRow[], componentName: string): AcRow[] {
	if (acs.length === 0) return [];

	// Prefer explicit component column match
	const byComponent = acs.filter((ac) => ac.component && ac.component === componentName);
	if (byComponent.length > 0) return byComponent;

	// Fallback: keyword match on criterion text
	const lower = componentName.toLowerCase();
	const isDomainModel =
		(lower.includes("entity") || lower.includes("aggregate") ||
		 lower.includes("snapshot") || lower.includes("enum")) &&
		!lower.includes("repository");

	const matched = acs.filter((ac) =>
		ac.criterion.toLowerCase().includes(lower.replace(/[^a-z0-9]/g, "")),
	);
	if (matched.length > 0) return matched;

	if (isDomainModel) {
		const domainKeywords = ["entity", "aggregate", "enum", "value object", "jpa", "migration", "flyway", "snapshot", "status"];
		const filtered = acs.filter((ac) =>
			domainKeywords.some((kw) => ac.criterion.toLowerCase().includes(kw)),
		);
		if (filtered.length > 0) return filtered;
	}

	// Last resort: evenly split ACs among all components (assuming n components, take 1/n-th)
	return acs.slice(0, Math.max(1, Math.ceil(acs.length / 4)));
}

/** Format AcRow[] as a markdown table body (no header). Includes Component column if any row has one. */
function acRowsToMarkdown(rows: AcRow[]): string {
	const hasComponent = rows.some((r) => r.component);
	if (hasComponent) {
		return rows.map((r) => `| ${r.num} | ${r.component} | ${r.criterion} | ${r.verifyIn} |`).join("\n");
	}
	return rows.map((r) => `| ${r.num} | ${r.criterion} | ${r.verifyIn} |`).join("\n");
}

/** Build a human-readable title for the component issue. */
function buildIssueTitle(comp: ModuleComponent, moduleId: string): string {
	return `Implement ${comp.name} — ${moduleId}`;
}

/** Derive `in_scope` lines from primary ACs. Falls back to generic if none. */
function buildInScope(comp: ModuleComponent, primaryAcRows: AcRow[], implSteps: string[], moduleId: string): string[] {
	if (primaryAcRows.length > 0) {
		return primaryAcRows.slice(0, 6).map((r) => r.criterion.replace(/^✅\s*/, ""));
	}
	if (implSteps.length > 0) {
		return implSteps.slice(0, 4).map((s) => s.replace(/^\d+\.\s*/, ""));
	}
	return [
		`Implement ${comp.name} for the ${moduleId} module`,
		"Write unit tests for all public methods",
		"Add integration tests with upstream/downstream components",
	];
}

/** Detect whether project at `cwd` is a Java project. */
function isJavaProject(cwd?: string): boolean {
	if (!cwd) return false;
	try {
		return existsSync(join(cwd, "pom.xml")) ||
		       existsSync(join(cwd, "build.gradle")) ||
		       existsSync(join(cwd, "build.gradle.kts"));
	} catch {
		return false;
	}
}

/** Get the test runner command for a project language. */
function testRunnerFromCwd(cwd?: string): string | null {
	if (!cwd) return null;
	try {
		if (existsSync(join(cwd, "Cargo.toml"))) return "cargo test";
		if (existsSync(join(cwd, "go.mod"))) return "go test";
		if (existsSync(join(cwd, "pyproject.toml")) || existsSync(join(cwd, "requirements.txt"))) return "pytest";
		if (existsSync(join(cwd, "package.json"))) return "bun test";
	} catch {}
	return null;
}

/** Get the implementation file suffix for a project language. */
function implSuffix(cwd?: string): string {
	if (!cwd) return "ts";
	try {
		if (existsSync(join(cwd, "pom.xml")) || existsSync(join(cwd, "build.gradle")) || existsSync(join(cwd, "build.gradle.kts"))) return "java";
		if (existsSync(join(cwd, "Cargo.toml"))) return "rs";
		if (existsSync(join(cwd, "go.mod"))) return "go";
		if (existsSync(join(cwd, "pyproject.toml")) || existsSync(join(cwd, "requirements.txt"))) return "py";
	} catch {}
	return "ts";
}

// ── Issue Generation ──

export function generateIssueMarkdown(
	comp: ModuleComponent,
	slice: ArchitectureSlice,
	issueIndex: number,
	_totalIssues: number,
	tdd?: boolean,
	cwd?: string,
): string {
	const moduleId = slice.module.replace(/^module-/, "");
	const issueId = `ISSUE-${moduleId.toUpperCase()}-${issueIndex + 1}`;

	// ── Read module doc for specifics ──
	const doc = cwd ? readModuleDoc(cwd, slice.module) : "";
	const allAcs = parseAcTable(doc);
	const implSteps = parseImplSequence(doc);
	const compAcs = primaryAcs(allAcs, comp.name);

	const title = buildIssueTitle(comp, moduleId);
	const inScope = buildInScope(comp, compAcs, implSteps, moduleId);

	// Language-aware test paths
	const isJava = isJavaProject(cwd);
	const testBaseDir = isJava ? "src/test/java" : "tests/unit";
	const testRunnerHint = isJava ? "mvn test" : testRunnerFromCwd(cwd) || "bun test";

	const testLine = tdd
		? `    - "update: ${testBaseDir}/ (failing tests already generated — make them pass)"`
		: `    - "create: ${testBaseDir}/"`;

	// YAML front-matter ACs
	const yamlAcs = compAcs.length > 0
		? compAcs.map((r) => {
			const c = r.criterion.replace(/^✅\s*/, "").replaceAll("\"", "'");
			return `    - "${c}"`;
		}).join("\n")
		: `    - "CI pipeline passes (validate-ci.sh)"
    - "All unit tests pass"
    - "Architecture compliance (validate-architecture.sh)"
    - "Canonical references valid (validate-canonical.sh)"`;

	// Markdown body: all ACs
	const hasCompCol = allAcs.some((a) => a.component);
	const allAcHeader = hasCompCol
		? `| # | Component | Criterion | Verify In |\n|---|-----------|-----------|-----------|`
		: `| # | Criterion | Verify In |\n|---|-----------|-----------|`;
	const allAcMarkdown = allAcs.length > 0
		? `${allAcHeader}\n${acRowsToMarkdown(allAcs)}`
		: `| # | Criterion | Validator |\n|---|-----------|-----------|\n| 1 | CI pipeline passes | \`validate-ci.sh\` |\n| 2 | All unit tests pass | \`validate-tests.sh\` |\n| 3 | Integration tests pass | \`validate-integration.sh\` |\n| 4 | Architecture compliance | \`validate-architecture.sh\` |\n| 5 | Canonical references valid | \`validate-canonical.sh\` |`;

	// Implementation steps from module doc
	const stepsMarkdown = implSteps.length > 0
		? implSteps.map((s) => `- ${s}`).join("\n")
		: `- Read .pi/architecture/modules/${slice.module}.md\n- Implement entities and interfaces\n- Implement infrastructure (adapter, mapper, repository)\n- Implement use case\n- Write unit + integration tests\n- Run validators\n- Create MR`;

	const tddSteps = tdd
		? [
				"1. Read canonical architecture references",
				`2. Run the pre-generated failing tests: \`cd ${testBaseDir} && ${testRunnerHint}\``,
				"3. Verify tests FAIL (Red phase)",
				"4. Implement domain entities and interfaces",
				"5. Implement application service/handler",
				"6. Add infrastructure connections",
				"7. Run tests again — they should PASS (Green phase)",
				"8. Refactor if needed (Refactor phase)",
				"9. Write integration tests",
				"10. Run all validators",
				"11. Create MR",
			].join("\n")
		: [
				"1. Read canonical architecture references",
				"2. Create domain entities and interfaces",
				"3. Implement application service/handler",
				"4. Add infrastructure connections",
				"5. Write unit tests (≥ 90% coverage)",
				"6. Write integration tests",
				"7. Run all validators",
				"8. Create MR",
			].join("\n");

	return `---
guardian_issue:
  id: "${issueId}"
  epic: "TBD"
  component: "${comp.name}"
  module: "${slice.module}"
  status: planned
  priority: high
  dependencies:
${comp.dependencies.map((d) => `    - "${d}"`).join("\n")}

  in_scope:
${inScope.map((s) => { const sc = s.replaceAll("\"", "'"); return `    - "${sc}"`; }).join("\n")}

  out_of_scope:
    - Changes to upstream components (${comp.dependencies.join(", ") || "none"})
    - UI/frontend changes
    - Deployment pipeline configuration

  canonical_references:
    - module: ".pi/architecture/modules/${slice.module}.md"
    - acceptance_criteria: ".pi/architecture/modules/${slice.module}.md#acceptance-criteria"

  acceptance_criteria:
${yamlAcs}

  validators:
    - ci
    - tests
    - security
    - architecture
    - canonical

  implementation_notes: |
    Read .pi/architecture/modules/${slice.module}.md BEFORE implementing.
    All Acceptance Criteria in that file must be satisfied before this issue is closed.
    Component focus: ${comp.name}.
    CONCRETE IMPLEMENTATIONS MUST BE CREATED — interface stubs from the contract freeze
    are not sufficient. Each domain service/aggregate needs a concrete .impl.${implSuffix(cwd)} file.

  file_changes:
    - "create: src/${moduleId}/domain/"
    - "create: src/${moduleId}/application/"
    - "create: src/${moduleId}/infrastructure/"
    - "modify: src/${moduleId}/interfaces/"
    - ${testLine}
---

# ${issueId}: ${title}

## Intent

Implement **${comp.name}** for the \`${slice.module}\` module.

> ⚠️ **Read before implementing:** \`.pi/architecture/modules/${slice.module}.md\`
> Every item in the **Acceptance Criteria** section of that file must be satisfied
> before this issue is closed — including adapter, mapper, and WireMock items.

## Architecture Context

- **Module:** ${slice.module}
- **Component:** ${comp.name}
- **Status:** ${comp.status}
- **Dependencies:** ${comp.dependencies.length > 0 ? comp.dependencies.join(", ") : "none"}

## In Scope (this component)

${inScope.map((s) => `- ${s}`).join("\n")}

## Acceptance Criteria (this issue)

These acceptance criteria must be satisfied before this issue can be closed:

| # | Criterion | Verify In |
|---|-----------|-----------|
${compAcs.length > 0 ? compAcs.map((r) => `| ${r.num} | ${r.criterion} | ${r.verifyIn} |`).join("\n") : `| 1 | CI pipeline passes | \`validate-ci.sh\` |\n| 2 | All unit tests pass | \`validate-tests.sh\` |\n| 3 | Architecture compliance | \`validate-architecture.sh\` |\n| 4 | Canonical references valid | \`validate-canonical.sh\` |`}

## Full Module Acceptance Criteria

> All items below must pass before the **epic** is closed.
> Items may be split across multiple issues — verify your component's items before creating the MR.

${allAcMarkdown}

## Implementation Sequence (from module doc)

${stepsMarkdown}

## Implementation

> **Agent instructions:**
> 1. Open \`.pi/architecture/modules/${slice.module}.md\` — read the full Acceptance Criteria table
> 2. Identify which rows are your responsibility for **${comp.name}**
> 3. Create concrete implementation files (\`.impl.${implSuffix(cwd)}\`) in \`src/${moduleId}/\` — the interface stubs from the contract freeze are NOT enough
> 4. Each domain aggregate/service must have a working implementation with business logic
> 5. Verify each AC row is satisfied in \`src/\` before marking done
> 6. Run validators and create MR

### Steps

${tddSteps}
`;
}
// ── Contract Freeze Generator ──

export function generateContractFreezeMarkdown(
	slice: ArchitectureSlice,
	epicName: string,
	_codegenSkill?: string,
	cwd?: string,
): string {
	const moduleId = slice.module.replace(/^module-/, "");
	const doc = cwd ? readModuleDoc(cwd, slice.module) : "";
	const implSteps = parseImplSequence(doc);
	const allAcs = parseAcTable(doc);

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
    - "All component interfaces defined as stubs (TODO bodies)"
    - "DTO schemas documented with field names and types"
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
    - "create: src/${moduleId}/domain/"
    - "create: src/${moduleId}/application/"
    - "create: src/${moduleId}/infrastructure/"
    - "create: src/${moduleId}/interfaces/"
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
| 1 | All component interfaces defined as stubs (TODO bodies) | Check src/<module>/domain/ and application/ |
| 2 | Contracts reviewed and frozen | PR approval |
| 3 | DTO schemas documented with field names and types | OpenAPI / record types |
| 4 | Implementation depends on contracts | No implementation without interface |

${allAcs.length > 0
	? (() => {
		const hasComp = allAcs.some((a) => a.component);
		const header = hasComp
			? `| # | Component | Criterion | Verify In |\n|---|-----------|-----------|-----------|`
			: `| # | Criterion | Verify In |\n|---|-----------|-----------|`;
		return `## Full Module Acceptance Criteria (for reference)\n\n> These are the complete ACs for the module. The contract freeze must define the interfaces\n> so every row below can be implemented in subsequent issues.\n\n${header}\n${acRowsToMarkdown(allAcs)}`;
	})()
	: ""}

${implSteps.length > 0
	? `## Full Implementation Sequence (for reference)\n\n${implSteps.map((s) => '- ' + s).join("\n")}`
	: ""}

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

### Coverage Threshold Check (real coverage)
- Coverage is measured by the REAL workspace tool: \`cargo llvm-cov\` via
  \`.pi/scripts/coverage.sh --gate\` (repo-wide line-coverage gate, default
  60%; wired in ci.yml Stage 3b / local-ci Stage 4b)
- Do NOT create a per-module \`*_coverage.sh\` script — the heuristic
  grep-based module coverage scripts were removed repo-wide in #780 as
  coverage-theater; the llvm-cov \`target/coverage.lcov\` artifact provides
  the per-file breakdown instead

### CI Integration
Each check becomes a CI stage in the hardening pipeline — it runs automatically
on every PR. No LLM cost. No human review. Just pass or fail.

## Scripts To Create

| Script | Purpose | Location |
|--------|---------|----------|
| check_${moduleId}_contracts.sh | Validate contract implementation | .pi/scripts/ci/ |
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
| 2 | Coverage meets the real project gate (cargo llvm-cov, ≥ COVERAGE_THRESHOLD) | .pi/scripts/coverage.sh --gate |
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

