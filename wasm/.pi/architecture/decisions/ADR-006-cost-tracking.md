# ADR-006: Cost Tracking & Usage Metering — Out of Scope; Nothing Is Counted on the Gate Path

**Status:** Accepted
**Date:** 2026-11-03
**Session:** 3baf6e5c-e352-4127-af09-a5e7fe0a2d29

## Context

The template asks for a cost-tracking and usage-metering design. For this crate the
answer must be an explicit **no**, for two independent reasons:

1. **Domain.** The exploration says plainly: "no cost sheet, no quote, no ledger
   semantics live here". `rigorix-engine` owns `budget_tracking` and
   `capacity_ledger`; the seam borrows decisions, it does not price them.
2. **Invariant.** Anything counted on the run path is state that can differ between
   the browser and the CLI, which breaks determinism (NFR-001) and with it the
   byte-identical claim (FR-007). A meter is also a write, and writes need a sink the
   wasm target does not have.

A third, subtler reason: a usage meter that a host can read creates an incentive to
"retry until allowed", which is the fail-open behaviour the invariants forbid.

## Decision

**The crate implements no cost tracking, no metering, no quotas and no counters.**

1. Nothing on the `open_session` → `attempt_run` → `export_artifact_set` path mutates
   shared state, increments a counter, or records a duration. The only time that exists
   is `ClockReading`, pinned by fixture.
2. If a host or partner needs product analytics (sessions opened, exports downloaded),
   it counts **outside** the gate: in JS, on its own side of the boundary, from events it
   observes. The crate will not emit analytics payloads.
3. Metering that must be auditable belongs to a server-side context with an identity
   and a ledger — the engine's, not this one. The seam's contribution is the evidence
   reference that lets such a system reconcile a charge to a real decision.
4. The conformance harness may report vector counts and bundle size; those are build
   metrics (ADR-009), never runtime usage data.

## Consequences

### Positive
- Determinism is not compromised by an accumulator that only sometimes runs.
- No PII in usage data because there is no usage data.
- The bundle does not grow a metrics/auth/uuid-of-the-day subsystem (NFR-002).
- "Retry until allowed" has no reward: nothing is optimised for volume.

### Negative
- The demo cannot show "3 of 5 attempts were refused" without doing the counting in JS.
  Accepted and documented in the demo's limits panel.
- A partner wanting per-tenant billing must build it around the artifact set. The seam
  gives them the evidence, not the invoice.

### Neutral
- If metering is ever added, it must be a *host-side observer* of exported events and it
  will need a superseding ADR with its own determinism argument.

## Alternatives Considered

| Alternative | Pros | Cons | Reason Rejected |
|-------------|------|------|-----------------|
| In-crate counters on the session | Simple dashboards | Adds mutable state to the aggregate; two runtimes could differ | Breaks NFR-001 |
| Emit telemetry analytics events | Host gets metrics | Analytics payloads are exactly where PII leaks; also unbounded growth | Contradicts NFR-003 and the evidence/telemetry split (ADR-003) |
| Reuse engine `budget_tracking` | No new code | Budget semantics are business meaning this crate does not own, and the wasm slice must exclude the module | Out of scope by the exploration |
| Quota enforced in the seam ("max N attempts") | Rate limiting | A quota that can deny is a policy the host controls — authority leak (FR-009) | Violates the host-cannot-influence invariant |

## Implementation

**Affected Modules:** `.pi/architecture/modules/portable-session.md`

**Enforcement paths:**
- `wasm/src/domain/session/replay_session.rs` — no counters, no `Instant`, no `SystemTime`
- architecture test: `domain` must not reference any metrics/counter crate
- `wasm/src/interfaces/wasm_api.rs` — no analytics payloads in any DTO

**Canonical References:** `// @canonical .pi/architecture/decisions/ADR-006-cost-tracking.md`

## Validation

- **architecture-validator**: dependency check (no `prometheus`, no `metrics`).
- **test-validator**: determinism test proves two runs of one session produce identical
  bytes, i.e. no hidden accumulator influenced the output.
- **operations-validator**: no wall-clock measurement on the run path.

## References

- Related ADRs: ADR-002, ADR-003, ADR-005
- Engine contexts deliberately not borrowed: `budget_tracking`, `capacity_ledger`

---

*Decision date: 2026-11-03*
*Decision makers: architecture-coordinator, domain exploration session 3baf6e5c*