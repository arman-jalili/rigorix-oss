---
guardian_issue:
  id: "ISSUE-RESOURCE-BUDGET-COUNTER-SUPERSEDED"
  title: "enforcement: ResourceBudget is a counter, not a reservation — supersede it via the capacity ledger"
  epic: "architecture hygiene"
  component: "Enforcement"
  module: "enforcement"
  status: planned
  priority: medium
  dependencies:
    - "ADR-018 capacity ledger"
    - "ISSUE-CAPACITY-LEDGER-SEAM"

  intent: |
    Resolve the duplication at the heart of this work. enforcement.ResourceBudget
    { resource, soft_warning_threshold, hard_limit, current_usage } is the generic resource
    concept, but it is a counter with a threshold — it can report that a limit was reached, it
    cannot reserve capacity atomically, and it cannot see other executions. After the capacity
    ledger lands there are two ways to express "a limited resource", which is a trap for the
    next contributor. Decide the relationship and record it.

  in_scope:
    - "Decide: ResourceBudget becomes a view over the capacity ledger, or is deprecated, or is explicitly scoped to non-contested limits"
    - "Implement the decision with a deprecation path if the type is kept"
    - "Collapse the double representation so only one is authoritative for a given resource"
    - "Update the module docs to state which is which"
    - "Report the blast radius before editing (ResourceBudget: 38 upstream, MEDIUM)"

  out_of_scope:
    - "Changing enforcement's threshold semantics for non-contested resources"
    - "The capacity ledger implementation itself"

  affected_layers:
    domain:
      - "Modified: enforcement/domain/config.rs ResourceBudget (deprecate, or reduce to a view)"
    application:
      - "Modified: enforcer_impl.rs usage_ratio / limit_reached paths"
    infrastructure:
      - "Modified: default_policy_repository if the config shape changes"
    api:
      - "Possibly modified: surfaces that render budget status"

  canonical_references:
    - module: ".pi/architecture/modules/capacity-ledger.md"
    - adr:
        - ".pi/architecture/decisions/ADR-018-distributed-capacity-ledger.md"
    - evidence: "engine/src/enforcement/domain/config.rs:513"
    - evidence: "engine/src/enforcement/application/enforcer_impl.rs:109-129"

  acceptance_criteria:
    - "For any given resource, exactly one mechanism is authoritative — stated in docs and enforced in code"
    - "ResourceBudget's status is either a read-only view over the ledger, or explicitly limited to non-contested use with a doc comment saying so"
    - "Impact analysis recorded in the issue/PR (upstream count + direct callers)"
    - "Existing enforcement tests pass unchanged"

  validators:
    - ci
    - tests
    - architecture
    - canonical

  implementation_notes: |
    - Do not delete ResourceBudget blindly: it has 38 upstream references and is used by the
      standard/strict/maximum presets.
    - The risk is not the type; it is having two answers to "how much of this resource is
      used?" — pick one.

  file_changes:
    - "modify: engine/src/enforcement/domain/config.rs"
    - "modify: engine/src/enforcement/application/enforcer_impl.rs"
    - "modify: engine/.pi/architecture/modules/capacity-ledger.md (cross-reference)"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

## Update — 2026-10-18 (dependency LANDED; the decision is now actionable)

The facts remain true, but the blocker is gone.

**Dependency satisfied.** The capacity-ledger dispatch seam landed in `a861e2a8`
(`feat(capacity-ledger): wire the seam into the dispatch path`). The engine now performs
reserve → dispatch → commit/release on the real `run_dispatch_loop` path, armed from
`.rigorix/capacity.toml` by `CapacitySetup::from_env` and attached at all three production
composition roots (CLI, MCP host, GitHub Action). Proof: `tests/capacity_ledger_composition_e2e.rs`
drives the real loop, and the over-cap case **fails if the seam call is removed** (verified by
removal).

**Re-verified evidence, today:**

| fact | evidence (current) |
|---|---|
| generic resource concept | `enforcement/domain/config.rs:513` — `ResourceBudget { resource, soft_warning_threshold, hard_limit, current_usage }` |
| it is a counter | `enforcement/application/enforcer_impl.rs:109-129` — `usage_ratio`, `current_usage >= hard_limit` |
| no reservation concept | `grep reserve engine/src/enforcement` → zero hits |
| **still zero integration** | `grep -rc 'capacity_ledger' engine/src/enforcement` → **0 files** |
| blast radius (grep) | **31 references across 9 files** |
| blast radius (graph) | 38 upstream symbols, MEDIUM risk, 7 direct (mostly tests) |

So there are now **two working mechanisms for "a limited resource" and nothing connects
them**: `ResourceBudget` counts and compares; `CapacityLedger` reserves atomically and is
wired into dispatch. Exactly the ambiguity this issue exists to remove.

**Decision required (unchanged):** view over the ledger / deprecated / explicitly scoped to
non-contested limits. Two considerations that are now concrete rather than hypothetical:

- `ResourceBudget` is per-run in-memory and cannot see other executions; the ledger can. For
  anything contested, the counter is not merely redundant — it is **wrong**.
- Non-contested per-run limits (tool calls, timeout) are legitimately not ledger-shaped, so a
  blanket "port everything to the ledger" is also wrong.

A likely landing zone: keep `ResourceBudget` for per-execution, non-contested budget keys and
state that explicitly in its doc comment; route anything whose limit is shared across
executions through the capacity ledger. Either way, **the doc must say which is which**.

**Acceptance (unchanged):** exactly one authoritative mechanism per resource, stated in docs
and enforced in code; existing enforcement tests pass; do not delete `ResourceBudget` blindly.

---

# ISSUE-RESOURCE-BUDGET-COUNTER-SUPERSEDED

## Intent

Remove the ambiguity between the existing generic counter and the new reservation primitive
before two teams adopt different answers.

## Evidence

| fact | evidence |
|---|---|
| generic resource concept | `config.rs:513` — `ResourceBudget { resource: String, ... }` |
| it is a counter | `enforcer_impl.rs:109-129` — `usage_ratio`, `current_usage >= hard_limit` |
| no reservation concept exists there | `grep reserve engine/src/enforcement` → zero hits |
| impact | 38 upstream symbols, MEDIUM risk, 7 direct (mostly tests) |

## In Scope

Decide the relationship, implement it, document which is authoritative.

## Out of Scope

Threshold semantics for non-contested resources; the ledger implementation.

## Notes

The type stays; the duplication goes.
