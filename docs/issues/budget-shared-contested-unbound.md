---
guardian_issue:
  id: "ISSUE-BUDGET-SHARED-CONTESTED-UNBOUND"
  title: "enforcement: BudgetScope::SharedContested defers the limit and nothing takes over — no binding to a capacity-ledger resource"
  epic: "capacity ledger"
  component: "Enforcement"
  module: "enforcement"
  status: planned
  priority: high
  dependencies:
    - "#1017 / PR #1056 — introduced BudgetScope and made the counter advisory for contested limits"
    - "ADR-018 — the capacity ledger is the authoritative mechanism for contested limits"
    - "The dispatch seam (a861e2a8) — the only place capacity is currently enforced"

  intent: |
    Close a hole created by the split decision. #1017 correctly established that a local
    counter must not block a contested limit, and documented that 'the capacity ledger is
    authoritative'. But nothing connects the two: enforcement never references the ledger,
    a shared_contested budget creates no capacity resource, and no validation requires the
    operator to declare one.

    So an operator who sets scope = "shared_contested" gets exactly one effect: local
    enforcement stops. Whether anything else enforces the limit depends on a *separate*
    config surface (.rigorix/capacity.toml) that may not exist. The safe-looking change
    silently disables the control.

  in_scope:
    - "Bind a shared_contested budget to a capacity-ledger resource (by name) or refuse the config"
    - "Validate at config load: a shared_contested budget must name a declared capacity resource"
    - "Surface the binding in budget status (which mechanism is authoritative, and is it armed)"
    - "Fail closed when the binding is missing or the declared resource is absent"
    - "Document the two-surface relationship: enforcement declares which limits are contested; capacity.toml declares the limits themselves"

  out_of_scope:
    - "Re-doing the #1017 decision (a local counter must not block a contested limit — that is correct)"
    - "Implementing the ledger's window/ambiguity semantics (#1051, #1050)"
    - "Migrating every existing budget"

  affected_layers:
    domain:
      - "Modified: ResourceBudget — a capacity resource reference for SharedContested"
      - "Modified: validation errors — missing/unknown capacity binding"
    application:
      - "Modified: config validation (config_service_impl) — enforce the binding"
      - "Modified: budget status — report the authoritative mechanism and whether it is armed"
    infrastructure:
      - "None (the ledger is reached through the dispatch seam today)"
    api:
      - "Modified: budget status output gains the authoritative-mechanism field"

  canonical_references:
    - module: ".pi/architecture/modules/enforcement.md"
    - adr:
        - ".pi/architecture/decisions/ADR-018-distributed-capacity-ledger.md"
    - evidence: "engine/src/enforcement/domain/config.rs:526 — BudgetScope { PerExecution, SharedContested }, settable via serde snake_case"
    - evidence: "engine/src/enforcement/domain/config.rs:537 — is_authoritative() == false for SharedContested (the counter stops blocking)"
    - evidence: "engine/src/enforcement/application/enforcer_impl.rs — the SharedContested test asserts only that it does NOT block"
    - evidence: "grep -rn 'capacity_ledger' engine/src/enforcement/ — a doc comment and nothing else"
    - evidence: "engine/src/capacity_ledger/application/config.rs — capacity resources come from .rigorix/capacity.toml, a separate surface with no link to enforcement budgets"

  acceptance_criteria:
    - "A shared_contested budget that names no capacity resource is refused at config load (fail closed)"
    - "A shared_contested budget naming an undeclared resource is refused"
    - "Budget status distinguishes 'advisory, ledger armed' from 'advisory, nothing is enforcing'"
    - "Test: configuring shared_contested alone does not silently disable the limit"
    - "The two-surface relationship is documented in the enforcement module doc"

  validators:
    - ci
    - tests
    - architecture
    - canonical

  implementation_notes: |
    - The trap is that the failure is silent and looks like a safety improvement: the operator
      'scopes' the budget and the counter stops blocking. Nothing warns.
    - Two workable shapes: (a) the budget names a capacity resource id and validation requires
      it to exist; (b) the ledger's declaration names the budgets it governs. Pick one owner
      for the binding — two surfaces each assuming the other is how this happened.
    - Fail closed. A contested limit with no authoritative mechanism is worse than a local
      counter that over-blocks.
    - Do not re-introduce the counter as the authority for contested limits; that is what
      #1017 correctly removed.

  file_changes:
    - "modify: engine/src/enforcement/domain/config.rs"
    - "modify: engine/src/enforcement/application/config_service_impl.rs"
    - "modify: engine/src/enforcement/application/enforcer_impl.rs"
    - "create: engine/tests/unit/enforcement/shared-contested-binding_test.rs"
    - "modify: engine/.pi/architecture/modules/enforcement.md"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-BUDGET-SHARED-CONTESTED-UNBOUND — the limit is deferred to nobody

## The defect

`#1017` correctly decided that a **local** counter cannot enforce a **contested** limit, and
introduced `BudgetScope`:

```rust
pub enum BudgetScope {
    #[default] PerExecution,   // this counter is authoritative
    SharedContested,           // "the capacity ledger enforces it; this counter is advisory"
}
```

The counter correctly stops blocking. **Nothing takes over.**

## Evidence

| claim | evidence |
|---|---|
| the scope is operator-settable | `config.rs:526` — `Serialize, Deserialize`, `rename_all = "snake_case"` |
| the counter stops blocking | `config.rs:537` — `is_authoritative()` is `false` for `SharedContested` |
| the test asserts only the deferral | `enforcer_impl.rs` — *"a shared/contested budget must not block locally"* |
| enforcement never reaches the ledger | `grep -rn 'capacity_ledger' engine/src/enforcement/` → **one doc comment** |
| capacity resources live in a different surface | `.rigorix/capacity.toml` via `CapacitySetup::from_env` |
| no validation links them | config validation checks `budget_key` → budgets only |

## The failure mode

```toml
# an operator tightening a shared limit
[budgets.shared]
resource = "shared"
hard_limit = 100
scope = "shared_contested"        # <-- from here, nothing blocks
```

Before the edit, the counter blocked (too eagerly, and wrongly across executions). After the
edit, **nothing** blocks unless the operator *also* hand-wrote a matching
`.rigorix/capacity.toml` with a resource and a per-step grant rule. Nothing warns, and the
budget status shows a limit that is not being enforced.

A change that looks like a safety improvement removes the control.

## Why it matters

This is the same class as the dormant gate and the unwired port: **a decision implemented as a
deferral, with no one to defer to.** #1017's reasoning was right; the binding is missing.

## Acceptance Criteria

See front matter. Load-bearing: **configuring `shared_contested` alone must not silently
disable the limit** — refuse the config, or report loudly that nothing is enforcing.
