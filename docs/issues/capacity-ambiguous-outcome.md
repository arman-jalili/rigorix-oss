---
guardian_issue:
  id: "ISSUE-CAPACITY-AMBIGUOUS-OUTCOME"
  title: "capacity_ledger: lease expiry is treated as proof the effect did not happen — there is no ambiguous-outcome state"
  epic: "capacity ledger"
  component: "CapacityLedger"
  module: "capacity-ledger"
  status: planned
  priority: critical
  dependencies:
    - "ADR-018 distributed capacity ledger"
    - "The dispatch seam (a861e2a8)"

  intent: |
    Close the double-exposure window. Today the ledger treats "the lease lapsed" as
    equivalent to "the effect did not happen" and hands the capacity back. For an external
    side effect that is false: a payment can succeed while the recording of that success is
    lost. The ledger then behaves correctly against its own state machine while real exposure
    exceeds the declared limit.

    The concrete path, currently reachable end to end:
      1. reserve 800 of a 1000 cap for a payment step
      2. the payment provider executes and the money moves
      3. the engine dies (or the step's result is lost) before commit
      4. the lease lapses; expire_lapsed releases the 800
      5. another session reserves 800 and pays again
      6. the first execution's late commit is rejected with LeaseExpired, so the ledger
         cannot even record that the first payment happened
    Two payments, one limit, no recorded violation.

    The ledger needs to distinguish reservation lifecycle from evidence about the effect.

  in_scope:
    - "Add an explicit non-terminal state for 'effect outcome unknown' (or an equivalent
       reconciliation record) that does NOT release capacity"
    - "Lease expiry must transition to that state, not to a capacity-releasing one, for
       resources declared to carry external side effects"
    - "Define the terminal resolutions: proven-succeeded -> commit; proven-not-started ->
       release; proven-failed-without-effect -> release (subject to the resource's rules)"
    - "Define how an unknown outcome is resolved (late commit, operator reconciliation, or
       an external confirmation source) and who may resolve it"
    - "The dispatch seam must stop mapping every non-success to release: a failure that
       cannot prove absence of effect must retain exposure"
    - "Expose the ambiguous count in exposure so an operator can see retained exposure"
    - "Declare, per resource, whether an external side effect is possible (the engine still
       learns no domain meaning, only whether retention applies)"

  out_of_scope:
    - "Implementing a reconciliation UI (the console work lives in the enterprise repo)"
    - "Deciding the operator's escalation policy"
    - "Changing the atomic reserve transaction"

  affected_layers:
    domain:
      - "New: ReservationState variant for unknown/unreconciled outcome (non-terminal)"
      - "New: resolution transitions (unknown -> committed | released | expired-unproven)"
      - "Modified: ResourceSpec — a side-effecting flag that selects the expiry policy"
      - "Modified: Exposure — retained/ambiguous exposure reported separately"
    application:
      - "Modified: expiry semantics in the service; resolution use case"
      - "Modified: dispatch seam settlement (failure is not proof of no effect)"
    infrastructure:
      - "Modified: InProcessLedger.reclaim + resolve (expiry policy per resource)"
      - "Modified: HttpLedger (the enterprise service must implement the same state)"
    api:
      - "Modified: exposure output gains retained/ambiguous; error taxonomy may gain a
         'resolution required' outcome"

  canonical_references:
    - module: ".pi/architecture/modules/capacity-ledger.md#reservation"
    - adr:
        - ".pi/architecture/decisions/ADR-018-distributed-capacity-ledger.md"
    - evidence: "engine/src/capacity_ledger/domain/reservation.rs — ReservationState has exactly Outstanding/Committed/Released/Expired"
    - evidence: "engine/src/capacity_ledger/infrastructure/in_process.rs — expire_lapsed releases outstanding capacity unconditionally"
    - evidence: "engine/src/capacity_ledger/infrastructure/in_process.rs — commit after expiry returns LedgerError::LeaseExpired"
    - evidence: "engine/src/execution_engine/application/service_impl/dispatch.rs — settle_capacity releases on any non-success"
    - counterpart: "rigorix-enterprise core/src/capacity_ledger/domain/value.rs — same four states"

  acceptance_criteria:
    - "A lapsed lease on a side-effecting resource does NOT make the capacity reusable"
    - "A late commit after the ambiguity window is recorded rather than rejected outright (or is explicitly rejected with the exposure retained)"
    - "An unknown outcome is visible in exposure (retained), and only a defined resolution releases or commits it"
    - "A failure with no proof of absence of effect retains exposure (the seam no longer releases blindly)"
    - "Reclaim never deletes the idempotency key of an ambiguous reservation (that would allow a fresh double reservation)"
    - "Tests: crash-before-commit does not allow a second full-size reserve; late commit is handled per the chosen policy"
    - "Documented in ADR-018 and the module doc as the accounting rule"

  validators:
    - ci
    - tests
    - architecture
    - canonical

  implementation_notes: |
    - This is an accounting-correctness issue, not a UX one. Retaining exposure for an
      unknown outcome is the safe default; releasing is the assumption that must be proven.
    - The engine must not learn what a resource means, but it does need to know whether an
      effect is possible (a boolean on the declaration). Anything side-effecting retains.
    - expire_lapsed currently also removes the idempotency key, so a retry after expiry
      creates a NEW reservation. That compounds this bug and must be resolved together.
    - Keep the states distinguishable on the wire; do not reuse 'Expired' for ambiguous.
    - The enterprise service must mirror the state machine or the HTTP adapter will diverge.

  file_changes:
    - "modify: engine/src/capacity_ledger/domain/reservation.rs"
    - "modify: engine/src/capacity_ledger/domain/resource.rs"
    - "modify: engine/src/capacity_ledger/domain/exposure.rs"
    - "modify: engine/src/capacity_ledger/infrastructure/in_process.rs"
    - "modify: engine/src/capacity_ledger/application/service_impl.rs"
    - "modify: engine/src/execution_engine/application/service_impl/dispatch.rs"
    - "modify: engine/src/capacity_ledger/application/config.rs (side-effecting flag)"
    - "create: engine/tests/capacity_ledger_ambiguous_outcome.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-CAPACITY-AMBIGUOUS-OUTCOME — a lapsed lease is not proof of no effect

