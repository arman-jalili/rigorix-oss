# Disaster Recovery Plan: failure-classification Module

<!--
Canonical Reference: .pi/architecture/modules/failure-classification.md
Last Updated: 2026-06-13
-->

## Scope

This DR plan covers the `failure-classification` module — a stateless, pure-logic
component for classifying execution failures. Since the module has no persistent
state, database connections, or external dependencies, the DR requirements are
minimal.

## RTO/RPO Targets

| Metric | Target | Rationale |
|--------|--------|-----------|
| RTO (Recovery Time Objective) | < 1 minute | Module is stateless — restart is instant |
| RPO (Recovery Point Objective) | N/A | No persistent state to recover |

## Backup Strategy

**No backups required.** The failure-classification module is stateless with:
- No database or file storage
- No persistent configuration beyond compile-time defaults
- No accumulated runtime state

Custom patterns registered via `register_pattern()` are ephemeral (in-memory).
If persistence is needed, implement `PatternRepository` with a database backend
and configure backups at that layer.

## Restore Procedure

Since the module has no state, restore == restart:

1. **Verify binary integrity:** Check that the `rigorix` library binary matches
   the expected SHA-256 checksum from the build artifact
2. **Restart application:** Re-initialize the module
   ```rust
   let classifier = FailureClassifierServiceImpl;
   let mapper = FailureMappingServiceImpl::new();
   let factory = StrategyFactoryImpl;
   ```
3. **Verify classification:** Run a quick smoke test
   ```rust
   let result = classify_failure("connection timeout");
   assert_eq!(result, FailureType::Transient);
   ```
4. **Re-register custom patterns** if applicable (these are ephemeral):
   ```rust
   mapper.register_pattern("custom error".into(), FailureType::Transient).await?;
   ```

## Failover Plan

### Scenario 1: Module fails to load

**Detection:** Application fails to start with module initialization error
**Impact:** All failure classification functionality unavailable
**Response:**
1. Check binary integrity (recompile if corrupted)
2. Verify all source files exist: `src/failure_classification/`
3. Ensure all dependencies are present (`async-trait`, `serde`, `thiserror`)
4. Restart application

### Scenario 2: Classification returns incorrect results

**Detection:** Errors classified to wrong `FailureType`, causing wrong retry behavior
**Impact:** Transient errors may be treated as fatal, or non-retryable errors may be retried
**Response:**
1. Check error message pattern matching in `failure_classifier_service_impl.rs`
2. Verify custom patterns haven't shadowed built-in patterns
3. Add test case for the misclassified pattern
4. Deploy fix via normal release process

### Scenario 3: Pattern repository lock poisoned

**Detection:** `FailureClassificationError::PatternRepository` errors
**Impact:** Custom pattern registration fails; default patterns still work
**Response:**
1. Restart the application (clears in-memory state)
2. Consider switching to a database-backed pattern repository
3. Re-register required custom patterns

## Recovery Testing

| Test | Frequency | Procedure |
|------|-----------|-----------|
| Smoke test | Every deploy | Run the full test suite: `cargo test --lib failure_classification` |
| Integration test | Every deploy | Verify classify + map + eligibility pipeline for all 7 FailureTypes |
| DR drill | Quarterly | Simulate module failure and verify recovery within RTO |

## Incident Response

### Severity Levels

| Level | Definition | Response Time |
|-------|------------|---------------|
| SEV-1 | Classification completely unavailable | 15 minutes |
| SEV-2 | Classification gives wrong results for key patterns | 1 hour |
| SEV-3 | Edge case misclassification, non-critical patterns | Next business day |

### Escalation Path

1. **Level 1:** Module owner — check logs, verify pattern matching
2. **Level 2:** Engineering lead — review code changes, coordinate fix
3. **Level 3:** Architecture team — review pattern matching strategy

## Post-Mortem Requirements

For any SEV-1 or SEV-2 incident, document:
1. Root cause analysis
2. Which patterns were affected
3. How many classifications were incorrect
4. What changes were made to prevent recurrence
5. Whether the default mapping needs updating
