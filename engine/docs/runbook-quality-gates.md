# Runbook: quality-gates Module

<!--
Canonical Reference: .pi/architecture/modules/quality-gates.md
Last Updated: 2026-06-19
-->

## Overview

The `quality-gates` module formalizes test quality into a four-tier escalation:
`TargetedTests` → `Package` → `Workspace` → `MergeReady`. Each tier represents
a broader scope of validation. The `GreenContract` pattern allows users and the
orchestrator to declare a required quality level, and the engine verifies whether
the observed test scope satisfies the contract.

## Startup Sequence

### Dependencies

| Dependency | Required | Description |
|------------|----------|-------------|
| execution_engine | Yes | Reports test scope for classification |
| enforcement | Yes | Gates task closeout based on quality outcome |
| event_system | No | Emits quality gate events |

### Initialization

1. Load `QualityGateConfig` from configuration (`.rigorix/quality.toml`)
2. Create `QualityGateServiceImpl` with config
3. Wire into orchestrator's closeout decision flow

```rust
use rigorix::quality_gates::application::*;
use rigorix::quality_gates::infrastructure::*;

let config = QualityGateConfig::new(QualityLevel::Workspace);
let service = QualityGateServiceImpl::new(config);
let repo = InMemoryQualityGateRepository::new(config);

// Evaluate a gate
let outcome = service.evaluate_gate(EvaluateGateInput {
    contract: GreenContract::new(QualityLevel::Workspace),
    observed_level: Some(QualityLevel::Package),
    task_id: None,
}).await?;
```

## Graceful Shutdown

1. **Complete evaluations**: Allow in-progress gate evaluations to finish
2. **Save config**: Persist any config changes via the repository
3. **Drain events**: Process remaining quality gate events

## Common Failure Modes

### "No contract defined"

**Cause:** A task has no associated `GreenContract`.

**Detection:** `QualityGateEvent::GateUnsatisfied` with `observed = None`.

**Recovery:** Falls back to the default contract level.

### "Could not classify test scope"

**Cause:** Test execution didn't produce scoped results.

**Detection:** `QualityGateError::ScopeClassificationFailed`.

**Recovery:** Defaults to `TargetedTests` level.

## Configuration Reference

```toml
# .rigorix/quality.toml
[quality]
default_required_level = "package"

[quality.templates.refactor]
required_level = "workspace"

[quality.templates.hotfix]
required_level = "merge_ready"
```

## Monitoring

### Key Metrics

- `quality_gate_evaluations_total{outcome}` — Total evaluations
- `quality_gate_satisfied_total` — Gates that passed
- `quality_gate_unsatisfied_total{template}` — Gates that failed
