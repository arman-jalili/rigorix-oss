# ADR-003: Cross-Context Communication — In-Process Domain Events, Evidence vs Telemetry

**Status:** Accepted
**Date:** 2026-11-03
**Session:** 3baf6e5c-e352-4127-af09-a5e7fe0a2d29

## Context

There is one bounded context in this crate, so there is no RPC-shaped messaging: no
broker, no outbox, no consumer group — none is possible on `wasm32-unknown-unknown`
single-threaded. Yet the exploration produced seven domain events, and they are not
decorative: six are the **evidence** that makes a refusal auditable, one is
**telemetry** that explains a divergence to a fixture author.

Conflating the two is the failure mode the review named. A host that can style an
"unverifiable" record as "not_verified" commits the same error as a system that lets a
log line stand in as evidence.

## Decision

Domain events are **in-process values raised by the aggregate**, split into two
classes with different lifetimes and different trust:

| Class | Events | Fate | May enter an artifact? |
|-------|--------|------|------------------------|
| **Evidence** | `ReplaySessionOpened`, `RunAllowed`, `RunRefused`, `StepDecisionRecorded`, `PortAnswerUnavailable`, `ArtifactSetExported` | Carried into the sealed envelopes / evidence references by `ExportArtifactSet` | Yes — that is their purpose |
| **Telemetry** | `RandomDrawConsumed` | Debug/replay diagnostics only; never published, never signed | No |

Rules:

1. Events are collected by the aggregate and returned with the result — no global bus,
   no registry, no `static` side channel. Delivery to a subscriber is the host's
   choice through the interface layer's observation callback.
2. An event is evidence only if the **engine** produced the fact it reports. The crate
   may raise `PortAnswerUnavailable` (it observed the failure) but may never mint a
   `RunAllowed`/`RunRefused`: those are projections of the engine's verdict, and the
   projection is checkable because the evidence reference must match.
3. Telemetry carries no personal data, no host paths, no key material, and is dropped
   by default in the release build (compile-time feature), so it cannot leak into a
   bundle a partner ships.
4. Ordering is deterministic: events are emitted in the order the run produced them,
   with the engine's tie-breaking, so an export is byte-stable.
5. Payloads are versioned by the artifact-format version, not by the event name.

## Consequences

### Positive
- The evidence/telemetry split is explicit and testable: a test asserts no telemetry
  event can reach `ArtifactSet`.
- No async runtime, no broker, no serialization framework beyond serde — the bundle
  stays inside budget (NFR-002).
- The host gets events for UI without being able to forge one: a forged event is not in
  the artifact, and the artifact is what the verifier reads.

### Negative
- No cross-process event replay; the artifact set is the only replay medium. Accepted —
  it is the stronger story for audit.
- Compiling telemetry out means a partner build cannot diagnose fixture problems in
  production; the demo build keeps it on and the feature is named explicitly.

### Neutral
- Event names are Rust enum variants; the JSON projection is snake_case and reaches the
  host contract only as *observations*, never as instructions.

## Alternatives Considered

| Alternative | Pros | Cons | Reason Rejected |
|-------------|------|------|-----------------|
| `tokio` broadcast channel | Familiar | Pulls a runtime into wasm; violates NFR-002 | No concurrency exists |
| One unified event stream (evidence + telemetry) | Simpler model | Telemetry can be mistaken for evidence, and signed | The review's central distinction |
| No domain events; return a struct | Smallest | Loses the audit story and the fixture author's signal | Exploration captured them; they carry meaning |
| Persist events in an event store | Replay | No store (ADR-002); the sealed artifact already replays | Redundant |

## Implementation

**Affected Modules:** `.pi/architecture/modules/portable-session.md`

**Crate paths:**
- `wasm/src/domain/events/mod.rs` — enum + evidence/telemetry classification
- `wasm/src/domain/session/replay_session.rs` — `take_events`
- `wasm/src/application/export_artifact_set.rs` — consumes evidence only
- `wasm/src/interfaces/events.rs` — host observation callback (no return path)

**Canonical References:** `// @canonical .pi/architecture/decisions/ADR-003-cross-context-events.md`

## Validation

- **architecture-validator**: `domain/events` carries no I/O; no `lazy_static`/`OnceLock` bus.
- **test-validator**: `telemetry_never_reaches_artifact_set` exists and passes.
- **security-validator**: telemetry payload scan (NFR-003).

## References

- Related ADRs: ADR-002, ADR-004, ADR-006
- Exploration domain events + triage: `.pi/domain/exploration/3baf6e5c-e352-4127-af09-a5e7fe0a2d29.raw.json`

---

*Decision date: 2026-11-03*
*Decision makers: architecture-coordinator, domain exploration session 3baf6e5c*