# ADR-012: Failure Semantics on wasm32 — a Trap Is Not a Refusal, and `catch_unwind` Does Not Exist

**Status:** Accepted
**Date:** 2026-11-03
**Session:** 3baf6e5c-e352-4127-af09-a5e7fe0a2d29

## Context

NFR-004 as first written promised that *"an internal error, trap, exhausted fixture or
unsupported input yields a refusal, never an allow"*. Three of those four are
implementable; one is not.

`wasm32-unknown-unknown` has no stack unwinding. A Rust panic aborts the instance:
JavaScript receives a `RuntimeError`, the linear memory is unusable, and no refusal
value is ever constructed. `std::panic::catch_unwind` cannot recover from it — there is
no unwinder to catch, so any design that reads "catch the panic and return Refused" is
writing code for a platform that does not exist.

The requirement also conflates three different failure kinds, which need three
different guarantees and three different tests.

## Decision

Failure handling is tiered, and each tier states what it can and cannot promise.

| Tier | Kind | Behaviour | Enforcement |
|------|------|-----------|-------------|
| 1 | Logical failure — exhausted/missing/malformed fixture answer, unparseable scenario, unsupported artifact-format version, unusable port | **Typed refusal** carrying the engine's vocabulary (`unverified` for an identity port that could not be checked, never `unauthenticated`) | Fault-injection matrix; 100 % refusal, 0 % allow (criterion 4, 5) |
| 2 | Program defect — panic, unwrap on an unexpected `None`, index out of range | **Must not exist on the gate path.** A panic is a bug, not a handled case | `.pi/scripts/validate-panic-sites.sh` baseline for the crate; `unwrap_used`/`expect_used`/`panic` denied under the `portable` feature; every criterion that touches the aggregate runs it |
| 3 | Trap or instantiation failure — the module aborts after a tier-2 defect, or the browser refuses to compile the wasm | **Fatal and host-visible.** The host reports "the gate could not run" and never renders an allow | Host contract states it; the demo's error surface distinguishes *refused* from *crashed*; a host-contract test asserts the distinction |

Two API rules follow:

1. **No exported function panics.** The wasm surface returns `Result`-shaped DTOs for
   every outcome, and the mapper rejects unknown fields rather than defaulting them.
2. **No `catch_unwind`.** It is not merely useless — writing it signals a misunderstanding
   of the target. `panic = "abort"` semantics are documented in the crate root so the next
   reader does not try again.

The honest summary for the demo's limits panel: *a refusal is always a decision; a crash
is always visible and never looks like permission.*

## Consequences

### Positive
- The claim becomes true rather than aspirational: what can be guaranteed is guaranteed
  and tested, and what cannot is named.
- Tier 2 moves quality upstream — the panic baseline and the deny-lints catch the defect
  at build time instead of trying (and failing) to catch it at runtime.
- The host learns the difference between "the gate said no" and "the gate stopped",
  which is the same rule the engine already applies to `unverified` vs `unauthenticated`.

### Negative
- A panic in a released bundle remains a user-visible crash. Accepting this is the point:
  the alternative is a promise the platform cannot keep.
- The deny-lints make some ergonomic patterns unavailable (`unwrap` on a proven-`Some`)
  and force explicit error mapping throughout the gate path.

### Neutral
- The host obligation is new text in the contract, not new code in the crate.

## Alternatives Considered

- **`panic = "unwind"` via wasm exception-handling.** Rejected: toolchain- and
  host-dependent, doubles the bundle for a path that should never execute, and still
  leaves the instance state undefined after a caught panic.
- **A `Refused` fallback in the JS wrapper for traps.** Rejected as a governance lie: it
  would convert "the gate crashed" into "the gate refused", which is the same defect
  class as reporting `unverified` as `unauthenticated`.
- **Blanket `unwrap` bans without a baseline.** Rejected: the repo's ratchet exists so a
  new site is a decision, not a silent addition.

## Implementation

1. Crate root documents abort semantics and why `catch_unwind` is absent.
2. `#![cfg_attr(feature = "portable", deny(clippy::unwrap_used, clippy::expect_used, clippy::panic))]`
   (or the workspace-lint equivalent) from the first commit.
3. Fault-injection matrix for tier 1; panic baseline row set for tier 2; host-contract
   test for tier 3.
4. The demo's error surface renders three distinct states: refused, unverifiable, crashed.

## Validation

- Tier 1: every injected port failure returns a refusal; zero allows.
- Tier 2: `validate-panic-sites.sh` passes with the crate's classed rows; a seeded `unwrap`
  in `domain/` fails the guard.
- Tier 3: a host-contract test asserts a trap is surfaced as fatal, never as a refusal.

## References

- Exploration NFR-004 (rewritten to this tiering), NFR-001, FR-004
- ADR-005 (no principal; structural authorization), ADR-002 (no persistence)
- `.pi/scripts/validate-panic-sites.sh`, `.pi/panic-baseline.tsv`
- engine `engine/.pi/architecture/decisions/ADR-009-error-handling.md` — same vocabulary, different platform constraint
