# System Architecture Overview

<!--
Canonical Reference: .pi/architecture/diagrams/system-overview.md
Superseded scaffold: the multi-module (api-gateway / data-layer / cache) template does not apply.
See ADR-001 (one core context, four layers) and ADR-002 (no persistence).
-->

## High-Level Architecture

```text
┌──────────────────────────────────────────────────────────────────────┐
│                     Host application (browser / partner)             │
│        UI · mock business system · fixture transcript authoring      │
└──────────────────────────────────────────────────────────────────────┘
                                │  JSON DTOs over wasm-bindgen (ADR-004)
                                ▼
┌──────────────────────────────────────────────────────────────────────┐
│                     rigorix-gate-wasm  (wasm32-unknown-unknown)           │
│  interfaces/   WasmContract · DTOs · RefusalDto · TelemetryObserver  │
│  application/  OpenReplaySession · AttemptRun · ExportArtifactSet    │
│                RunConformanceVector (CI only)                        │
│  domain/       ReplaySession (aggregate) · 12 VOs · 6 ports          │
│                RefusalVocabulary · EvidenceProjection · 7 events     │
│  infrastructure/ 6 deterministic adapters · ContentScanner           │
│                  CanonicalSerializer · no verifier (ADR-013)          │
└──────────────────────────────────────────────────────────────────────┘
        │ engine types + gate evaluation          │ artifact bytes
        ▼                                         ▼
┌───────────────────────────────┐   ┌──────────────────────────────────┐
│ rigorix-engine (shared kernel)│   │ rigorix-verifier (independent)   │
│ precondition · sequence_policy│   │ three-valued: verified ·         │
│ approval · enforcement ·      │   │ not_verified · unverifiable      │
│ audit envelope · identity     │   └──────────────────────────────────┘
└───────────────────────────────┘                 ▲
        ▲                                         │
        │ same commit, build time                 │
┌───────────────────────────────┐   ┌──────────────────────────────────┐
│ rigorix-cli (oracle)          │   │ Auditor / recipient              │
│ pinned clock + ids (FR-011)   │   │ trusts neither producer nor host │
└───────────────────────────────┘   └──────────────────────────────────┘
```

## Layer Map

| Layer | Module content | Purpose | Entry point |
|-------|----------------|---------|-------------|
| Presentation / Interfaces | `WasmContract`, DTOs, error projection | The only published surface: 5 functions | `wasm/src/interfaces/wasm_api.rs` |
| Application | 5 catalog services + read helper | Orchestration, no business rule | `wasm/src/application/` |
| Domain | Aggregate, value objects, ports, services, events | Decisions' shape: invariants, evidence, determinism | `wasm/src/domain/` |
| Infrastructure | Adapters, verifier bridge, scanner, store | Boundaries: fixture answers, delegation, redaction | `wasm/src/infrastructure/` |

There is no data layer: no database, no cache, no object storage, no migrations (ADR-002).
The artifact set is the only durable state, and it is the host's to keep.

## Module Dependency Graph

```text
interfaces ──► application ──► domain ◄── infrastructure
     │              │             │              │
     │              └── ports ────┘              │
     └──────────────── canonical serializer ─────┘

domain  ──► rigorix-engine types (shared kernel, read-only)
infrastructure ──► rigorix-engine (portable feature) · rigorix-verifier
tests/conformance ──► rigorix-cli (build time only, never shipped)
```

Rules enforced by the `domain_isolation` architecture test:

- `domain` names no `std::time`, `std::env`, `Uuid::new_v4`, `reqwest`, `rand::thread_rng`.
- `domain` has no dependency on `infrastructure` or `interfaces`.
- `infrastructure` implements traits declared in `domain`; it never declares its own
  domain vocabulary.

## Crate / Artifact View

| Artifact | Version rule | Ships to |
|----------|--------------|----------|
| `rigorix-gate-wasm` crate | equals `rigorix-engine` | crates.io (internal-ish) |
| npm package | **exactly** `rigorix-engine` version | browser demo, partner product |
| `artifact_format_version` | integer, own cadence | stamped inside every export |
| `contract_version` | semver of the wasm surface | returned by `contract_version()` |

## CI Gates

```text
build ──► domain_isolation ──► fault matrix (6 ports × 4 failures)
      ──► determinism (two runs → identical bytes)
      ──► goldens ──► live CLI diff (same commit)  ──► seeded divergence MUST fail
      ──► verifier independence (wasm artifact → rigorix-verifier)
      ──► PII/secret/path scan ──► bundle size budget (+25%) ──► npm version parity
```

---

*Canonical: .pi/architecture/diagrams/system-overview.md*
*Updated: 2026-11-03 (session 3baf6e5c-e352-4127-af09-a5e7fe0a2d29)*