## The defect

The ledger equates **"the lease lapsed"** with **"the effect did not happen"**, and hands the
capacity back. For an external side effect that is false.

### Currently reachable path

| step | what happens |
|---|---|
| 1 | reserve 800 of a 1000 cap for a payment step |
| 2 | the provider executes; the money moves |
| 3 | the engine dies (or the result is lost) before commit |
| 4 | the lease lapses → `expire_lapsed` releases the 800 |
| 5 | another session reserves 800 and pays again |
| 6 | the first execution's late commit → `LedgerError::LeaseExpired` |

**Two payments, one limit, no recorded violation.** The ledger was self-consistent the whole
time.

## Evidence

| claim | evidence |
|---|---|
| no state can express "outcome unknown" | `domain/reservation.rs` — exactly `Outstanding / Committed / Released / Expired` |
| expiry releases unconditionally | `infrastructure/in_process.rs` — `expire_lapsed` decrements `outstanding` and re-keys |
| a late success is rejected | `in_process.rs` — `commit` after expiry returns `LeaseExpired` |
| the seam releases on *any* non-success | `dispatch.rs` — `settle_capacity(node_id, success)`; `false` → release |
| the raw signal is only `task_result.success` | `dispatch.rs:289` |
| the enterprise has the same four states | `rigorix-enterprise core/src/capacity_ledger/domain/value.rs` |

## Why it matters

A timeout is not proof of failure. For a financial or otherwise consequential resource, the
unsafe assumption is **release**; the safe default is **retain until proven**.

## Required contract

| outcome | safe capacity treatment |
|---|---|
| execution definitely did not start | release |
| effect succeeded and is confirmed | commit |
| effect definitely failed with no side effect | release (subject to the resource's rules) |
| **effect outcome unknown** | **retain exposure; enter an explicit reconciliation state** |

The last row does not exist today.

## Notes

Compounded by the same function removing the idempotency key on expiry: a retry after expiry
creates a **new** reservation rather than recovering the original (see the idempotency issue).
The two must be resolved together.
