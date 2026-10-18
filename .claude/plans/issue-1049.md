# Batch 1 — Issue #1049

**Branch:** `issue/1049`
**Decision:** Option A — the service owns the guards; remove the DTO field.

## Plan
1. `CapacityLedgerServiceImpl` registry holds `ReservationGuardImpl` keyed by `ReservationId`.
2. `reserve` constructs the guard (production call) and returns no guard field.
3. `commit(id)` / `release(id)` take the guard from the registry; `release` calls a
   crate-internal `ReservationGuardImpl::release` (deterministic await, not drop-race).
4. Dropping the service drops every outstanding guard → their `Drop` releases
   (service-lifetime RAII, lease still the backstop).
5. Remove `ReserveCapacityOutput::guard`; update docs, seam cross-reference,
   module doc. Update the DTO test.

## Acceptance mapping
- No DTO field promises an unimplemented guarantee → field removed.
- `ReservationGuardImpl` has a production caller → constructed in `reserve`.
- Exactly one owner → the registry (no field escapes).
- By-id commit/release still work → unchanged API.
- Release-on-drop real → service teardown drops guards.
- `validate-architecture.sh` + `validate-canonical.sh` pass.
