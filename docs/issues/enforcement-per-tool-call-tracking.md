---
guardian_issue:
  id: "ISSUE-ENFORCEMENT-PER-TOOL-CALL-TRACKING"
  title: "enforcement: resolve the 'reserved for a future implementation' per-tool call counter"
  epic: "architecture hygiene"
  component: "Enforcement"
  module: "enforcement"
  status: planned
  priority: low
  dependencies:
    - "ISSUE-RESOURCE-BUDGET-COUNTER-SUPERSEDED"

  intent: |
    enforcer_impl.rs carries a comment marking per-tool call counting as "reserved for a future
    implementation". Decide whether that counter is needed; if it is, it is a per-resource
    limit like any other and belongs behind the capacity ledger rather than as a third counting
    mechanism. If it is not needed, remove the placeholder so the code stops implying a
    capability that does not exist.

  in_scope:
    - "Decide: needed (express it via the ledger) or not needed (delete the comment and any stub)"
    - "If needed, express it as a governed resource rather than a bespoke counter"
    - "Either way, leave no comment implying an unimplemented guarantee"

  out_of_scope:
    - "The capacity ledger implementation"
    - "Tool policy semantics beyond call counting"

  affected_layers:
    domain:
      - "Possibly modified: tool policy config"
    application:
      - "Modified: enforcer_impl.rs (remove the placeholder or implement via the ledger)"
    infrastructure:
      - "None"
    api:
      - "None"

  canonical_references:
    - evidence: "engine/src/enforcement/application/enforcer_impl.rs:173"
    - module: ".pi/architecture/modules/capacity-ledger.md"

  acceptance_criteria:
    - "No comment in enforcement advertises an unimplemented capability"
    - "If implemented, the counter is a ledger-governed resource and is tested for the concurrent case"
    - "If removed, the build and tests are unaffected"

  validators:
    - ci
    - tests
    - architecture

  implementation_notes: |
    - This is a small issue by design. Its value is removing a false implication and preventing a
      third counting mechanism from appearing.

  file_changes:
    - "modify: engine/src/enforcement/application/enforcer_impl.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-ENFORCEMENT-PER-TOOL-CALL-TRACKING

## Intent

Either implement per-tool call counting properly (via the ledger) or delete the placeholder.

## Evidence

`enforcer_impl.rs:173` — *"Per-tool call count tracking is reserved for a future implementation."*

## Why it matters

A comment that advertises a future capability reads, to a maintainer, as a capability that
exists. This project has already had one such case (`precondition` reachability) and resolved it
by making the wiring explicit. Do the same, at this much smaller scale.

## Acceptance Criteria

No unimplemented guarantee is advertised in enforcement. If built, it is ledger-governed and
concurrency-tested.
