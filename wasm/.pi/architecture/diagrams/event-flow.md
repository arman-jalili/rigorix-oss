# Event Flow

<!--
Canonical Reference: .pi/architecture/diagrams/event-flow.md
Related ADRs: ADR-003 (evidence vs telemetry), ADR-010 (sealed artifact set)
-->

## Context

One bounded context, in-process events only: no broker, no outbox, no async runtime
(`wasm32-unknown-unknown`, single-threaded). The interesting question is not *transport*
but **which events become evidence** and which stay diagnostics.

## Event Classification Flow

```mermaid
flowchart TD
    A["Application service raises event"] --> B{"Produced by the engine?"}
    B -->|"yes — verdict, finding, envelope"| E["Evidence class"]
    B -->|"no — observed by an adapter<br/>(port failure, draw consumed)"| C{"Auditable fact<br/>about the refusal?"}
    C -->|"yes"| E
    C -->|"no — diagnostics only"| T["Telemetry class"]

    E --> S["Collected by ReplaySession<br/>ordered, deterministic"]
    S --> X["ExportArtifactSet<br/>stamps scenario/engine/format versions"]
    X --> ART["ArtifactSet (sealed bytes)"]
    ART --> V["rigorix-verifier<br/>verified | not_verified | unverifiable"]

    T --> D{"release build?"}
    D -->|"yes"| DROP["compiled out"]
    D -->|"no (demo/debug)"| OBS["host observation callback<br/>never signed"]

    X -->|"forbidden content found"| FAIL["export refused<br/>fail closed (NFR-004)"]
    style ART fill:#e8f4ea
    style DROP fill:#f4eae8
    style FAIL fill:#f4eae8
```

## Sequence Across the Boundary

```mermaid
sequenceDiagram
    autonumber
    participant H as Host application
    participant I as interfaces/ (wasm-bindgen)
    participant A as application/
    participant S as ReplaySession (domain)
    participant P as adapters (infra)
    participant E as rigorix-engine

    H->>I: open_session(scenario, fixtures, clock, seed)
    I->>A: OpenReplaySession
    A->>E: parse scenario (engine types)
    A->>S: ReplaySession::open(...)
    S-->>A: ReplaySessionOpened (evidence)
    A-->>I: session_handle

    H->>I: attempt_run(handle, action)
    I->>A: AttemptRun
    A->>S: begin_attempt(action)
    S->>P: Clock.advance / Randomness.draw / Provider.call
    alt port cannot answer
        P-->>S: PortFailure
        S-->>A: RunRefused + PortAnswerUnavailable (evidence)
    else all ports answered
        S->>E: evaluate gate (precondition, sequence, approval claim)
        E-->>S: findings + verdict + evidence refs + envelopes
        S-->>A: RunAllowed | RunRefused + StepDecisionRecorded[]
    end
    A-->>I: RunOutcome
    I-->>H: run_outcome_json

    H->>I: export_artifact_set(handle)
    I->>A: ExportArtifactSet
    A->>S: seal()
    S-->>A: ArtifactSetExported (evidence)
    A->>A: scan for secrets / paths / PII
    A-->>I: ArtifactSet (bytes)
    I-->>H: artifact_set_json

    Note over H: the crate's role ends at the bytes (ADR-013)
    H->>H: rigorix-verifier: check artifact_format_version, then the HMAC
    H->>H: render verified | not_verified | unverifiable(reason)
```

## Event Catalogue by Destination

| Event | Class | Destination | Signed? |
|-------|-------|-------------|---------|
| `ReplaySessionOpened` | Evidence | Artifact set (session stamp) | via envelope |
| `StepDecisionRecorded` | Evidence | Artifact set (ordered step outcomes) | via envelope |
| `RunAllowed` | Evidence | RunOutcome + artifact set | via envelope |
| `RunRefused` | Evidence | RunOutcome + artifact set | via envelope |
| `PortAnswerUnavailable` | Evidence | Artifact set (refusal grounds) | via envelope |
| `ArtifactSetExported` | Evidence | Host observation + export stamp | n/a (export boundary) |
| `RandomDrawConsumed` | Telemetry | Debug callback only | never |

---

*Generated from session: 3baf6e5c-e352-4127-af09-a5e7fe0a2d29*
*Date: 2026-11-03*