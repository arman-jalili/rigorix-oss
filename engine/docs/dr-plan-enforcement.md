# Disaster Recovery Plan: enforcement Module

<!--
Canonical Reference: .pi/architecture/modules/enforcement.md
Last Updated: 2026-06-13
-->

## Scope

This DR plan covers the `enforcement` module — the runtime enforcement system
that gates tool calls, tracks resource budgets, and enforces execution limits.
The enforcement module is purely in-memory and stateless at startup. All
enforcement state is ephemeral and tied to a single execution session.

## RTO/RPO Targets

| Metric | Target | Rationale |
|--------|--------|-----------|
| RTO (Recovery Time Objective) | < 1 minute | Module is stateless at startup — `ExecutionEnforcerImpl` is created fresh from config |
| RPO (Recovery Point Objective) | 0 (in-memory only) | Enforcement state is ephemeral; no persistent state to recover |

## Backup Strategy

**No backups required — enforcement state is ephemeral per execution.**

The enforcement module operates entirely in memory:
1. `EnforcementConfig` is loaded at startup from the preset or configuration
2. Budget usage (`current_usage`) is tracked in memory during execution
3. Warnings and limit violations are runtime-only state
4. At execution end, all enforcement state is discarded

The configuration itself (`EnforcementConfig`, `SafetyCaps`) is derived from the
global `Config` which is backed up by the Configuration module (see `dr-plan-configuration.md`).

### What Gets Recreated

On enforcer creation, the following is built fresh from config:

| Component | Source | Recovery Method |
|-----------|--------|-----------------|
| `EnforcementConfig` | Preset or serialized config | Rebuilt from `Config.enforcement` |
| `ResourceBudget.current_usage` | Always starts at 0 | Zeroed on creation |
| `ToolPolicy` overrides | Optional runtime overrides | Reloaded from repository if available |
| Active warnings | Runtime tracking | Tracked fresh on each execution |

## Restore Procedure

### Scenario: Enforcer State Corruption

If the enforcer's internal state becomes corrupted (e.g., `RwLock` poisoning):

1. **Detect corruption:** `evaluate_tool_call()` or `track_resource_usage()` returns
   `EnforcementError::InvalidState`
2. **Create new enforcer:**
   ```rust
   let factory = ExecutionEnforcerFactoryImpl;
   let new_enforcer = factory.create_default("exec-1").await?;
   ```
3. **Reset budget trackers:** If the execution needs to continue with fresh budgets,
   the new enforcer starts with zeroed usage
4. **Log the incident:** Record the state corruption event with the old enforcer's
   execution ID for audit

### Scenario: Config Validation Failure

If the enforcement configuration fails validation against safety caps:

1. **Detect failure:** Factory returns `EnforcementError::InvalidConfiguration` with
   a list of validation errors
2. **Use preset fallback:** Switch to a predefined preset that is guaranteed valid:
   ```rust
   let safe_config = EnforcementConfig::standard(); // Always valid
   ```
3. **Report configuration error:** Log the validation errors for operator review
4. **Proceed with safe defaults:** The execution can continue with standard preset

### Scenario: Budget Tracking Desync

If budget tracking becomes desynchronized from actual resource consumption:

1. **Check current budget status:**
   ```rust
   let status = enforcer.get_budget_status(GetBudgetStatusInput {
       execution_id: "exec-1".to_string(),
       resources: None,
   }).await?;
   ```
2. **Reload configuration** to reset all budgets:
   ```rust
   enforcer.reload_config().await?;
   ```
   This resets all budgets to zero and clears all warnings.
3. **Re-track known consumption** by calling `track_resource_usage()` with accurate
   amounts since the last known-good state

## Failover Plan

### Single Instance Architecture

The enforcement module runs as part of the orchestrator process. There is no
standby or failover for the enforcer itself — each execution creates its own
`ExecutionEnforcerImpl` instance.

### Failure Scenarios

| Scenario | Impact | Mitigation |
|----------|--------|------------|
| Enforcer panic | Execution loses enforcement; no tool gating | Create new enforcer from factory |
| Config validation error | Execution cannot start without valid enforcement | Fall back to standard preset |
| Budget tracking lost | Execution continues without budget enforcement | Reload config to reset budgets |
| RwLock poisoned | All enforcement operations fail | Recreate enforcer; restart execution |
| Preset mismatch | Wrong enforcement profile applied | Audit tool call logs; correct config |

### Monitoring Signs

Watch for these indicators that enforcement is degraded:

1. **No tool calls being blocked** — If `evaluate_tool_call` never returns `allowed: false`,
   the enforcer may be running with permissive settings
2. **Zero active warnings** — In long-running executions, some budget warnings are
   expected; zero may indicate tracking is not working
3. **Rapid budget exhaustion** — If budgets hit hard limits much faster than expected,
   the preset may be too restrictive or tracking is double-counting

## DR Testing

### Test Scenarios

| Scenario | Test Method | Frequency |
|----------|-------------|-----------|
| Enforcer created from standard preset | Unit test: `test_build_standard_preset` | Every CI run |
| Enforcer validates and blocks blocked tools | Unit test: `test_evaluate_tool_not_allowed_by_policy` | Every CI run |
| Budget tracking correctly limits resources | Unit test: `test_evaluate_tool_blocked_when_budget_exhausted` | Every CI run |
| Execution limits enforced correctly | Unit test: `test_check_execution_limits_tool_call_limit` | Every CI run |
| Warnings triggered at threshold | Unit test: `test_warning_after_threshold_crossed` | Every CI run |
| Config validation detects exceeded caps | Unit test: `test_validation_detects_exceeded_tool_call_cap` | Every CI run |
| Concurrent access is safe | Unit test: `test_concurrent_evaluate_and_track` | Every CI run |

### CI Validation

68 enforcement-specific tests run as part of the standard test suite. These
cover all preset profiles, all enforcement operations, and edge cases.

---

*Last updated: 2026-06-13*
