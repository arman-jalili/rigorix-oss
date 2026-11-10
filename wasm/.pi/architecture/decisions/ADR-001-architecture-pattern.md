# ADR-001: Layered Single-Crate Seam (DDD inside one bounded context)

**Status:** Accepted
**Date:** 2026-11-03
**Session:** 3baf6e5c-e352-4127-af09-a5e7fe0a2d29
**Supersedes:** the scaffolded "Modular Monolith with N bounded contexts" wording (only one context exists)

## Context

The domain exploration found exactly **one** bounded context owned by this crate:
`Portable Session` (core subdomain, modeling depth *tactical*). Everything else the
crate touches is borrowed authority:

- `rigorix-engine` — shared kernel: the gate, policy evaluation, approval binding,
  envelope construction and signing, identity attestation.
- `rigorix-verifier` — conformist, outbound only: three-valued verification.
- `rigorix-cli` — conformance oracle, built from the same commit.
- the host application — customer/supplier: supplies fixtures, consumes outcomes.

The crate is compiled to `wasm32-unknown-unknown`: no filesystem, no network, no
threads, single-threaded, size-budgeted in CI. The original ADR-001 stub proposed a
modular monolith with several independently deployable modules and per-module
persistence. That is not this system: there is nothing to deploy, and the only
durable state is a byte string the host chooses to keep.

## Decision

We keep **Domain-Driven Design** but scope it honestly: one bounded context, one
crate (`rigorix-gate-wasm`), four layers inside it, enforced by module visibility and a
dependency-direction test rather than by process boundaries.

| Layer | Crate path | Allowed to depend on |
|-------|-----------|----------------------|
| `domain` | `src/domain/` | core + `rigorix-engine` domain types only |
| `application` | `src/application/` | `domain` |
| `infrastructure` | `src/infrastructure/` | `domain`, `application`, engine adapters, verifier |
| `interfaces` | `src/interfaces/` | `application`, `infrastructure` (thin) |

Invariants of the decision:

1. `ReplaySession` is the single aggregate root and the only consistency boundary.
2. No cross-context persistence, no cross-context event bus, no service discovery:
   the only contracts are the engine's Rust API, the artifact format, and the
   wasm-bindgen surface.
3. `rigorix-engine` changes are **additive and behind a non-default `portable`
   feature**; the four published crates (engine, mcp, cli, actions) keep identical
   default-feature behaviour, verified from the same commit (NFR-005).
4. The crate publishes the npm package `rigorix-gate` at exactly the engine's version;
   crate name, version, licence and release-train registration are decided in
   [ADR-011](ADR-011-crate-identity-and-release-train.md).

## Consequences

### Positive
- Matches the real topology — one context, one artifact, one release train.
- The layer rule is testable without infrastructure: an architecture test forbids
  `domain` from naming `std::time`, `std::env`, `uuid::Uuid::new_v4`, or any
  `infrastructure` path.
- Determinism ports live in `domain`, their fixture-backed implementations in
  `infrastructure`, so "no ambient capability on the run path" is a compile-time
  property, not a review rule.

### Negative
- The crate cannot be split per context later without moving code; accepted, because
  a second context would arrive with its own crate and npm package.
- `rigorix-engine` gains a feature-flagged seam, which is real work in a crate we do
  not own; the feature must be additive and tested for default-build parity.

### Neutral
- The scaffolded multi-module wording in `.pi/architecture/diagrams/system-overview.md`
  is corrected to the one-context topology by this ADR.

## Alternatives Considered

| Alternative | Pros | Cons | Reason Rejected |
|-------------|------|------|-----------------|
| Multiple bounded contexts in one crate (as scaffolded) | Familiar shape; room to grow | Invents contexts the exploration did not find; every extra boundary needs a contract nobody asked for | Only `Portable Session` is owned; the rest is borrowed authority |
| Reimplement the gate in TypeScript/JS | Small bundle, no engine work | Duplicates the source of truth; guarantees divergence from the CLI | Explicit non-goal; destroys the byte-identical claim |
| Thin FFI over the native engine binary | No reimplementation | No filesystem/process in the browser; impossible on `wasm32-unknown-unknown` | Environment forbids it |
| Split into `rigorix-gate-wasm-domain` + `rigorix-gate-wasm-host` crates | Harder boundary | Two npm packages for one context; version skew risk | Release train wants one package at engine version |

## Implementation

**Affected Modules:**
- `.pi/architecture/modules/portable-session.md`

**Crate paths:**
- `wasm/src/domain/` — aggregate, value objects, ports, domain events, errors
- `wasm/src/application/` — the five catalog services
- `wasm/src/infrastructure/` — deterministic adapters, verifier bridge, sealing
- `wasm/src/interfaces/` — wasm-bindgen surface + JSON DTOs

**Canonical References:** implementation files carry
`// @canonical .pi/architecture/decisions/ADR-001-architecture-pattern.md`

## Validation

- **architecture-validator**: layer-direction test (`domain` must not reference
  `infrastructure`/`interfaces`; no ambient-capability symbols in `domain`).
- **ci-mr-validator**: `cargo tree -e normal,no-dev --target wasm32-unknown-unknown`
  diffed before/after; bundle size gate.
- **operations-validator**: fail-closed behaviour at the interface boundary.

## References

- Related ADRs: ADR-002 (storage), ADR-003 (events), ADR-004 (host contract),
  ADR-005 (authority), ADR-008 (modeling depth)
- Domain exploration: `.pi/domain/exploration/3baf6e5c-e352-4127-af09-a5e7fe0a2d29.md`
- Engine spec: `docs/spec/execution-governance/03-requirements.md` §L, `docs/spec/terminology.md`

---

*Decision date: 2026-11-03*
*Decision makers: architecture-coordinator, domain exploration session 3baf6e5c*