# Quality Gates Architecture

<!--
Canonical Reference: .pi/architecture/modules/quality-gates.md
Rationale: Formal escalation of test quality levels — targeted → package → workspace → merge-ready
-->

## Overview

The Quality Gates system formalizes test quality into a four-tier escalation: `TargetedTests` → `Package` → `Workspace` → `MergeReady`. Each tier represents a broader scope of validation. The `GreenContract` pattern allows users and the orchestrator to declare a required quality level, and the engine verifies whether the observed test scope satisfies the contract.

This replaces the binary "tests passed" signal with a structured quality framework that gates merge/closeout decisions.

## Adoption Rationale

Rigorix currently treats test outcomes as binary (pass/fail). The Quality Gates system adds:

- **Scope-aware testing**: knows whether a node ran targeted tests, the full crate, or the whole workspace
- **Declarative quality contracts**: `GreenContract { required_level: Workspace }` means "I need workspace-level green"
- **Automatic gating**: orchestrator can refuse to close a task unless the contract is satisfied
- **Observability**: `QualityGateOutcome` provides structured evidence (not just pass/fail)
- **Integration with policy engine**: quality level feeds into policy rules for merge/closeout decisions

## Responsibilities

- Define four-tier quality level: TargetedTests, Package, Workspace, MergeReady
- Create GreenContract with required quality level
- Evaluate observed quality level against contract
- Track per-execution quality outcomes
- Integrate with enforcement to gate task closeout
- Emit quality gate events for audit trail

## Components

| Component | File Path | Purpose | Canonical Section |
|-----------|-----------|---------|-------------------|
| QualityLevel | `engine/src/quality/domain/level.rs` | Enum: TargetedTests, Package, Workspace, MergeReady | #level |
| GreenContract | `engine/src/quality/domain/contract.rs` | Required quality level + evaluation | #contract |
| QualityGateOutcome | `engine/src/quality/domain/outcome.rs` | Result: Satisfied or Unsatisfied with evidence | #outcome |
| QualityGateConfig | `engine/src/quality/domain/config.rs` | Per-task quality requirements | #config |
| QualityGateService | `engine/src/quality/application/service.rs` | Service trait: evaluate, classify_test_scope | #service |
| QualityGateEvent | `engine/src/quality/domain/event.rs` | Event payloads for gate evaluations | #event |

---

## Component Details

### QualityLevel

**Purpose:** Four-tier escalation of test scope

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityLevel {
    /// Only tests directly relevant to the change passed
    TargetedTests = 0,
    /// The crate/package tests passed
    Package = 1,
    /// Full workspace tests passed
    Workspace = 2,
    /// Workspace + all integration gates (lint, format, audit) passed
    MergeReady = 3,
}
```

**Level semantics:**
| Level | What must pass | Typical command |
|-------|---------------|-----------------|
| TargetedTests | Tests matching the changed files only | `cargo test -p crate -- specific::test` |
| Package | All tests in the affected crate | `cargo test -p crate` |
| Workspace | All tests across all crates | `cargo test --workspace` |
| MergeReady | Workspace + `cargo fmt --check`, `cargo clippy`, `cargo audit` | Full CI pipeline |

### GreenContract

**Purpose:** Declares required quality level and evaluates observed level

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GreenContract {
    /// The minimum quality level required
    pub required_level: QualityLevel,
}

impl GreenContract {
    pub fn new(required_level: QualityLevel) -> Self;

    /// Evaluate an observed quality level against this contract
    pub fn evaluate(&self, observed: Option<QualityLevel>) -> QualityGateOutcome {
        match observed {
            Some(level) if level >= self.required_level => QualityGateOutcome::Satisfied {
                required: self.required_level,
                observed: level,
            },
            Some(level) => QualityGateOutcome::Unsatisfied {
                required: self.required_level,
                observed: level,
                gap: self.required_level as i32 - level as i32,
            },
            None => QualityGateOutcome::Unsatisfied {
                required: self.required_level,
                observed: QualityLevel::TargetedTests, // lowest possible
                gap: self.required_level as i32,
            },
        }
    }
}
```

### QualityGateOutcome

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum QualityGateOutcome {
    Satisfied {
        required: QualityLevel,
        observed: QualityLevel,
    },
    Unsatisfied {
        required: QualityLevel,
        observed: QualityLevel,
        /// How many levels below requirement
        gap: i32,
    },
}
```

---

## Data Flow

```
Execution Engine runs tests
        │
        ▼
QualityGateService::classify_test_scope(task_node)
  - Did we run targeted tests? → TargetedTests
  - Did we run the whole crate? → Package
  - Did we run the workspace? → Workspace
  - Did lint/fmt/audit pass too? → MergeReady
        │
        ▼
