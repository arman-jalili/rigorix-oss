# System Context Diagram

<!--
Canonical Reference: .pi/architecture/diagrams/system-context.md
Related ADRs: ADR-001 (one context, layered seam), ADR-004 (host contract), ADR-009 (conformance oracle)
-->

## Context

`rigorix-gate-wasm` compiles a slice of the Rigorix engine to WebAssembly so a host
application can run the **real** gate locally: evaluate whether an AI agent's proposed
action may proceed, refuse it with evidence, produce signed envelopes, and verify them
offline — byte-identically to the CLI for the same input.

The crate owns **one** bounded context (`Portable Session`) and borrows everything else.

## System Context

```mermaid
graph TB
    subgraph Host["Host application (browser demo / partner product)"]
        UI["UI + mock business system"]
        FIX["Fixture transcript<br/>(provider answers, approvals, sink expectations)"]
    end

    subgraph Wasm["rigorix-gate-wasm  (wasm32-unknown-unknown)"]
        API["interfaces/ — wasm-bindgen contract<br/>open_session · attempt_run · export_artifact_set · verify_artifact_set"]
        APP["application/ — 5 services"]
        DOM["domain/ — ReplaySession aggregate<br/>12 value objects · 5 ports · 8 invariants"]
        INF["infrastructure/ — 5 deterministic adapters<br/>+ verifier bridge"]
    end

    subgraph Engine["rigorix-engine  (shared kernel, additive `portable` feature)"]
        GATE["precondition · sequence_policy · approval<br/>enforcement · audit envelope · identity"]
    end

    CLI["rigorix-cli<br/>(conformance oracle, same commit)"]
    VER["rigorix-verifier<br/>(independent SDK, shares no code)"]
    AUD["Auditor / recipient<br/>(trusts neither producer nor host)"]

    UI --> API
    FIX --> API
    API --> APP --> DOM
    APP --> INF
    DOM -.->|"ports (declared by domain)"| INF
    INF -->|"evaluate gate"| GATE
    GATE -->|"verdict + evidence refs + envelopes"| INF
    API -->|"RunOutcome · ArtifactSet"| UI

    CLI -->|"golden bytes, CI only"| APP
    API -->|"ArtifactSet (bytes)"| VER
    VER -->|"verified | not_verified | unverifiable"| AUD
    UI -->|"downloaded record"| AUD
```

## Trust Boundary

| Direction | Crosses | Why it is safe |
|-----------|---------|----------------|
| Host → crate | fixtures, clock readings, seed, approval **claims**, action parameters | Treated as data the engine evaluates; never authority (FR-009) |
| Crate → host | `RunOutcome`, `ArtifactSet`, `VerificationResult`, telemetry observations | No secrets, no key material, no host paths, no PII (NFR-003) |
| Crate → engine | scenario reference, fixture answers via engine ports | Engine decides; the crate only supplies answers |
| Engine → crate | findings, verdicts, envelopes, evidence refs | Verbatim projection; never re-encoded (ADR-010) |
| Crate → verifier | sealed artifact bytes | The artifact format is the entire contract (ADR-010) |
| CLI → conformance | bytes for the same vector | Build-time only; never shipped (ADR-009) |

## Bounded Context Flow

```mermaid
graph LR
    PS["Portable Session<br/>(core · tactical)"]
    PS -->|"shared-kernel (conformist to engine types)"| ENG["rigorix-engine"]
    PS -->|"conformist (artifact format only)"| VER["rigorix-verifier"]
    PS -->|"conformance-oracle (build time)"| CLI["rigorix-cli"]
    HOST["Host application"] -->|"customer-supplier (versioned contract)"| PS
```

## Deployment View

```mermaid
graph LR
    BROWSER["Browser page<br/>rigorix-gate-wasm + wasm-bindgen glue"]
    NPM["npm package<br/>version == rigorix-engine version"]
    NPM --> BROWSER
    BUNDLE["CI gates:<br/>size budget · no tokio/reqwest/tree-sitter/prometheus/libc<br/>live CLI diff · seeded divergence"]
    BUNDLE --> NPM
```

---

*Generated from session: 3baf6e5c-e352-4127-af09-a5e7fe0a2d29*
*Date: 2026-11-03*