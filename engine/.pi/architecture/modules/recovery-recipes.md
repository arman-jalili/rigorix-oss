# Recovery Recipes Architecture

<!--
Canonical Reference: .pi/architecture/modules/recovery-recipes.md
Blueprint Source: claw-code-parity analysis (2026-06-19)
Rationale: Automatic recovery before escalation — known failure modes auto-heal once before human intervention
-->

## Overview

The Recovery Recipes system encodes known failure scenarios and their automatic recovery procedures. The core rule is: **one automatic recovery attempt per scenario before human escalation.** Each failure scenario has a `RecoveryRecipe` — a sequence of recovery steps with a maximum attempt count and an escalation policy for when attempts are exhausted.

This transforms Rigorix's DR plans from prose documentation into **executable code** — the engine can actually run `cargo clean && cargo build` on a compile failure instead of just documenting that as a recovery procedure.

## Adoption Rationale

Rigorix's `failure_classification` module already classifies failures and selects retry strategies. The Recovery Recipes system extends this with:

- **Known recovery procedures encoded as data**, not hardcoded in the retry engine
- **One-attempt-before-escalation discipline** — prevents infinite recovery loops
- **Structured recovery events** for audit trail and observability
- **Per-scenario attempt tracking** — the engine knows it already tried a `clean_build` and shouldn't try again
- **Bridging to worker failures** — `FailureScenario` maps from `WorkerFailureKind` for coherent error handling
- **Human escalation as a first-class policy** — not an afterthought

## Responsibilities

- Define known failure scenarios: CompileError, TestFailure, ToolConnectionError, ProviderFailure, PartialInitialization, AuthorizationError, StaleBranch
- Encode recovery recipes per scenario: ordered steps, max attempts, escalation policy
- Track per-scenario attempt counts within an execution session
- Execute recovery steps: CleanBuild, RetryWithContext, RestartService, RebaseBranch, AcceptTrust, EscalateToHuman
- Emit structured recovery events: RecoveryAttempted, RecoverySucceeded, RecoveryFailed, Escalated
- Integrate with failure_classification to select recipes by FailureType
- Enforce one-attempt policy before escalation

## Components

| Component | File Path | Purpose | Canonical Section |
|-----------|-----------|---------|-------------------|
| FailureScenario | `engine/src/recovery/domain/scenario.rs` | Enum of known recoverable failure scenarios | #scenario |
| RecoveryStep | `engine/src/recovery/domain/step.rs` | Enum of executable recovery actions | #step |
| RecoveryRecipe | `engine/src/recovery/domain/recipe.rs` | Scenario + steps + max_attempts + escalation_policy | #recipe |
| EscalationPolicy | `engine/src/recovery/domain/escalation.rs` | Enum: AlertHuman, LogAndContinue, Abort | #escalation |
| RecoveryResult | `engine/src/recovery/domain/result.rs` | Outcome: Recovered, PartialRecovery, EscalationRequired | #result |
| RecoveryEvent | `engine/src/recovery/domain/event.rs` | Event payloads: RecoveryAttempted, RecoverySucceeded, RecoveryFailed, Escalated | #event |
| RecoveryContext | `engine/src/recovery/application/context.rs` | Per-session attempt tracker + event log | #context |
| RecoveryRecipeRepository | `engine/src/recovery/infrastructure/recipe_repo.rs` | Repository of known recipes (configurable) | #repo |
| RecoveryService | `engine/src/recovery/application/service.rs` | Service trait: attempt_recovery, recipe_for | #service |
| RecoveryServiceImpl | `engine/src/recovery/application/service_impl.rs` | Implements recovery dispatch and step execution | #impl |

---

## Component Details

### FailureScenario

**Purpose:** Typed enumeration of all known recoverable failure scenarios

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureScenario {
    /// Build failure (cargo build, npm build, etc.)
    CompileError,
    /// Test failure (non-compile)
    TestFailure,
    /// Tool connection failure (LSP, MCP, external service)
    ToolConnectionError,
    /// LLM provider API failure (rate limit, timeout, 5xx)
    ProviderFailure,
    /// Partial initialization (some components started, some didn't)
    PartialInitialization,
    /// Authorization failure (trust prompt, API key, permission)
    AuthorizationError,
    /// Branch is stale relative to main
    StaleBranch,
}