observed: QualityLevel (e.g., Package)
        │
        ▼
GreenContract { required_level: Workspace }
        │
        ▼
contract.evaluate(Some(Package))
        │
        ▼
QualityGateOutcome::Unsatisfied {
    required: Workspace,
    observed: Package,
    gap: 1
}
        │
        ▼
Orchestrator decision:
  - gap > 0 && required == MergeReady → cannot merge
  - gap > 0 && required == Workspace → run broader tests
  - gap == 0 → satisfied → proceed to closeout
```

**Flow Description:**
1. Execution Engine runs tests as part of node execution
2. `QualityGateService::classify_test_scope()` determines what scope was tested
3. The task's `GreenContract` is evaluated against the observed level
4. If satisfied, the orchestrator can proceed with closeout
5. If unsatisfied, the orchestrator can request broader testing or escalate

---

## Dependencies

### Depends On
- **Execution Engine**: Runs tests and reports scope
- **Enforcement**: Gates task closeout based on quality outcome
- **Event System**: Emits quality gate events

### Used By
- **Orchestrator**: Evaluates quality gates before task closeout
- **Policy Engine**: `GreenAt { level }` condition for policy rules
- **Planning Pipeline**: Can set `GreenContract` in plan configuration

---

## Integration with Policy Engine

Quality gates feed directly into the Policy Engine:

```rust
// Policy rule: only closeout when workspace-green
PolicyRule::new(
    "closeout-requires-workspace-green",
    PolicyCondition::And(vec![
        PolicyCondition::LaneCompleted,
        PolicyCondition::GreenAt { level: QualityLevel::Workspace as u8 },
    ]),
    PolicyAction::CloseoutLane,
    priority: 10,
)
```

---

## Configuration

```toml
# .rigorix/quality.toml
[quality]
# Default required level for all tasks
default_required_level = "package"

# Per-template overrides
[quality.templates.refactor]
required_level = "workspace"

[quality.templates.hotfix]
required_level = "merge_ready"
```

---

## Testing Requirements

| Test Type | Coverage Target | Files |
|-----------|-----------------|-------|
| Unit | 95% | `engine/src/quality/` — per-component test modules |

**Key Test Scenarios:**
- Observed `Workspace` against required `Package` → `Satisfied`
- Observed `TargetedTests` against required `Workspace` → `Unsatisfied { gap: 2 }`
- Observed `None` against any requirement → `Unsatisfied`
- `QualityLevel` ordering: `TargetedTests < Package < Workspace < MergeReady`

---

*Last updated: 2026-06-19*
*Module version: 1.0.0 (Planned)*

---

**Status:** Planned  
**Implementation priority:** P0 — structural quality enforcement

## The level's subject (#1585)

`QualityLevel` has four tiers and one classifier. Until #1585 the classifier took six booleans, and both
production callers wrote them from control flow rather than from an outcome:

| producer | what it claimed |
|---|---|
| `plan_validation::factory_impl` | `targeted_tests_run: true`, `workspace_tests_run: input.iteration > 2` |
| `orchestrator_impl` (§6 and the legacy §7a) | `workspace_tests_run: final_status != ExecutionStatus::Failed` |

`lint_passed` was the literal `false` at both sites, so `MergeReady` was unreachable by construction, and
"workspace tests passed" was an assertion about the loop's own bookkeeping. Any `source: Engine` stamped
from either would have put a counter's arithmetic into a signed record.

`ClassifyTestScopeInput` now carries `Vec<RecordedOutcome>` — the step, the tool it ran, what it ran
(`subject`), and whether it reported success — and `classify_from_outcomes` derives the tier from the
passing outcomes only, naming the steps it used. A failure never raises a tier and is named separately.

`ClassifyTestScopeOutput` gains two fields that decide whether the value may travel as an engine claim:

- `source: ClaimSource` — `Engine` only when the level came from recorded outcomes; `Caller` when there
  were none. A validation loop executes no plan step, so it supplies nothing and its claim is `Caller`.
- `evidence: Vec<String>` — the step ids that decided the tier. Empty exactly when the claim is the floor.

The orchestrator joins the graph (tool + serialized `intent`) with the executed results (status) in
`recorded_outcomes`, and carries the classification out on `RunOutput.quality` instead of dropping it into
a `tracing::info!`. A step whose command was not recorded yields `CheckKind::Other` rather than a guess:
"no `--workspace` and no `-p`" is read as the narrower scope, because an unrecognised argv cannot raise
the level.
