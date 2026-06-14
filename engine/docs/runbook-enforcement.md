# Runbook: enforcement Module

<!--
Canonical Reference: .pi/architecture/modules/enforcement.md
Last Updated: 2026-06-13
-->

## Overview

The `enforcement` module provides runtime enforcement of safety limits during
execution. It gates every tool call against configured policies, tracks resource
budgets (tokens, tool calls, execution time), and checks execution hard limits.
The `ExecutionEnforcer` sits between the executor and tool execution, ensuring
no action exceeds configured bounds without explicit approval.

## Components

| Component | Type | Description |
|-----------|------|-------------|
| `EnforcementConfig` | Domain entity | Resource budgets, execution limits, tool policies, preset profiles |
| `ExecutionEnforcerImpl` | Application service | Runtime enforcer — gates tool calls, tracks budgets, checks limits |
| `ExecutionEnforcerFactoryImpl` | Factory | Constructs enforcer instances from config presets with overrides |
| `DefaultPolicyRepository` | Repository | In-memory policy store with runtime tool policy overrides |
| `ConfigBuilder` | Service | Builds and validates enforcement configs from preset profiles |

### Preset Profiles

| Preset | Tokens | Tool Calls | Execution Time | Bash | Write | Read |
|--------|--------|------------|----------------|------|-------|------|
| Standard | 100K | 500 | 15 min | Allowed + Confirm | Allowed | Allowed |
| Strict | 50K | 200 | 10 min | Allowed + Confirm | Confirm | Allowed |
| Maximum | 20K | 50 | 10 min | Blocked | Dry-run | Allowed |

## Startup Sequence

### Dependencies

| Dependency | Required | Description |
|------------|----------|-------------|
| tokio runtime | Yes | Async runtime for RwLock and async trait methods |
| serde | Yes | Config serialization/deserialization |
| chrono | Yes | Warning timestamps (ISO 8601 UTC) |

### Initialization

1. Select a preset profile (`Standard`, `Strict`, or `Maximum`)
2. Create an `EnforcementConfig` using `EnforcementConfig::standard()`, `::strict()`, or `::maximum()`
3. (Optional) Apply custom overrides using builder methods: `.with_budget()`, `.with_tool_policy()`
4. Validate the config against safety caps: `config.validate(&SafetyCaps::default())`
5. Create the enforcer via factory or directly:

```rust
use rigorix::enforcement::application::*;
use rigorix::enforcement::domain::*;

// Using factory
let factory = ExecutionEnforcerFactoryImpl;
let enforcer = factory.create_default("exec-1").await.unwrap();

// Using direct construction with custom config
let config = EnforcementConfig::strict()
    .with_budget(ResourceBudget {
        resource: "tokens".to_string(),
        soft_warning_threshold: 0.8,
        hard_limit: 100_000,
        current_usage: 0,
    });
let enforcer = ExecutionEnforcerImpl::new("exec-1", config);
```

### Quick Start

```rust
use rigorix::enforcement::application::*;
use rigorix::enforcement::domain::*;

// Create enforcer with custom config
let config = EnforcementConfig::standard();
let enforcer = ExecutionEnforcerImpl::new("exec-1", config);

// Evaluate a tool call
let eval = enforcer.evaluate_tool_call(EvaluateToolCallInput {
    execution_id: "exec-1".to_string(),
    node_id: "node-1".to_string(),
    tool: "bash".to_string(),
    arguments: None,
    is_retry: false,
    attempt: 1,
}).await.unwrap();

if eval.allowed {
    // Execute the tool...
    // Track resource consumption afterward
    enforcer.track_resource_usage(TrackResourceUsageInput {
        execution_id: "exec-1".to_string(),
        resource: "tool_calls".to_string(),
        amount: 1,
        context: None,
    }).await.unwrap();
}
```

## Graceful Shutdown

### Normal Shutdown

1. Before shutdown, check for active warnings:
   ```rust
   if enforcer.has_active_warnings() {
       let warnings = enforcer.active_warnings();
       // Log or report warnings before shutdown
   }
   ```
2. Check execution limits:
   ```rust
   let limits = enforcer.check_execution_limits(
       CheckExecutionLimitsInput { execution_id: "exec-1".to_string() }
   ).await?;
   ```
3. Drop the `ExecutionEnforcerImpl` — all resources are cleaned up automatically
4. The `RwLock` guards are dropped, freeing the budget state

### Forced Shutdown

If the process terminates without cleanup:
- In-memory budget state is lost (acceptable for stateless executions)
- Enforcement state will be reset on next initialization
- No persistent side effects from enforcement state

## Common Failure Modes

### Tool Call Blocked

**Symptom:** `evaluate_tool_call()` returns `allowed: false`.

**Causes:**
1. **Tool policy blocks the tool** — The tool is not allowed by the current preset or policy override
2. **Budget exhausted** — The associated resource budget has reached its hard limit
3. **Execution limit reached** — Max tool calls or other limit has been hit

