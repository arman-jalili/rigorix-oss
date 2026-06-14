# Disaster Recovery Plan: risk-gating Module

<!--
Canonical Reference: .pi/architecture/modules/risk-gating.md
Last Updated: 2026-06-14
-->

## Scope

This DR plan covers the `risk-gating` module — the risk classification and gating
system that assigns risk levels to tool calls and enforces gating policies before
execution. The risk-gating module is stateless at startup and fully in-memory
during execution. All gate state (pending gates, active overrides) is ephemeral
and tied to the `GateStateRegistry`.

## RTO/RPO Targets

| Metric | Target | Rationale |
|--------|--------|-----------|
| RTO (Recovery Time Objective) | < 1 minute | Module is stateless at startup — `RiskGateServiceImpl` is created fresh from config |
| RPO (Recovery Point Objective) | 0 (in-memory only) | Gate state is ephemeral per execution; no persistent state to recover |

## Backup Strategy

**No backups required — risk-gating state is ephemeral per execution.**

The risk-gating module operates entirely in memory:
1. `RiskConfig` is loaded at startup from config or defaults
2. `GateStateRegistry` tracks pending gates in memory during execution
3. Runtime tool overrides are stored in the `RiskConfig` within the service
4. At execution end, all risk-gating state is discarded (or cleaned up via
   `gate_registry.cleanup_execution()`)

### What Gets Recreated

On service creation, the following is built fresh from config:

| Component | Source | Recovery Method |
|-----------|--------|-----------------|
| `RiskConfig` | Config section or defaults | Rebuilt from `Config.risk_gating` |
| `DefaultClassifier` | `RiskConfig` | Rebuilt with rules + overrides |
| `GateStateRegistry` | Empty registry | Fresh registry per application |
| Pending gates | Runtime tracking | Lost on failure (re-evaluate) |
| Runtime overrides | `override_tool()` calls | Can be replayed from audit log |

## Restore Procedure

### Scenario: Service State Corruption

If the `RiskGateServiceImpl` internal state becomes corrupted (e.g., `RwLock` poisoning):

1. **Detect corruption:** `evaluate_gate()` or `classify_tool()` returns
   `RiskGatingError::InvalidState`
2. **Create new service using factory:**
   ```rust
   let factory = RiskGateFactoryImpl::new(gate_registry);
   let new_service = factory.create_default("exec-1").await?;
   ```
3. **Re-apply runtime overrides:** If any overrides were active, replay them
   from the audit log or application state:
   ```rust
   new_service.override_tool(OverrideToolInput {
       execution_id: "exec-1".to_string(),
       tool: "bash".to_string(),
       new_level: RiskLevel::Medium,
       reason: Some("Restored from audit".to_string()),
   }).await?;
   ```
4. **Re-evaluate pending gates:** Any gates that were pending before corruption
   must be re-created by re-calling `evaluate_gate()` with the original inputs

### Scenario: GateRegistry Data Loss

If the `GateStateRegistry` is accidentally cleared or its data lost:

1. **Detect data loss:** `resolve_gate()` returns "Gate not found" for a known gate
2. **Re-create gates:** Re-call `evaluate_gate()` for each pending tool call to
   generate new gate IDs
3. **Notify affected users:** Inform users that their previous confirmation
   decisions must be re-made
4. **Log the incident:** Record the registry data loss event for audit

### Scenario: Configuration Corruption

If the `RiskConfig` becomes corrupted (invalid values, missing fields):

1. **Detect corruption:** Factory returns error during service creation
2. **Fall back to defaults:** Use `RiskConfig::default()` or `RiskConfig::strict()`
   which are guaranteed valid:
   ```rust
   let safe_config = RiskConfig::strict(); // Always valid
   ```
3. **Report configuration error:** Log the validation failure for operator review
4. **Proceed with safe defaults:** The execution continues with strict gating

### Scenario: Factory Failure

If the `RiskGateFactoryImpl` cannot create a service:

1. **Check configuration:** Verify that the `RiskConfig` passed to the factory
   is valid (no infinite maps, sensible values)
2. **Use default factory:** Create a fresh factory:
   ```rust
   let factory = RiskGateFactoryImpl::default(); // Fresh registry
   let service = factory.create_default("exec-1").await?;
   ```
3. **Escalate:** If the factory continues to fail, the tokio runtime or
   memory allocation may be the root cause

## Failover Plan

The risk-gating module has no active replication or failover mechanism because
it is:

1. **Stateless at startup** — any instance can be created fresh from config
2. **Per-execution state only** — gate state is tied to a single execution
3. **Low criticality** — a risk-gating failure blocks tool execution but does
   not cause data loss (tools are not executed without gate approval)

In a multi-instance execution environment:

- Each execution gets its own `RiskGateServiceImpl` instance
- The `GateStateRegistry` can be application-global or per-execution
- No distributed state coordination is needed
- If one service instance fails, the execution is retried with a fresh service

## Dependency Recovery

| Dependency | Failure Mode | Recovery |
|------------|-------------|----------|
| Tokio runtime | Panic in async task | Restart the application; all gate state is ephemeral |
| Serde deserialization | Invalid config format | Use `RiskConfig::default()` as fallback |
| Memory allocation | OOM | Reduce number of concurrent executions; gate state is lightweight (~few KB per execution) |

## Testing the DR Plan

1. **Unit test:** Scenarios for lock poisoning recovery are tested in
   `gate_service_impl.rs` via error path assertions
2. **Integration test:** Verify that a fresh service after corruption produces
   correct classifications (tested via factory tests)
3. **Manual drill:** Simulate a GateRegistry data loss by calling
   `gate_registry.cleanup_execution()` mid-execution and verify that
   `evaluate_gate()` re-creates gates
