# Policy Engine Runbook

## Overview

The Policy Engine evaluates declarative `PolicyRule`s against a typed execution context (`LaneContext`) and produces a flat list of actions in priority order. It replaces hardcoded if-else enforcement chains with user-configurable rules loaded from `.rigorix/policy.toml`.

## Architecture

```
Config (.rigorix/policy.toml)
       │
       ▼
PolicyRepository (load_config)
       │
       ▼
PolicyEngineFactory (create_from_config)
       │
       ▼
PolicyEngineService (evaluate)
       │
       ▼
Orchestrator (executes actions)
```

## Startup Sequence

1. **Configuration Loading**: On startup, the `PolicyEngineFactoryImpl` loads rules from:
   - `PolicyConfig` (programmatic) — via `create_from_config()`
   - `DefaultPolicyRepository` (file-based) — via `create_with_repository()`
   - Default rules — via `create_default()` (4 built-in rules)

2. **Initialization**: The factory returns a `PolicyEngineServiceImpl` with rules loaded in memory, sorted by priority.

3. **Readiness**: The engine is ready immediately after construction. `has_rules()` returns `true` once rules are loaded.

## Common Operations

### Rule Evaluation
```
PolicyEngineService::evaluate(EvaluatePolicyInput {
    context: LaneContext { ... },
    rule_filter: None,  // or specific rule names
})
```
Returns `EvaluatePolicyOutput` with matched actions in priority order.

### Rule Loading
```
PolicyEngineService::load_rules(LoadRulesInput {
    config: PolicyConfig { rules: [...] },
    replace_all: true,  // replaces all existing rules
})
```

### Rule Querying
```
PolicyEngineService::get_active_rules()  // returns all rules sorted by priority
PolicyEngineService::rule_count()        // returns number of loaded rules
```

## Configuration

### Default Configuration (`create_default()`)
| Rule Name | Condition | Action | Priority |
|-----------|-----------|--------|----------|
| closeout-completed-lane | LaneCompleted AND GreenAt(3) | CloseoutLane | 10 |
| cleanup-completed-session | LaneCompleted | CleanupSession | 20 |
| reconcile-empty-diff | LaneCompleted AND ScopedDiff | Reconcile(EmptyDiff) | 15 |
| escalate-startup-blocked | StartupBlocked | Escalate("blocked") | 5 |

### User Configuration (`.rigorix/policy.toml`)
```toml
[[rules]]
name = "custom-rule"
condition = { type = "lane_completed" }
action = { type = "closeout_lane" }
priority = 10
```

## Common Failure Modes

| Failure | Symptom | Recovery |
|---------|---------|----------|
| No rules loaded | `evaluate()` returns empty actions | Call `load_rules()` or recreate engine via factory |
| Invalid TOML config | `RepositoryError` / `DeserializationError` | Fix `.rigorix/policy.toml` syntax |
| File not found | `InvalidConfiguration` | Use defaults — optional config |
| Empty config | Factory returns error | Use `create_default()` instead |
| Lock contention | `InvalidState` (lock error) | Engine uses RwLock — should not occur in single-threaded orchestrator |

## Dependencies

- **Quality Gates**: `GreenAt` condition reads quality level from context
- **Execution Engine**: Provides completion state for `LaneCompleted`
- **Risk Gating**: Provides blocker state for `StartupBlocked`
- **Event System**: Emits `PolicyEvent` on evaluation

## Related Components

- `Orchestrator` — calls `evaluate()` after execution, dispatches actions
- `Enforcement` — enforces execution limits (resource budgets)
- `Permission` — gates tool calls by permission mode

## Graceful Shutdown

The Policy Engine is stateless (rules are in-memory). On shutdown:
1. No special cleanup needed
2. Pending evaluations will be interrupted
3. Rules are re-loaded from config on next startup