impl FailureScenario {
    /// Map from FailureType (failure_classification) to FailureScenario
    pub fn from_failure_type(ft: &FailureType) -> Option<Self> {
        match ft {
            FailureType::BuildError => Some(Self::CompileError),
            FailureType::TestFailure => Some(Self::TestFailure),
            FailureType::ToolConnectionError => Some(Self::ToolConnectionError),
            FailureType::LlmApiError => Some(Self::ProviderFailure),
            FailureType::PartialInitialization => Some(Self::PartialInitialization),
            FailureType::AuthorizationError => Some(Self::AuthorizationError),
            FailureType::StaleBranch => Some(Self::StaleBranch),
            _ => None, // unknown failure types have no recipe
        }
    }
}
```

### RecoveryStep

**Purpose:** Individual executable recovery actions

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryStep {
    /// Run clean build (cargo clean && cargo build)
    CleanBuild,
    /// Retry with expanded context (more file reads, broader search)
    ExpandContext,
    /// Retry tool connection with timeout
    RetryConnection { timeout_ms: u64 },
    /// Restart an external service by name
    RestartService { name: String },
    /// Rebase branch onto main
    RebaseBranch,
    /// Auto-accept trust prompt (for known repos)
    AcceptTrust,
    /// Restart the worker/executor
    RestartWorker,
    /// Escalate to human with reason
    EscalateToHuman { reason: String },
}
```

### RecoveryRecipe

**Purpose:** Binds a scenario to its recovery steps and escalation policy

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryRecipe {
    pub scenario: FailureScenario,
    /// Ordered sequence of recovery steps to attempt
    pub steps: Vec<RecoveryStep>,
    /// Maximum automatic recovery attempts for this scenario (1 = try once)
    pub max_attempts: u32,
    /// What to do when attempts are exhausted
    pub escalation_policy: EscalationPolicy,
}
```

**Built-in Recipe Catalog:**

| Scenario | Steps | Max Attempts | Escalation |
|----------|-------|-------------|------------|
| CompileError | CleanBuild → Retry with ExpandContext | 1 | AlertHuman |
| TestFailure | ExpandContext | 1 | AlertHuman |
| ToolConnectionError | RetryConnection(30s) → RestartService | 2 | AlertHuman |
| ProviderFailure | RetryConnection(10s) → RetryConnection(60s) | 2 | AlertHuman |
| PartialInitialization | RestartWorker | 1 | AlertHuman |
| AuthorizationError | AcceptTrust | 1 | AlertHuman |
| StaleBranch | RebaseBranch | 1 | LogAndContinue |

### RecoveryContext

**Purpose:** Tracks per-scenario attempt counts and recovery events within an execution session

```rust
#[derive(Debug, Clone)]
pub struct RecoveryContext {
    /// Per-scenario attempt count (reset per execution session)
    attempts: HashMap<FailureScenario, u32>,
    /// Ordered recovery event log
    events: Vec<RecoveryEvent>,
}

impl RecoveryContext {
    pub fn new() -> Self;

    /// Check if scenario has remaining attempts
    pub fn can_attempt(&self, scenario: FailureScenario, recipe: &RecoveryRecipe) -> bool {
        self.attempts.get(&scenario).unwrap_or(&0) < &recipe.max_attempts
    }

    /// Record an attempt (increments counter, emits event)
    pub fn record_attempt(&mut self, scenario: FailureScenario);

    /// Get all events for audit trail
    pub fn events(&self) -> &[RecoveryEvent];
}
```

### RecoveryResult

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryResult {
    /// All steps completed successfully
    Recovered { steps_taken: u32 },
    /// Some steps succeeded, some couldn't run
    PartialRecovery {
        recovered: Vec<RecoveryStep>,
        remaining: Vec<RecoveryStep>,
    },
    /// Attempts exhausted — escalation required
    EscalationRequired { reason: String },
}
```

---

## Data Flow

```
Execution Engine
    │
    ▼
Node execution fails
    │
    ▼
FailureClassification::classify(error)
    │
    ▼
FailureType (e.g., BuildError)
    │
    ▼
FailureScenario::from_failure_type(BuildError) → CompileError
    │
    ▼
RecoveryRecipeRepository::recipe_for(CompileError)
    │
    ▼
RecoveryRecipe { steps: [CleanBuild, ExpandContext], max_attempts: 1 }
    │
    ▼
RecoveryContext::can_attempt(CompileError, &recipe)?
    ├── yes → attempt_recovery(recipe)
    │           │
    │           ▼
    │        Execute steps sequentially:
    │          1. CleanBuild → cargo clean && cargo build
    │          2. ExpandContext → re-read files, retry
    │           │
    │           ▼
    │        RecoveryResult::Recovered { steps_taken: 1 }
    │           │
    │           ▼
    │        Emit RecoveryEvent::RecoverySucceeded
    │           │
    │           ▼
    │        Re-execute the failed node
    │
    └── no → RecoveryResult::EscalationRequired { reason: "max attempts reached" }
                │
                ▼
             EscalationPolicy::AlertHuman
                │
                ▼
             Emit RecoveryEvent::Escalated
                │
                ▼
             Mark node as Failed (terminal)
```

