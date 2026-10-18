---
guardian_issue:
  id: "ISSUE-CAPACITY-GUARD-DTO"
  title: "capacity_ledger: ReserveCapacityOutput::guard is always None — the DTO advertises an RAII handle that does not exist"
  epic: "capacity ledger"
  component: "CapacityLedger"
  module: "capacity-ledger"
  status: planned
  priority: medium
  dependencies:
    - "ADR-018 capacity ledger (the seam settles by reservation id)"
    - "ADR-008 RAII reservation pattern (the pattern this field claims to offer)"
    - "Contract freeze ISSUE-CONTRACT-FREEZE (the DTO is frozen; this is a contract change)"

  intent: |
    Resolve a false contract. ReserveCapacityOutput.guard is documented as "RAII handle
    owning the reservation... dropping it without committing releases the hold", but
    CapacityLedgerServiceImpl::reserve hard-codes guard: None, so the field is never
    populated and ReservationGuardImpl is constructed only inside its own tests. There are
    two competing ownership designs in the code (by-id settlement in the service vs a
    drop-releasing guard), and only one of them is wired. Pick one, make the DTO honest,
    and stop implying a release-on-drop guarantee that the code does not provide.

  in_scope:
    - "Decide the ownership model: who owns the Grant — the service registry or the caller's guard"
    - "Make ReserveCapacityOutput stop advertising a handle it never returns"
    - "Either bring ReservationGuardImpl into production use, or delete it"
    - "Keep by-id commit/release working (the dispatch seam and the HTTP adapter both use it)"
    - "Update the DTO doc comment, the module doc and the seam's cross-reference"
    - "Retain release-on-drop semantics somewhere real (service teardown), or state plainly that leases are the only backstop"

  out_of_scope:
    - "Changing the HTTP wire shape (the field is #[serde(skip)]; see Wire impact below)"
    - "Changing lease semantics or the atomic reserve transaction"
    - "The dispatch seam's reserve/commit/release ordering (already correct)"

  affected_layers:
    domain:
      - "None (Grant ownership is unchanged in the domain)"
    application:
      - "Modified: dto/mod.rs — ReserveCapacityOutput.guard removed or re-typed"
      - "Modified: service_impl.rs — the grant/guard registry, take_grant, commit, release"
      - "Modified: guard_impl.rs — either used in production or deleted"
    infrastructure:
      - "Modified: http.rs — decodes ReserveCapacityOutput; already ignores the field"
    api:
      - "None on the wire; Rust API consumers of ReserveCapacityOutput are affected"

  canonical_references:
    - module: ".pi/architecture/modules/capacity-ledger.md#reservationguard"
    - adr:
        - ".pi/architecture/decisions/ADR-018-distributed-capacity-ledger.md"
        - ".pi/architecture/decisions/ADR-008-raii-budget-reservation.md"
    - evidence: "engine/src/capacity_ledger/application/dto/mod.rs:64-67"
    - evidence: "engine/src/capacity_ledger/application/service_impl.rs:96-100"
    - evidence: "engine/src/capacity_ledger/application/service_impl.rs:17-18"
    - evidence: "engine/src/capacity_ledger/application/guard_impl.rs:226,240,258 (tests only)"

  acceptance_criteria:
    - "No public DTO field documents a guarantee the implementation does not provide"
    - "ReservationGuardImpl has either a production caller or zero references (no test-only type in the public API)"
    - "Exactly one owner of a Grant at any time — no double-settle path exists"
    - "By-id commit and release still work (the dispatch seam's reachability test still passes)"
    - "Release-on-drop is either real (guards dropped with the service) or explicitly disclaimed in the docs"
    - "The frozen-DTO change is recorded: the field is #[serde(skip)], absent from every response schema, and nowhere read by a caller — so this is a Rust-API change, not a wire change"
    - "validate-architecture.sh and validate-canonical.sh pass"

  validators:
    - ci
    - tests
    - architecture
    - canonical

  implementation_notes: |
    - Recommended: the service owns the guards. Store Box<dyn ReservationGuard> in the
      registry keyed by ReservationId; reserve inserts; commit(id)/release(id) take from it
      and settle through the guard; dropping the service drops every outstanding guard and
      releases it. Then remove ReserveCapacityOutput.guard (it cannot be handed out without
      breaking by-id settlement) and ReservationGuardImpl becomes production-live. This keeps
      the by-id API, restores RAII at service lifetime, and removes the double owner.
    - Do NOT hand out the guard while the service also holds the grant: that is two owners of
      one reservation and a guaranteed double-settle attempt.
    - Do NOT just correct the comment. The type would stay dead and the next contributor would
      re-derive the same confusion.
    - The seam currently settles by id and documents why (service_impl.rs:552); that note must
      be updated with whatever this issue decides.
    - Because #[serde(skip)] excludes the field from the wire, no client can be relying on it;
      no SDK catalog change is needed.

  file_changes:
    - "modify: engine/src/capacity_ledger/application/dto/mod.rs"
    - "modify: engine/src/capacity_ledger/application/service_impl.rs"
    - "modify or delete: engine/src/capacity_ledger/application/guard_impl.rs"
    - "modify: engine/src/capacity_ledger/infrastructure/http.rs (if the type changes)"
    - "modify: engine/src/execution_engine/application/service_impl.rs (seam cross-reference)"
    - "modify: engine/.pi/architecture/modules/capacity-ledger.md"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-CAPACITY-GUARD-DTO — `ReserveCapacityOutput::guard` is always `None`

