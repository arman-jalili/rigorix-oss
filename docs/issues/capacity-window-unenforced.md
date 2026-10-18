---
guardian_issue:
  id: "ISSUE-CAPACITY-WINDOW-UNENFORCED"
  title: "capacity_ledger: ResourceSpec.window is accepted, stored and echoed, but never enforced — committed exposure never ages out"
  epic: "capacity ledger"
  component: "CapacityLedger"
  module: "capacity-ledger"
  status: planned
  priority: high
  dependencies:
    - "ADR-018 distributed capacity ledger"

  intent: |
    Make the declared window mean something, or stop accepting it. Today window_seconds is
    parsed into ResourceSpec.window, carried into Exposure, and then ignored by every
    decision: availability is computed from two lifetime counters (committed, outstanding)
    that are never decremented by time. A resource declared with window_seconds = 3600 keeps
    its committed total forever, so the second hour is refused with the first hour's spend
    still counted.

    This is a silent-wrong-answer defect, not a missing feature: the operator declares a
    period, validation accepts it, exposure reports it back, and the engine enforces a
    different rule (lifetime) without saying so.

  in_scope:
    - "Define the window semantics explicitly: rolling duration, calendar period, or fixed execution window"
    - "Enforce it in availability: committed exposure outside the window must stop counting"
    - "Define behaviour at a window boundary (a reservation that straddles it)"
    - "State how expired/ambiguous reservations interact with the window"
    - "Reject a window the implementation cannot honour (fail closed) instead of accepting and ignoring it"
    - "Report in Exposure which period the numbers cover, and whether the window is enforced"
    - "Mirror the same semantics in the enterprise service (the HTTP adapter calls it)"

  out_of_scope:
    - "Calendar/timezone configuration beyond what the chosen semantics require"
    - "Retention or archival of old reservations"

  affected_layers:
    domain:
      - "Modified: Window — a defined semantic (rolling vs calendar vs fixed) rather than a bare duration"
      - "Modified: Exposure — period start/end and whether the window is enforced"
    application:
      - "Modified: config validation — refuse windows that cannot be honoured"
    infrastructure:
      - "Modified: InProcessLedger — time-bucketed or rolling aggregate instead of lifetime counters"
      - "Modified: HttpLedger / enterprise repository — the SQL aggregate must apply the window"
    api:
      - "Modified: exposure output semantics"

  canonical_references:
    - module: ".pi/architecture/modules/capacity-ledger.md#resource"
    - adr:
        - ".pi/architecture/decisions/ADR-018-distributed-capacity-ledger.md"
    - evidence: "engine/src/capacity_ledger/domain/resource.rs:56 — Window { duration_secs }"
    - evidence: "engine/src/capacity_ledger/infrastructure/in_process.rs — committed/outstanding are HashMap<ResourceId, u64>, no timestamps"
    - evidence: "engine/src/capacity_ledger/infrastructure/in_process.rs — the window is only copied into Exposure::new"
    - evidence: "rigorix-enterprise core/src/capacity_ledger/infrastructure/capacity_repository.rs — no window reference in the aggregate"
    - evidence: "engine/src/capacity_ledger/application/config.rs — window_seconds is parsed and accepted"

  acceptance_criteria:
    - "A declared window is enforced in the availability computation (test: spend to the cap, advance past the window, the capacity is available again)"
    - "A window that the implementation cannot honour is refused at declaration, never silently ignored"
    - "Exposure states the period its numbers cover"
    - "Boundary behaviour is defined and tested (reservation taken at t=window-1s)"
    - "The enterprise service applies the same rule, or the contract explicitly narrows to rolling"
    - "window_seconds = 0 (unbounded) remains explicit and documented rather than the accidental default"
    - "Docs updated: ADR-018 and the module doc state the semantics"

  validators:
    - ci
    - tests
    - architecture
    - canonical

  implementation_notes: |
    - Pick ONE semantic and say so. Rolling duration is the least surprising for a cap and is
      implementable with time-bucketed counters or a timestamped query; a calendar period
      needs a timezone and a reset moment; a fixed execution window needs a boundary owner.
    - Prefer deriving the aggregate from reservation timestamps over decrementing counters:
      a counter that must be decremented on a timer is a second source of truth.
    - The in-process adapter currently keeps counters with no timestamps, so this is a
      representation change, not a patch.
    - Unbounded must stay representable: existing declarations with window_seconds omitted
      currently mean unbounded.
    - Do NOT implement this as "reset the counter on a timer" — that reintroduces the
      naive-counter class of bug and breaks under concurrency.

  file_changes:
    - "modify: engine/src/capacity_ledger/domain/resource.rs"
    - "modify: engine/src/capacity_ledger/domain/exposure.rs"
    - "modify: engine/src/capacity_ledger/infrastructure/in_process.rs"
    - "modify: engine/src/capacity_ledger/application/config.rs"
    - "create: engine/tests/capacity_ledger_window.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-CAPACITY-WINDOW-UNENFORCED — the window is decorative

## The defect

`window_seconds` is parsed, stored in `ResourceSpec`, and copied into `Exposure` — then
ignored by every decision. Availability comes from two **lifetime** counters that time never
touches.

```text
declared: hard_limit = 100, window_seconds = 3600
hour 1:   spend 100  -> committed = 100
hour 2:   reserve 1  -> Insufficient (available 0)      # window never elapsed
```

The operator asked for a per-hour cap and got a lifetime cap. Validation accepted the
declaration; `Exposure` echoes the window back as if it were in force.

## Evidence

| claim | evidence |
|---|---|
| a window type exists | `domain/resource.rs:56` — `Window { duration_secs }` |
| it is parsed and accepted | `application/config.rs` — `window_seconds` |
| the aggregate has no time dimension | `in_process.rs` — `committed`/`outstanding` are `HashMap<ResourceId, u64>` |
| the window only reaches output | `in_process.rs` — `Exposure::new(..., window)` |
| the enterprise has no window either | `capacity_repository.rs` — no `window` reference in the aggregate |

## Why it matters

This is the **silent** class: nothing errors, nothing warns, and the enforcement rule differs
from the declared rule. A reader of the module doc would reasonably assume the window is
honoured because `ResourceSpec` carries it.

## Decision required

Name one semantic and enforce it:

- **Rolling duration** — "at most N units in any trailing W" — least surprising, needs
  timestamps.
- **Calendar period** — "at most N per day/month" — needs a timezone and a reset moment.
- **Fixed execution window** — "at most N per run" — needs an explicit owner of the boundary.

Whatever is chosen must also answer: what happens to a reservation straddling the boundary,
and how do expired/ambiguous reservations count toward it.

## Acceptance Criteria

See front matter. The load-bearing test: **spend to the cap, advance past the window, and the
capacity becomes available again.** Unbounded (`window_seconds` absent or 0) must stay
representable.