**Flow Description:**
1. Node execution fails, `FailureClassification` produces a `FailureType`
2. `FailureScenario::from_failure_type()` maps to a known scenario (or returns `None` for unknown)
3. `RecoveryRecipeRepository` looks up the recipe for the scenario
4. `RecoveryContext` checks if this scenario still has attempts remaining
5. If yes, execute recovery steps sequentially, track result, re-execute node
6. If no, escalate per policy — `AlertHuman` (notify), `LogAndContinue` (skip), or `Abort` (terminate execution)
7. All events recorded for audit trail

---

## Dependencies

### Depends On
- **Failure Classification**: `FailureType` → `FailureScenario` mapping
- **Execution Engine**: Re-execute nodes after recovery
- **Event System**: Emit `RecoveryEvent` types
- **Cancellation**: Abort recovery if execution is cancelled

### Used By
- **Execution Engine**: Integrates recovery into node execution retry loop
- **Orchestrator**: Constructs `RecoveryContext` per execution session

---

## Integration with Execution Engine

```rust
// Inside ExecutionEngine retry loop
async fn execute_with_recovery(
    node: &TaskNode,
    retry_policy: &RetryPolicy,
    recovery_ctx: &mut RecoveryContext,
    recipe_repo: &RecoveryRecipeRepository,
) -> Result<TaskResult, ExecutionError> {
    let mut attempt = 0;

    loop {
        attempt += 1;
        match execute_node(node).await {
            Ok(result) => return Ok(result),
            Err(error) => {
                // 1. Classify
                let failure_type = FailureClassifier::classify(&error);

                // 2. Map to scenario
                let Some(scenario) = FailureScenario::from_failure_type(&failure_type) else {
                    return Err(error); // Unknown failure — no recipe
                };

                // 3. Look up recipe
                let Some(recipe) = recipe_repo.recipe_for(scenario) else {
                    return Err(error); // No recipe configured
                };

                // 4. Check attempts
                if !recovery_ctx.can_attempt(scenario, &recipe) {
                    recovery_ctx.record_escalation(scenario, &recipe);
                    return Err(error); // Attempts exhausted
                }

                // 5. Execute recovery
                let result = execute_recovery_steps(&recipe.steps).await;
                recovery_ctx.record_attempt(scenario, &recipe, &result);

                match result {
                    RecoveryResult::Recovered { .. } => continue, // Re-execute node
                    RecoveryResult::PartialRecovery { .. } => continue, // Try with partial recovery
                    RecoveryResult::EscalationRequired { .. } => {
                        match recipe.escalation_policy {
                            EscalationPolicy::AlertHuman => notify_human(&scenario),
                            EscalationPolicy::Abort => return Err(error),
                            EscalationPolicy::LogAndContinue => continue,
                        }
                    }
                }
            }
        }
    }
}
```

---

## Security Considerations

| Concern | Mitigation | Validator |
|---------|------------|-----------|
| Recovery step causes more damage | Steps are predefined in config — no arbitrary command execution | security-validator |
| Infinite recovery loop | `max_attempts` per scenario; `RecoveryContext` tracks counts; escalation after exhaustion | security-validator |
| Recovery runs with elevated permissions | Recovery steps execute under same permission mode as the failed node | security-validator |

---

## Testing Requirements

| Test Type | Coverage Target | Files |
|-----------|-----------------|-------|
| Unit | 90% | `engine/src/recovery/` — per-component test modules |
| Integration | 85% | Simulated failure + recovery cycles |
| E2E | 80% | Live build failure → clean build → re-execute |

**Key Test Scenarios:**
- CompileError triggers `CleanBuild` → build succeeds → node re-executes
- CompileError triggers `CleanBuild` → still fails → escalation to human
- Unknown failure type → no recipe → returns original error
- Scenario at max attempts → immediate escalation
- RecoveryContext tracks attempts correctly across multiple failures
- EscalationPolicy::Abort terminates execution
- RecoveryEvent emitted for every attempt

---

## Error Handling

```rust
#[derive(Debug, Error)]
pub enum RecoveryError {
    #[error("No recipe for scenario: {0}")]
    NoRecipe(FailureScenario),
    #[error("Max recovery attempts reached for {0}")]
    MaxAttemptsReached(FailureScenario),
    #[error("Recovery step failed: {step:?} — {reason}")]
    StepFailed { step: RecoveryStep, reason: String },
    #[error("Recovery aborted by cancellation signal")]
    Aborted,
}
```

---

*Last updated: 2026-06-19*
*Module version: 1.0.0 (Planned)*
*Adopted from: claw-code-parity analysis — recovery_recipes.rs (630 LOC), lane_events.rs*

---

**Status:** Planned  
**Blueprint Source:** claw-code-parity pattern analysis  
**Implementation priority:** P0 — extends failure_classification with automatic recovery
