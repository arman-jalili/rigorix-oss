# Runbook: failure-classification Module

<!--
Canonical Reference: .pi/architecture/modules/failure-classification.md
Last Updated: 2026-06-13
-->

## Overview

The `failure-classification` module classifies execution failures into typed categories
for retry routing. It maps error messages to `FailureType` via pattern matching, and
each `FailureType` maps to a recommended `RetryStrategy`.

## Startup Sequence

### Dependencies

| Dependency | Required | Description |
|------------|----------|-------------|
| None | — | The module is pure logic with no external dependencies |

### Initialization

1. Module loads automatically as part of the `rigorix` library
2. Create `FailureClassifierServiceImpl` for classification operations
3. Create `FailureMappingServiceImpl` for strategy mapping
4. Create `StrategyFactoryImpl` for strategy construction
5. Register any custom patterns via `FailureMappingService::register_pattern()` if needed

```rust
use rigorix::failure_classification::application::*;

let classifier = FailureClassifierServiceImpl;
let mapper = FailureMappingServiceImpl::new();
let factory = StrategyFactoryImpl;
```

### Quick Start

```rust
use rigorix::failure_classification::classify::classify_failure;
use rigorix::failure_classification::domain::FailureType;

// Simple classification
let ft = classify_failure("connection timeout");
assert_eq!(ft, FailureType::Transient);

// Full service
use rigorix::failure_classification::application::*;

let service = FailureClassifierServiceImpl;
let result = service.classify(ClassifyFailureInput {
    error_message: "build failed".to_string(),
    operation_context: None,
    source: None,
}).await.unwrap();

println!("Type: {:?}, Retryable: {}", result.failure_type, result.is_retryable);
```

## Graceful Shutdown

The failure-classification module has no persistent state or connections,
so no explicit shutdown is required. Memory is reclaimed through normal
Rust drop semantics.

## Common Failure Modes

### Failure During Classification

**Symptom:** `FailureClassificationError::InvalidInput` returned
**Cause:** Empty or whitespace-only error message provided
**Recovery:** Validate input before calling classify — ensure message is non-empty
**Prevention:** Implement input validation at the API boundary

### Missing Strategy Mapping

**Symptom:** `FailureClassificationError::MissingStrategy` returned
**Cause:** A `FailureType` is not in the default mapping
**Recovery:** Use `FailureMappingService::get_strategy()` with an override
**Prevention:** Verify the `default_mapping()` function includes all 7 FailureTypes

### Invalid Expansion Level

**Symptom:** `FailureClassificationError::InvalidExpansionLevel` returned
**Cause:** `ExpandContext` level outside 0–5 range
**Recovery:** Clamp the level to valid range before calling factory
**Prevention:** Validate level input at the API boundary

### Pattern Repository Poisoned

**Symptom:** `FailureClassificationError::PatternRepository` returned
**Cause:** Internal lock contention in `FailureMappingServiceImpl`
**Recovery:** Restart the service (pattern state is ephemeral)
**Prevention:** Use short-lived locks; consider a database-backed pattern store

## Configuration Reference

The failure-classification module has no configuration knobs. All behavior is
determined by code-level patterns defined in `failure_classifier_service_impl.rs`.

### Built-in Patterns

| Pattern | FailureType | Priority |
|---------|-------------|----------|
| "out of memory", "oom", "disk full" | ResourceExhausted | 1 (highest) |
| "signal", "process crash", "killed", "segfault" | SystemError | 2 |
| "build fail", "compile error" | BuildFailure | 3 |
| "test" + "fail"/"error" (both keywords) | TestFailure | 4 |
| "lsp", "type mismatch", "type conflict" | LspConflict | 5 |
| "timeout", "connection", "network", "429" | Transient | 6 |
| (no match) | NonRetryable | 7 (lowest) |

### Default Strategy Mapping

| FailureType | RetryStrategy | Retryable |
|-------------|---------------|-----------|
| Transient | SameOperation | ✅ |
| LspConflict | ReExecute | ✅ |
| ResourceExhausted | Fallback | ✅ |
| SystemError | Fallback | ✅ |
| TestFailure | PatchWithFeedback | ❌ |
| BuildFailure | PatchWithFeedback | ❌ |
| NonRetryable | SameOperation (fallback) | ❌ |

## Disaster Recovery

See [DR Plan](./dr-plan-failure-classification.md) for detailed failover and recovery procedures.

## Monitoring

### Health Check

The module exposes no runtime health check (it is stateless logic).
Applications consuming this module should verify at startup:
1. `FailureClassifierServiceImpl::classify()` returns a valid result for a known input
2. `FailureMappingServiceImpl::get_strategy()` returns correct mappings for all types

### Logging

Key events emitted as `FailureClassificationEvent`:
- `FailureClassified` — successful classification with strategy assignment
- `ClassificationFailed` — no matching pattern found
- `StrategySelected` — strategy selection with source (default/override)
- `PatternRegistered` — custom pattern added

### Metrics

| Metric | Type | Description |
|--------|------|-------------|
| classify.count | Counter | Total classification requests |
| classify.errors | Counter | Classification failures |
| classify.latency | Histogram | Classification duration |
| strategy.overrides | Counter | Number of strategy overrides applied |
| retryable.count | Gauge | Current retry eligibility count |