## Intent

The DTO advertises a release-on-drop handle. The service never returns one. Resolve the
contradiction rather than documenting it away.

## Evidence

| claim | evidence |
|---|---|
| the field promises release-on-drop | `dto/mod.rs:64-67` — *"RAII handle owning the reservation… dropping it without committing releases the hold"* |
| it is never populated | `service_impl.rs:96-100` — `guard: None`, hard-coded |
| the code knows | `service_impl.rs:17-18` — *"RAII guard wiring lands in ISSUE-CAPACITY-LEDGER-8; `reserve` currently returns `guard: None`"* |
| the guard type is production-dead | `ReservationGuardImpl::new` appears **only** at `guard_impl.rs:226,240,258`, all inside `#[cfg(test)]` |
| the service settles by id instead | `take_grant(&ReservationId)` + the `grants` registry, used by `commit`/`release` |

So there are **two ownership designs** and only the by-id one is wired.

## Why it matters

A caller who reads the DTO contract will write:

```rust
if let Some(guard) = out.guard { /* rely on drop to release */ }
```

That branch is unreachable. The caller concludes either that no hold exists (it does) or that
dropping releases it (it does not) — and capacity then strands until the lease expires. The
documented guarantee is the opposite of the behaviour, which is the same defect class as the
unwired port and the dormant gate: *the shape of the API promises something the code does not do*.

## Wire impact: none

- The field is `#[serde(skip)]`, so it is absent from every serialized payload.
- No JSON schema names `ReserveCapacityOutput` (`interfaces/http/mod.rs` uses the name only as a
  Rust-side `response_schema` label for `EndpointContract`).
- `HttpLedger` decodes `ReserveCapacityOutput` and already reads only `.reservation`, never
  `.guard`.
- Therefore removing the field is a **Rust API change, not a wire change** — no SDK catalog
  change and no client impact.

## Decision required

**Option A (recommended) — the service owns the guards; remove the DTO field.**
Registry holds `Box<dyn ReservationGuard>` keyed by `ReservationId`. `reserve` inserts;
`commit(id)`/`release(id)` take and settle through the guard; dropping the service drops every
outstanding guard and releases it. By-id APIs keep working (the seam and the HTTP adapter need
them), `ReservationGuardImpl` becomes live, and RAII applies at service lifetime — strictly
better than per-call.

**Option B — hand out the guard and retire the by-id APIs.** Correct RAII per reservation, but
it breaks the HTTP adapter (a remote reservation cannot return a local guard) and requires the
seam to hold guards. Larger, and the HTTP adapter must keep the by-id path anyway.

**Option C — keep the behaviour, delete the field, state the lease is the only backstop.**
Smallest change; honest. Loses the immediate-release property on teardown.

All three are acceptable; **doing nothing is not**, because the contract currently lies.

## Acceptance Criteria

See front matter. The load-bearing ones: no DTO field promises an unimplemented guarantee, no
public type is production-dead, exactly one owner per `Grant`, and the dispatch seam's
reachability test still passes.

## Notes

This does not block anything: the seam already settles by id and documents why
(`engine/src/execution_engine/application/service_impl.rs:552`). It is a correctness-of-contract
fix, not a functional gap.
