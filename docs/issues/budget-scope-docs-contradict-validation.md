---
guardian_issue:
  id: "ISSUE-BUDGET-SCOPE-DOCS-CONTRADICT-VALIDATION"
  title: "enforcement: the BudgetScope docs instruct operators to use shared_contested, which validation always refuses"
  epic: "capacity ledger"
  component: "Enforcement"
  module: "enforcement"
  status: planned
  priority: medium
  dependencies:
    - "#1061 / PR #1062 — refuse unbound shared_contested budgets"

  intent: |
    Make the documentation agree with the behaviour #1061 shipped. Validation now rejects
    every budget whose scope is `SharedContested`, because no binding from `budgets` to a
    declared ledger resource exists. But both the enum doc and the field doc still present
    `SharedContested` as the supported way to express a contested limit, and the field doc
    says "use" it.

    A reader who follows the docs gets a configuration error. That is the same class of
    defect as #1060 — prose left behind when behaviour changed — which makes it worth fixing
    rather than tolerating.

  in_scope:
    - "State in the BudgetScope doc that SharedContested is refused until a binding exists"
    - "Fix the ResourceBudget.scope field doc: it must not instruct operators to use it"
    - "Explain the supported path: declare the resource in .rigorix/capacity.toml"
    - "Make the runtime warning and the validation message say the same thing"
    - "Reconcile the enforcement module doc with the shipped behaviour"

  out_of_scope:
    - "Implementing the binding (that is #1061's remaining in_scope item, if it is picked up)"
    - "Removing the enum variant — it is the reservation for the eventual binding"
    - "The capacity ledger's own semantics"

  affected_layers:
    domain:
      - "Modified: BudgetScope and ResourceBudget doc comments only"
    application:
      - "None"
    infrastructure:
      - "None"
    api:
      - "None"

  canonical_references:
    - module: ".pi/architecture/modules/enforcement.md"
    - evidence: "engine/src/enforcement/domain/config.rs:455 — validation pushes an error for every SharedContested budget"
    - evidence: "engine/src/enforcement/domain/config.rs:536 — the enum doc presents SharedContested as a supported mechanism"
    - evidence: "engine/src/enforcement/domain/config.rs:588 — the field doc says 'use BudgetScope::SharedContested'"
    - evidence: "engine/src/enforcement/application/enforcer_impl.rs:80 — the runtime warning calls it 'advisory only'"

  acceptance_criteria:
    - "No doc comment instructs an operator to set shared_contested"
    - "The doc states it is refused, and names the supported alternative"
    - "The enum doc, the field doc, the validation message and the runtime warning agree"
    - "validate-docs.sh and validate-canonical.sh pass"

  validators:
    - ci
    - canonical
    - docs

  implementation_notes: |
    - Small, but it is the third instance of the same failure mode in this area (dormant gate,
      unwired port, stale ports doc). Document-as-you-change is the actual fix; this issue is
      the cleanup.
    - Keep the variant and say why: it marks the shape the binding will need, and removing it
      would make #1061's deferral invisible.
    - The runtime warning currently says "advisory only: no local limit applies". If validation
      refuses the config, the process-constructed case is the only one that reaches it. Say so,
      so the warning is not read as "the limit is advisory" by someone who never ran validation.

  file_changes:
    - "modify: engine/src/enforcement/domain/config.rs"
    - "modify: engine/src/enforcement/application/enforcer_impl.rs"
    - "modify: engine/.pi/architecture/modules/enforcement.md"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-BUDGET-SCOPE-DOCS-CONTRADICT-VALIDATION

## The contradiction

#1061 shipped a fail-closed refusal:

```rust
// engine/src/enforcement/domain/config.rs:455
if budget.scope == BudgetScope::SharedContested {
    errors.push(ConfigValidationError {
        field: format!("budgets.{}.scope", name),
        message: "… declare the resource in .rigorix/capacity.toml (capacity ledger) and remove this budget",
        ...
    });
}
```

No binding from `budgets` to a declared ledger resource was added, so the scope is now
**always refused**. That is the right fail-closed call.

The docs were not updated:

| location | currently says |
|---|---|
| `config.rs:536` | "`BudgetScope::SharedContested` — the capacity ledger is authoritative. This counter is advisory and never blocks" |
| `config.rs:588` | "use [`BudgetScope::SharedContested`] for a limit the capacity ledger enforces" |

So the field doc **instructs the operator to do the thing validation rejects.**

## Why file this separately

It is not cosmetic. The enum doc is the only in-code explanation of the two mechanisms, and it
reads as a supported option. An operator who follows it gets:

```
budgets.shared.scope: budget 'shared' is shared_contested, but the enforcement counter
cannot enforce a shared limit; declare the resource in .rigorix/capacity.toml …
```

Correct error, misleading path to it — and the same class of drift as #1060 (prose left behind
when behaviour changed), filed hours apart in the same area.

## Acceptance

See front matter. Load-bearing: **no doc comment instructs setting `shared_contested`**; the
enum doc, field doc, validation message and runtime warning must all say the same thing.
