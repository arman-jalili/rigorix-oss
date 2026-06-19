# Policy Engine Architecture

<!--
Canonical Reference: .pi/architecture/modules/policy-engine.md
Blueprint Source: claw-code-parity analysis (2026-06-19)
Rationale: Declarative, configurable policy rules with priority ordering — replaces hardcoded enforcement chains
-->

## Overview

The Policy Engine evaluates declarative `PolicyRule`s against a typed execution context (`LaneContext`) and produces a flat list of actions in priority order. Rules combine boolean conditions (And/Or) over observable state — quality level, branch freshness, review status, completion state — and map them to executable actions like merge, closeout, escalate, or reconcile.

This replaces hardcoded if-else enforcement chains with **user-configurable policy rules** that can be loaded from `.rigorix/policy.toml`.

## Adoption Rationale

Rigorix currently has enforcement logic embedded in the `ExecutionEngine` and `Orchestrator`. The Policy Engine separates policy from mechanism:

- **Declarative rules**: policy is data, not code — users can configure enforcement behavior
- **Priority ordering**: rules evaluate in priority order, earliest matching rule wins
- **Composable conditions**: `And`/`Or` over observable state
- **Auditable**: every policy evaluation produces structured outcomes
- **Extensible**: new conditions and actions can be added without changing the engine
- **Integration with quality gates**: `GreenAt { level }` condition bridges policy to quality gates

## Responsibilities

- Define PolicyRule: name, condition, action, priority
- Support composable conditions: And, Or, GreenAt, StaleBranch, LaneCompleted, ReviewPassed, ScopedDiff, TimedOut
- Support executable actions: Merge, Closeout, Recover, Escalate, Reconcile, Notify, Block, Cleanup, Chain
- Evaluate rules against LaneContext (typed execution state)
- Return ordered action list for the orchestrator to execute
- Support rule loading from config (TOML)
- Emit policy evaluation events

## Components

| Component | File Path | Purpose | Canonical Section |
|-----------|-----------|---------|-------------------|
| PolicyRule | `engine/src/policy/domain/rule.rs` | Rule: name + condition + action + priority | #rule |
| PolicyCondition | `engine/src/policy/domain/condition.rs` | Composable conditions: And, Or, GreenAt, StaleBranch, etc. | #condition |
| PolicyAction | `engine/src/policy/domain/action.rs` | Action enum: Merge, Closeout, Recover, Escalate, etc. | #action |
| LaneContext | `engine/src/policy/domain/context.rs` | Typed execution state evaluated by conditions | #context |
| PolicyEngine | `engine/src/policy/application/engine.rs` | Evaluates rules against context, returns actions | #engine |
| PolicyConfig | `engine/src/policy/domain/config.rs` | User-configurable rule definitions (TOML) | #config |
| PolicyConfigLoader | `engine/src/policy/infrastructure/loader.rs` | Loads rules from `.rigorix/policy.toml` | #loader |
| PolicyEvent | `engine/src/policy/domain/event.rs` | Events: RuleMatched, ActionsDispatched | #event |

---

## Component Details

### PolicyRule

**Purpose:** A single policy rule with priority

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyRule {
    pub name: String,
    pub condition: PolicyCondition,
    pub action: PolicyAction,
    /// Lower numbers = higher priority. Evaluated in ascending order.
    pub priority: u32,
}
```

### PolicyCondition

**Purpose:** Composable conditions evaluated against LaneContext

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PolicyCondition {
    /// All sub-conditions must match
    And(Vec<PolicyCondition>),
    /// Any sub-condition must match
    Or(Vec<PolicyCondition>),
    /// Quality gate at or above the given level
    GreenAt { level: u8 },
    /// Branch has been stale beyond threshold
    StaleBranch,
    /// Lane is blocked at startup
    StartupBlocked,
    /// Lane execution completed
    LaneCompleted,
    /// Lane has been reconciled
    LaneReconciled,
    /// Review has been approved
    ReviewPassed,
    /// Diff is scoped (not full-repo)
    ScopedDiff,
    /// Branch has been untouched for duration
    TimedOut { duration_secs: u64 },
}
```

### PolicyAction

