# Runbook: risk-gating Module

<!--
Canonical Reference: .pi/architecture/modules/risk-gating.md
Last Updated: 2026-06-14
-->

## Overview

The `risk-gating` module classifies tools/operations by risk level (Low, Medium, High)
and enforces gating policies before execution. Every tool invocation passes through the
risk gate before being allowed to execute. The gate determines whether the tool
auto-executes, requires user confirmation, or runs in dry-run mode.

## Components

| Component | Type | Description |
|-----------|------|-------------|
| `RiskLevel` | Domain enum | Three-level risk classification: Low, Medium, High |
| `RiskClassifier` | Domain trait | Maps tool name → RiskLevel with configurable overrides |
| `DefaultClassifier` | Service impl | Built-in classification rules (20+ tool patterns) |
| `RiskConfig` | Domain entity | Per-tool overrides + gating policy flags |
| `RiskGateServiceImpl` | Application service | classify + evaluate + resolve + override operations |
| `RiskGateFactoryImpl` | Factory | Constructs service instances from config with overrides |
| `GateStateRegistry` | Domain service | Thread-safe pending gate tracking across executions |
| `InMemoryConfigRepository` | Repository | In-memory RiskConfig storage per execution ID |
| `RiskGatingError` | Domain error | Typed errors: UnknownTool, InvalidConfig, InvalidOverride, InvalidState, ClassificationError |

### Classification Rules (DefaultClassifier)

| Tool Pattern | Risk Level | Gate Policy |
|-------------|------------|-------------|
| `file_read`, `read`, `lsp_query`, `git_read`, `git_diff`, `git_log`, `git_status`, `glob`, `grep`, `list_files`, `search_files` | Low | Auto-execute |
| `file_write`, `write`, `file_append`, `file_patch`, `edit`, `git_stage`, `git_add`, `create_file` | Medium | Requires confirmation |
| `run_command`, `bash`, `git_commit`, `git_push`, `git_reset`, `delete_file`, `remove` | High | Dry-run by default |
| Unknown tools | Medium (safe default) | Requires confirmation |

### Gating Policy Flags

| Flag | Default | Effect |
|------|---------|--------|
| `auto_confirm_low` | `true` | Low-risk tools auto-execute without gate |
| `require_review_medium` | `true` | Medium-risk tools require user confirmation |
| `dry_run_high` | `true` | High-risk tools execute in dry-run mode |

## Startup Sequence

### Dependencies

| Dependency | Required | Description |
|------------|----------|-------------|
| tokio runtime | Yes | Async runtime for async trait methods |
| serde | Yes | DTO/config serialization/deserialization |
| chrono | Yes | Gate creation timestamps (ISO 8601 UTC) |
| uuid | No | Gate ID generation (auto-increment fallback) |
| async-trait | Yes | Trait object safety for RiskGateService |

### Initialization

1. Create a `RiskConfig` with desired overrides and gating flags:
   ```rust
   use rigorix::risk_gating::domain::RiskConfig;
   
   let config = RiskConfig::default(); // or strict(), permissive(), custom()
   ```

2. Create the service via factory:
   ```rust
   use std::sync::Arc;
   use rigorix::risk_gating::application::RiskGateFactoryImpl;
   use rigorix::risk_gating::domain::GateStateRegistry;
   
   let gate_registry = Arc::new(GateStateRegistry::new());
   let factory = RiskGateFactoryImpl::new(gate_registry);
   let service = factory.create_default("exec-1").await.unwrap();
   ```

3. Start gating tool calls:
   ```rust
   let result = service.evaluate_gate(EvaluateGateInput {
       execution_id: "exec-1".to_string(),
       node_id: "node-1".to_string(),
       tool: "bash".to_string(),
       parameters: None,
       is_retry: false,
   }).await.unwrap();
   
   match result.gating_action {
       GatingAction::AutoExecute => execute_tool(),
       GatingAction::RequireConfirmation => request_user_confirmation(result.gate_id),
       GatingAction::DryRun => execute_dry_run().await,
   }
   ```

### Quick Start

```rust
use std::sync::Arc;
use rigorix::risk_gating::domain::*;
use rigorix::risk_gating::application::*;

// 1. Create registry and factory
let gate_registry = Arc::new(GateStateRegistry::new());
let factory = RiskGateFactoryImpl::new(gate_registry);

// 2. Create service with overrides
let mut overrides = std::collections::HashMap::new();
overrides.insert("bash".to_string(), RiskLevel::Medium);
let config = RiskConfig::new(overrides);
let service = factory.create_from_config("exec-1", config).await.unwrap();

// 3. Classify and gate a tool call
let output = service.evaluate_gate(EvaluateGateInput {
    execution_id: "exec-1".to_string(),
    node_id: "node-1".to_string(),
    tool: "bash".to_string(),
    parameters: None,
    is_retry: false,
}).await.unwrap();

// 4. Resolve the gate (user approves)
if output.gating_action == GatingAction::RequireConfirmation {
    let resolution = service.resolve_gate(ResolveGateInput {
        execution_id: "exec-1".to_string(),
        gate_id: output.gate_id,
        approved: true,
        reason: Some("User approved".to_string()),
    }).await.unwrap();
    
    if resolution.can_proceed {
        // Execute the tool
    }
}
```

