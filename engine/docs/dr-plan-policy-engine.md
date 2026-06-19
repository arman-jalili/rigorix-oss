# Disaster Recovery Plan: Policy Engine

## Overview

The Policy Engine is a stateless evaluation component with in-memory rule storage. Rules are loaded from optional configuration files (`.rigorix/policy.toml`) or from programmatic defaults. No persistent state is maintained — full recovery is achieved by reloading rules.

## RTO/RPO Targets

| Metric | Target | Notes |
|--------|--------|-------|
| RTO (Recovery Time) | < 1 second | Stateless — just recreate engine and load rules |
| RPO (Recovery Point) | N/A | No mutable state to lose |

## Failure Scenarios

### Scenario 1: Corrupted Configuration File

**Impact**: Rules fail to load, engine falls back to defaults.

**Detection**: 
- `PolicyRepository::load_config()` returns `DeserializationError`
- Error logged with file path and parse error details

**Recovery**:
1. Check `.rigorix/policy.toml` for syntax errors
2. Fix the TOML syntax
3. Call `PolicyEngineService::reload_rules()` or recreate the engine

**Prevention**:
- Validate TOML before saving (TOML parser catches issues)
- Keep a backup of working config

### Scenario 2: Missing Configuration File

**Impact**: No user-defined rules loaded. Default rules used.

**Detection**:
- `DefaultPolicyRepository::load_config()` returns `InvalidConfiguration` with "file not found"
- Factory falls back to `create_default()` if configured

**Recovery**:
1. Create `.rigorix/policy.toml` with desired rules
2. Load rules via `load_rules()` or `create_with_repository()`

**Prevention**:
- The `.rigorix/policy.toml` is optional — no recovery action needed if defaults are acceptable

### Scenario 3: Engine In-Memory State Corruption

**Impact**: Rules are lost or corrupted in memory.

**Detection**:
- `evaluate()` returns unexpected results
- `rule_count()` returns 0 unexpectedly

**Recovery**:
1. Call `load_rules()` to reload from source
2. If source is unavailable, recreate engine via `create_default()`

**Prevention**:
- Engine uses `RwLock` for thread-safe access
- Immutable `PolicyRule` struct prevents mutation after loading

### Scenario 4: Integration Failure (Quality Gates / Risk Gating Down)

**Impact**: Conditions that depend on external state (`GreenAt`, `StartupBlocked`) may fail.

**Detection**:
- `PolicyCondition::matches()` returns unexpected results
- No panic — condition gracefully evaluates based on available context

**Recovery**:
1. Restore dependent service (Quality Gates, Risk Gating)
2. Re-evaluate with updated `LaneContext`
3. The engine itself is unaffected — only the context data is incomplete

**Prevention**:
- LaneContext is provided by the orchestrator as a complete snapshot
- Context is immutable once constructed

## Backup Strategy

| Asset | Backup Method | Frequency | Retention |
|-------|--------------|-----------|-----------|
| `.rigorix/policy.toml` | Regular file backup (git) | Per commit | Full git history |

## Restore Procedure

1. **Restore configuration file** from git: `git checkout <commit> -- .rigorix/policy.toml`
2. **Recreate PolicyEngine** with restored config:
   ```
   let repo = DefaultPolicyRepository::new(path);
   let config = repo.load_config().await?;
   let factory = PolicyEngineFactoryImpl::new();
   let engine = factory.create_from_config(config).await?;
   ```
3. **Verify** engine has expected rules: `engine.rule_count()`
4. **Resume** normal operation — engine is ready immediately

## Failover

No failover required — the Policy Engine is stateless and can be recreated on any node.

## DR Testing Schedule

| Test | Frequency | Success Criteria |
|-----|-----------|-----------------|
| Missing config recovery | Per release | Default rules load successfully |
| Config restore from git | Per release | Rules match git content |
| Engine recreation | Per CI run | `create_default()` returns engine with 4 rules |
