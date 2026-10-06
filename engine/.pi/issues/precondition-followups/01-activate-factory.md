---
guardian_issue:
  id: "ISSUE-PF-ACT-1"
  title: "ACT: PreconditionSetup + ParallelExecutionFactoryConfig wiring"
  epic: "precondition-followups"
  epic_id: "EPIC-PRECONDITION-FOLLOWUPS"
  component: "PreconditionSetup"
  module: "precondition"
  status: planned
  priority: critical
  dependencies:
    - "EPIC-PRECONDITION-FOLLOWUPS"

  in_scope:
    - "Add PreconditionSetup (from_env): load .rigorix/preconditions.toml -> PreconditionDispatchGate + GatingMode"
    - "Add ParallelExecutionFactoryConfig fields: precondition gate + gating mode"
    - "ParallelExecutionFactoryImpl::create attaches them via with_precondition_gate / with_gating_mode"
    - "Default preserves today's behavior (None gate, GatingMode::default())"

  out_of_scope:
    - "Composition roots (ISSUE-PF-ACT-2)"
    - "New precondition semantics"

  affected_layers:
    domain:
      - "Reuse: precondition/domain/{gating,precondition}.rs"
    application:
      - "Modify: execution_engine/application/factory.rs (config + setup)"
      - "Modify: execution_engine/application/factory_impl.rs (create attaches)"
      - "Reuse: precondition/application/{factory,gate_impl,repository}.rs"

  canonical_references:
    - module: "engine/.pi/architecture/modules/consequence-gating.md#dispatch-integration"
    - adr: "engine/.pi/architecture/decisions/ADR-017-consequence-gating.md"
    - code: "engine/src/precondition/application/gate_impl.rs"
    - code: "engine/src/execution_engine/application/service_impl.rs#with_precondition_gate"

  acceptance_criteria:
    - "ParallelExecutionFactoryConfig exposes a precondition setup (gate + gating mode)"
    - "Config::default() leaves the gate unset and GatingMode default (no behavior change)"
    - "create() attaches the gate and gating mode when set"
    - "PreconditionSetup::from_env loads .rigorix/preconditions.toml (missing => no gate, fail-open-absent; malformed => Err, fail-closed)"
    - "Unit test: a factory built with a setup produces an executor whose gate is armed"

  validators:
    - ci
    - tests
    - architecture
    - canonical

  implementation_notes: |
    PreconditionDispatchGate::new takes a PreconditionRepository + PreconditionService
    (see engine/src/precondition/application/gate_impl.rs). from_env mirrors
    ApprovalBindingSetup::from_env (engine/src/execution_engine/application/factory.rs):
    read the repo root, open .rigorix/preconditions.toml via the TOML repository,
    build the service + gate, and read [gating].release_dependents_on_failure.
    Fail-closed on malformed config. Do not add new env vars unless needed; the
    config file is the switch (presence of matching preconditions).

  file_changes:
    - "modify: engine/src/execution_engine/application/factory.rs"
    - "modify: engine/src/execution_engine/application/factory_impl.rs"
    - "create: engine/src/precondition/application/setup.rs"
---

# ISSUE-PF-ACT-1: Factory setup

## Problem

`ParallelExecutionServiceImpl` holds `precondition_gate: Option<Arc<dyn DispatchGate>>`
and `gating_mode: GatingMode`, but `ParallelExecutionFactoryConfig` has neither,
and `with_precondition_gate` has **zero callers** — so production executors never
arm the gate.

## Target shape

```rust
// engine/src/execution_engine/application/factory.rs
pub struct ParallelExecutionFactoryConfig {
    // …existing…
    /// ADR-017: optional dispatch-time precondition gate + R2 gating mode.
    pub precondition: Option<PreconditionSetup>,
}

pub struct PreconditionSetup {
    pub gate: Arc<dyn DispatchGate>,
    pub gating_mode: GatingMode,
}

impl PreconditionSetup {
    pub fn from_env(repo_root: &std::path::Path) -> Result<Option<Self>, PreconditionError> { … }
}
```

`create()` then:

```rust
let mut svc = ParallelExecutionServiceImpl::new(...);
if let Some(p) = config.precondition {
    svc = svc.with_gating_mode(p.gating_mode).with_precondition_gate(p.gate);
}
```

## Verification

- `ParallelExecutionFactoryConfig::default().precondition.is_none()`
- factory with a fixture `.rigorix/preconditions.toml` yields an armed gate
- malformed config → `Err` (fail closed)

## References

- `engine/src/precondition/application/gate_impl.rs`
- `engine/src/execution_engine/application/factory.rs` (`ApprovalBindingSetup::from_env` pattern)
