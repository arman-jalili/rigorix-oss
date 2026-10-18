# Batch 2 — Issues #1052 + #1050

**Branch:** `feature/capacity-idempotency-ambiguity`
**Why together:** one key lifecycle. Expiry must not delete the key (#1052) and must not
release an unknown outcome (#1050); both changes live in `expire_lapsed`.

## #1052 — idempotency payload mismatch
- Same key + same amount → idempotent hit (unchanged).
- Same key + **different** amount → `LedgerError::Contended` with the existing/requested amounts.
- Key bound to an `Ambiguous` reservation → `Contended`; reconcile the original first.
- Expiry no longer removes the key, so a retry cannot silently create a second reservation.

## #1050 — a lapsed lease is not proof of no effect
- New `ReservationState::Ambiguous` (non-terminal reconciliation state).
- `expire_lapsed` moves `Outstanding` → `Ambiguous`, **retains** outstanding exposure, keeps
  the key, logs a warning.
- A late `commit` proves success and reconciles (outstanding → committed); a late `release`
  proves no effect and hands capacity back.
- `Reservation::mark_ambiguous(now)` is lease-gated; `expire()` stays the definitive-void path.
- `Ambiguous` may only transition to a terminal state.

## Tests
- lapsed lease retains exposure and reconciles via late commit
- same key + different amount is a conflict; identical retry still collapses
- retry after expiry is rejected and does not create a second reservation
- `mark_ambiguous` is lease-gated and non-terminal

## Validation
ci, tests, architecture, canonical, docs, panic surface.
