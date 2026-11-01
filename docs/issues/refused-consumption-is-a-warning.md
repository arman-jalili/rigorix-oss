---
guardian_issue:
  id: "ISSUE-REFUSED-CONSUMPTION-IS-A-WARNING"
  title: "a refused consumption is a warning on the terminal path, so a double dispatch is recorded nowhere and the run completes"
  epic: "authority-boundary"
  component: "Approval"
  module: "approval"
  status: planned
  priority: high
  tracking_issue: 1263
  dependencies:
    - "#1261 — the insert-if-absent ledger write"
    - "#1241 — the anti-rollback marker"

  intent: |
    The approval is verified before dispatch (`dispatch.rs:386`) and consumed **after** the terminal
    outcome (`:568`). Nothing is held across the window, so two dispatchers both verify, both dispatch,
    and only then do both consume. #1262 made the second consumption refuse — and the refusal is
    discarded:

    ```rust
    match binding.service.consume(node_id).await {
        Ok(()) => tracing::info!(...),
        Err(ApprovalServiceError::NotFound(_) | ApprovalServiceError::AlreadyConsumed(_)) => {}
        Err(e) => tracing::warn!(node_id = %node_id, error = %e, "consume failed (non-fatal)"),
    }
    ```

    `ConsumptionReplay` is not `NotFound` and not `AlreadyConsumed`, so it lands in the `warn!` arm.
    The guard that exists to stop a replay therefore reports it to a log line, after the second
    dispatch has already happened, and the run continues to completion as if it were legitimate.

    This is not a log-level preference. The store's purpose is to make an approval authorize exactly
    one dispatch, and when it fails the failure leaves no trace in the evidence: the audit envelope of
    the second run records a completed node, the ledger holds one marker, and the two agree.

  in_scope:
    - "A refused consumption is recorded as evidence on the run whose consumption was refused — a named event/annotation in that run's envelope, not a log line"
    - "The node whose consumption was refused is reported as an integrity failure for that run, not as a completed node"
    - "The first consumption is unaffected"
    - "Seeds: two repositories on one store consume the same approval → the second's run carries the refusal event and its node is not Completed; the ledger still holds one marker with the first recorded_at"

  out_of_scope:
    - "Moving the consumption earlier — that is ADR-020 D6 and it changes the dispatch path"
    - "Raising the log level"
    - "The `NotFound` and `AlreadyConsumed` arms, which are benign"

  affected_layers:
    domain:
      - "None"
    application:
      - "Modified: consume_on_terminal distinguishes a refusal from a benign miss"
    infrastructure:
      - "None"

  citations:
    - file: "engine/src/execution_engine/application/service_impl/dispatch.rs"
      note: ":386 verify_intent before dispatch; :568 consume_on_terminal after the outcome, with the warn! arm"
    - file: "engine/src/approval/infrastructure/repository/approval_repository_impl.rs"
      note: ":678 record_consumption — an existing marker refuses with ConsumptionReplay"

  verification:
    - "two dispatchers, one approval: the second run's envelope carries the refusal, its node is not Completed"
    - "the ledger holds one marker and the first recorded_at"
---

# ISSUE-REFUSED-CONSUMPTION-IS-A-WARNING

## Measured

By reading the two call sites, which are unambiguous:

```
dispatch.rs:386   verify_intent(node_id)        -> Matched -> dispatch
dispatch.rs:568   consume_on_terminal(node_id)  -> Ok -> info!
                                                -> ConsumptionReplay -> warn! "consume failed (non-fatal)"
```

## Why it is worse than a noisy log

A replay refusal has exactly one consumer: whoever reads the evidence afterwards. The refusal is
written to the process log — which is not the evidence, not retained, and not part of the artifact a
recipient gets. The envelope for the second run says the node completed; the ledger says the approval
was consumed once; so both artifacts agree that one dispatch happened, while two did.

`AlreadyConsumed` being benign is right — it is the same decision consuming twice in one run.
`ConsumptionReplay` is a different fact: **another decision consumed it.**

## The required end state

The refusal is evidence on the run it happened in, and the run does not represent the node as
completed.

## How it is proven

| seed | expected |
|---|---|
| two repositories, one store, both consume | second returns the refusal; its run carries the event; its node is not Completed |
| the ledger after both | one marker, the first `recorded_at` |
| the refusal arm restored to `warn!` | the first seed fails |