## Graceful Shutdown

1. **Complete pending gates:** Wait for all pending user confirmation requests
   to be resolved (approve or reject).
2. **Clean up gate state:** Call `gate_registry.cleanup_execution(execution_id)`
   to release memory for completed gates.
3. **Log final state:** Emit a summary of gates resolved vs. rejected for audit.

The `GateStateRegistry` is shared across executions — cleanup per execution
ensures memory is freed. The registry itself lives for the lifetime of the
application.

## Common Failure Modes and Recovery

### Failure: UnknownTool

**Symptom:** `RiskGatingError::UnknownTool` returned from `classify_tool()`.

**Cause:** The tool name doesn't match any default rule or configured override.

**Recovery:**
1. Add a tool override via `service.override_tool(OverrideToolInput { ... })` or
2. Add the tool to `RiskConfig.tool_overrides` before service creation
3. The classifier defaults unknown tools to Medium (in `evaluate_gate`, not `classify_tool`)

### Failure: Gate Already Resolved

**Symptom:** `RiskGatingError::InvalidState` with "Gate X has already been resolved".

**Cause:** Duplicate `resolve_gate` call for the same gate_id.

**Recovery:**
1. Check if the gate is still pending using `gate_registry.is_gate_pending()`
2. Ensure idempotent gate resolution in callers

### Failure: Lock Poisoning

**Symptom:** Thread panic with "Classifier lock poisoned" or "Config lock poisoned".

**Cause:** A previous operation panicked while holding the RwLock.

**Recovery:**
1. Create a new service instance via the factory
2. Restore any runtime overrides from the repository
3. The old service should be discarded

### Failure: Override Not Taking Effect

**Symptom:** Tool classification doesn't reflect a recently set override.

**Cause:** The override was set after the classifier was created.

**Recovery:**
1. Use `service.override_tool()` which updates both the config and the classifier
2. If using config directly, ensure `classifier.set_config()` is called after
   modifying the config

## Configuration Reference

### TOML Format

```toml
[risk_gating]
# Per-tool risk level overrides
tool_overrides = { "run_command" = "high", "git_push" = "high" }

# Gating policy flags (all default to true)
auto_confirm_low = true
require_review_medium = true
dry_run_high = true
```

### Programmatic Configuration

```rust
use rigorix::risk_gating::domain::*;

// Default — all gates enabled, no overrides
let config = RiskConfig::default();

// Strict — all gates enabled (same as default with explicit intent)
let config = RiskConfig::strict();

// Permissive — all gates disabled (auto-execute everything)
let config = RiskConfig::permissive();

// Custom — full control
let mut overrides = std::collections::HashMap::new();
overrides.insert("bash".to_string(), RiskLevel::Medium);
let config = RiskConfig::custom(overrides, true, true, false);
```

## Metrics

The following metrics should be exposed for monitoring:

| Metric | Type | Description |
|--------|------|-------------|
| `risk_gating.classifications.total` | Counter | Total tool classifications performed |
| `risk_gating.classifications.low` | Counter | Classifications resulting in Low risk |
| `risk_gating.classifications.medium` | Counter | Classifications resulting in Medium risk |
| `risk_gating.classifications.high` | Counter | Classifications resulting in High risk |
| `risk_gating.classifications.override` | Counter | Classifications using a configured override |
| `risk_gating.gates.pending` | Gauge | Number of pending unresolved gates |
| `risk_gating.gates.resolved` | Counter | Total gates resolved (approved + rejected) |
| `risk_gating.gates.approved` | Counter | Gates approved by user |
| `risk_gating.gates.rejected` | Counter | Gates rejected by user |
| `risk_gating.overrides.active` | Gauge | Number of active tool overrides |

## Logging

All risk-gating operations emit structured log events:

```json
{
  "event": "risk_gate.evaluate",
  "execution_id": "exec-1",
  "node_id": "node-2",
  "tool": "file_write",
  "risk_level": "medium",
  "gating_action": "require_confirmation",
  "gate_id": "gate-exec-1-42"
}

{
  "event": "risk_gate.resolve",
  "execution_id": "exec-1",
  "gate_id": "gate-exec-1-42",
  "approved": true,
  "reason": "User approved"
}

{
  "event": "risk_gate.override",
  "execution_id": "exec-1",
  "tool": "bash",
  "new_level": "medium",
  "previous_level": "high"
}
```