**Purpose:** What the engine does when a rule matches

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyAction {
    /// Merge the lane branch into dev/main
    MergeToDev,
    /// Merge forward (dev → main or similar)
    MergeForward,
    /// Attempt recovery once
    RecoverOnce,
    /// Escalate with a specific reason
    Escalate { reason: String },
    /// Close out the lane (cleanup resources)
    CloseoutLane,
    /// Cleanup session state
    CleanupSession,
    /// Reconcile the lane (no merge needed — already merged, empty diff, etc.)
    Reconcile { reason: ReconcileReason },
    /// Send notification to a channel
    Notify { channel: String },
    /// Block the lane with a reason
    Block { reason: String },
    /// Execute multiple actions in sequence
    Chain(Vec<PolicyAction>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconcileReason {
    AlreadyMerged,
    Superseded,
    EmptyDiff,
    ManualClose,
}
```

### LaneContext

**Purpose:** Typed snapshot of execution state evaluated by conditions

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaneContext {
    pub lane_id: String,
    pub green_level: u8,            // QualityLevel as u8
    pub branch_freshness_secs: u64, // Time since last commit
    pub blocker: LaneBlocker,
    pub review_status: ReviewStatus,
    pub diff_scope: DiffScope,
    pub completed: bool,
    pub reconciled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneBlocker { None, Startup, External }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewStatus { Pending, Approved, Rejected }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffScope { Full, Scoped }
```

### PolicyEngine

**Purpose:** Evaluates rules against context, returns flat action list

```rust
pub struct PolicyEngine {
    rules: Vec<PolicyRule>,
}

impl PolicyEngine {
    pub fn new(rules: Vec<PolicyRule>) -> Self;

    /// Evaluate all matching rules and return actions in priority order
    pub fn evaluate(&self, context: &LaneContext) -> Vec<PolicyAction> {
        let mut actions = Vec::new();

        for rule in &self.rules {
            if rule.condition.matches(context) {
                rule.action.flatten_into(&mut actions);
            }
        }

        actions
    }
}
```

---

## Data Flow

```
Execution completes
        │
        ▼
LaneContext built from:
  - ExecutionRecord (completed, green_level)
  - Git state (branch_freshness, diff_scope)
  - Risk Gating (blocker)
  - Review state (review_status)
        │
        ▼
PolicyEngine::evaluate(&context)
        │
        ▼
For each rule (priority order):
  ├─ "closeout-completed-lane" (priority 10)
  │   Condition: And(LaneCompleted, GreenAt(3))
  │   Action: CloseoutLane
  │   Matches? → yes → add CloseoutLane to actions
  │
  ├─ "cleanup-completed-session" (priority 5)
  │   Condition: LaneCompleted
  │   Action: CleanupSession
  │   Matches? → yes → add CleanupSession to actions
  │
  ├─ "stale-branch-warning" (priority 20)
  │   Condition: StaleBranch
  │   Action: Block { reason: "branch is stale" }
  │   Matches? → no (freshness < threshold)
  │
  └─ "escalate-blocked" (priority 1)
      Condition: StartupBlocked
      Action: Escalate { reason: "startup blocked" }
      Matches? → no
        │
        ▼
Actions: [CloseoutLane, CleanupSession]
        │
        ▼
Orchestrator executes actions:
  1. CloseoutLane → cleanup task resources
  2. CleanupSession → prune session data
```

**Flow Description:**
1. After execution completes, a `LaneContext` is built from all observable state
2. `PolicyEngine::evaluate()` iterates rules in priority order
3. Each rule's condition is evaluated against the context
4. Matching rules contribute their actions (flattened from `Chain`)
5. The orchestrator executes the resulting action list

---

## Dependencies

### Depends On
- **Quality Gates**: `GreenAt` condition reads `QualityLevel` from context
- **Execution Engine**: Provides completion state for `LaneCompleted`
- **Risk Gating**: Provides blocker state for `StartupBlocked`
- **Event System**: Emits policy evaluation events

### Used By
- **Orchestrator**: Evaluates policy after execution, before closeout
- **Enforcement**: Can query policy engine for gating decisions
- **TUI**: Displays pending policy actions to the user

---

## Configuration

```toml
# .rigorix/policy.toml
[[rules]]
name = "closeout-completed-lane"
condition = { type = "and", conditions = [
    { type = "lane_completed" },
    { type = "green_at", level = 3 }
]}
action = { type = "chain", actions = [
    { type = "closeout_lane" },
    { type = "notify", channel = "discord" }
]}
priority = 10

[[rules]]
name = "stale-branch-block"
condition = { type = "stale_branch" }
action = { type = "block", reason = "Branch is stale — rebase required" }
priority = 20

[[rules]]
name = "reconcile-empty-diff"
condition = { type = "and", conditions = [
    { type = "lane_completed" },
    { type = "scoped_diff" }
]}
action = { type = "reconcile", reason = "empty_diff" }
priority = 15
```

---

## Testing Requirements

| Test Type | Coverage Target | Files |
|-----------|-----------------|-------|
| Unit | 95% | `engine/src/policy/` — per-component test modules |

**Key Test Scenarios:**
- `And` condition with all sub-conditions true → matches
- `And` condition with one sub-condition false → doesn't match
- `Or` condition with one sub-condition true → matches
- Rules evaluated in priority order (lower number = higher priority)
- `Chain` action flattens to individual actions
- Stale branch exceeding threshold → `StaleBranch` matches
- Green level below threshold → `GreenAt` doesn't match
- Multiple matching rules → all actions collected

---

*Last updated: 2026-06-19*
*Module version: 1.0.0 (Planned)*
*Adopted from: claw-code-parity analysis — policy_engine.rs (581 LOC)*

---

**Status:** Planned  
**Blueprint Source:** claw-code-parity pattern analysis  
**Implementation priority:** P1 — configurable enforcement
