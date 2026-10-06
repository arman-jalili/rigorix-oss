---
guardian_issue:
  id: "ISSUE-PF-REL-1"
  title: "REL (optional): relocate R2 GatingMode to execution_engine/dag_engine"
  epic: "precondition-followups"
  epic_id: "EPIC-PRECONDITION-FOLLOWUPS"
  component: "RelocateR2"
  module: "execution-engine"
  status: planned
  priority: low
  dependencies:
    - "ISSUE-PF-REN-2"

  in_scope:
    - "Move GatingMode (release_dependents_on_failure) from precondition/domain to execution_engine/domain (or dag_engine)"
    - "Keep the config key [gating].release_dependents_on_failure unchanged (wire-compatible)"
    - "Update the executor's dependency so R2 no longer depends on the precondition module"

  out_of_scope:
    - "Behavior change (semantics are frozen)"
    - "R1/R3"

  affected_layers:
    domain:
      - "Move: GatingMode -> execution_engine/domain"
    application:
      - "Modify: execution_engine/application/service_impl.rs imports"
      - "Modify: engine/src/precondition/domain/gating.rs consumers"

  canonical_references:
    - module: "engine/.pi/architecture/modules/consequence-gating.md#r2"
    - code: "engine/src/execution_engine/application/service_impl/dispatch.rs"

  acceptance_criteria:
    - "execution_engine no longer imports GatingMode from the precondition module"
    - "[gating].release_dependents_on_failure behavior is unchanged (existing tests pass)"
    - "No wire string or config key changes"

  validators:
    - ci
    - tests
    - architecture

  implementation_notes: |
    R2 is a dependency-release policy on the graph/executor, not a precondition
    concern. It only shares the config file with R1. Relocation is optional; do
    it only if strict DDD is desired. If deferred, close this issue as
    'won't do' with the rationale (shared config, single choke point).

  file_changes:
    - "move: engine/src/precondition/domain/gating.rs -> engine/src/execution_engine/domain/"
    - "modify: engine/src/execution_engine/application/service_impl.rs"
---

# ISSUE-PF-REL-1: Relocate R2

Optional DDD cleanup. `release_dependents_on_failure` is enforced in
`dispatch.rs` (`skip_dependents_on_failure`), which is an execution-engine
concern. Keeping the `GatingMode` type in `precondition` creates a needless
dependency from the executor to the precondition module.