**Resolution:**
1. Check the `reason` field in the evaluation output for the specific cause
2. If budget exhausted: increase budget limits or reduce tool usage
3. If policy blocked: switch to a less restrictive preset or add a tool policy override
4. If execution limit reached: the execution should be terminated

### Budget Warning

**Symptom:** `has_active_warnings()` returns `true`.

**Cause:** A resource budget has crossed its soft warning threshold but not yet reached its hard limit.

**Resolution:**
1. `active_warnings()` lists all resources near their limits
2. This is informational — execution continues normally
3. Monitor warning count growth; if multiple resources are warning, consider terminating early

### Budget Not Found

**Symptom:** `track_resource_usage()` returns `EnforcementError::BudgetNotFound`.

**Cause:** The resource being tracked does not exist in the configuration.

**Resolution:**
1. Verify the resource name matches a key in `config.budgets`
2. Standard budgets: "tokens", "tool_calls", "execution_time_ms"
3. Add the missing budget via `config.with_budget()` before creating the enforcer

### Invalid Configuration

**Symptom:** Factory returns `EnforcementError::InvalidConfiguration`.

**Cause:** The config values exceed the safety caps.

**Resolution:**
1. Run `config.validate(&SafetyCaps::default())` to get a list of validation errors
2. Adjust config values to be within caps
3. Use the preset builders (`standard()`, `strict()`, `maximum()`) which are guaranteed valid

### Concurrent Access Contention

**Symptom:** `RwLock` poisoning due to a panic while holding the write lock.

**Cause:** A panic occurred inside a write-locked section.

**Resolution:**
1. This should not happen in normal operation — the enforcer handles errors gracefully
2. If it occurs, recreate the enforcer with `ExecutionEnforcerFactoryImpl`
3. Report the bug with a stack trace

## Configuration Reference

### EnforcementConfig Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `budgets` | `HashMap<String, ResourceBudget>` | 3 standard budgets | Named resource budgets with soft thresholds and hard limits |
| `execution_limits` | `ExecutionLimits` | Standard defaults | Hard limits for tool calls, time, tokens, retries, concurrency |
| `tool_policies` | `HashMap<String, ToolPolicy>` | 3 standard policies | Per-tool policy overrides |
| `default_tool_policy` | `ToolPolicy` | Medium risk, allowed | Fallback policy for tools without specific override |
| `preset` | `EnforcementPresetProfile` | `Standard` | The preset profile that generated this config |

### ResourceBudget Fields

| Field | Type | Description |
|-------|------|-------------|
| `resource` | `String` | Resource name (e.g., "tokens", "tool_calls") |
| `soft_warning_threshold` | `f64` | Fraction (0.0–1.0) at which warning is emitted |
| `hard_limit` | `u64` | Absolute maximum; tool blocked when exceeded |
| `current_usage` | `u64` | Current runtime usage (mutated by enforcer) |

### ToolPolicy Fields

| Field | Type | Description |
|-------|------|-------------|
| `allowed` | `bool` | Whether the tool can execute at all |
| `risk_level` | `ToolRiskLevel` | Low, Medium, High, or Critical |
| `requires_confirmation` | `bool` | User must confirm before execution |
| `dry_run` | `bool` | Execute without side effects |
| `max_calls` | `Option<u64>` | Maximum calls allowed (None = unlimited) |
| `budget_key` | `Option<String>` | References a budget for tracking |

## Performance Characteristics

| Metric | Target | Notes |
|--------|--------|-------|
| Tool evaluation latency | < 10µs | Synchronous RwLock read; no IO |
| Resource tracking latency | < 10µs | Synchronous RwLock write; no IO |
| Memory per enforcer | ~1 KB | Config + budget state + warnings |
| Concurrent capacity | Unlimited | RwLock allows concurrent reads |

## Health Checks

The enforcement module exposes health information via:

1. **`get_budget_status()`** — returns all budget usage vs. limits
2. **`check_execution_limits()`** — returns list of reached limits
3. **`has_active_warnings()`** — returns whether any budget warnings are active
4. **HTTP endpoints** — `GET /api/v1/enforcement/{id}/budgets` and `GET /api/v1/enforcement/{id}/limits`

## Metrics

| Metric | Source | Description |
|--------|--------|-------------|
| `enforcer_tool_calls_allowed` | `evaluate_tool_call()` count | Total tool calls allowed |
| `enforcer_tool_calls_blocked` | `evaluate_tool_call()` count | Total tool calls blocked |
| `enforcer_budgets_exhausted` | `track_resource_usage()` | Number of budgets at hard limit |
| `enforcer_active_warnings` | `active_warnings()` | Current number of active warnings |
| `enforcer_limits_reached` | `check_execution_limits()` | Number of execution limits reached |

---

*Last updated: 2026-06-13*
