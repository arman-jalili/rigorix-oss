# System Context Diagram

<!--
Canonical Reference: .pi/architecture/diagrams/system-context.md
Blueprint Source: Domain Exploration Session 63c25384
-->

## Context

Rigorix is a deterministic coding CLI built in Rust. It operates as a task graph compiler with execution profiles. The system context below shows how the 22 bounded contexts interact (17 original + Quality Gates + Scored Evaluation + Approval Binding + Identity + Sequence Policy (proposed 2026-09-04)).

## Bounded Contexts Interaction Flow

```mermaid
graph TB
    subgraph "User Facing"
        UI[("CLI / TUI")]
    end

    subgraph "Planning Phase"
        PP[Planning Pipeline]
        TS[Template System]
        TG[Template Generation]
        RE[Repo Engine]
        BT[Budget Tracking]
    end

    subgraph "Execution Phase"
        DAG[DAG Engine]
        EE[Execution Engine]
        RG[Risk Gating]
        TSYS[Tool System]
        FC[Failure Classification]
        CAN[Cancellation]
        ENF[Enforcement]
        QG[Quality Gates]
        SE[Scored Evaluation]
        AP[Approval Binding]
        SP[Sequence Policy]
    end

    subgraph "Observability & Persistence"
        ES[Event System]
        SP[State Persistence]
        AUD[Audit]
    end

    subgraph "Policy & Enforcement"
        POL[Policy Engine]
    end

    subgraph "Cross-Cutting"
        CFG[Configuration]
        EH[Error Handling]
        ID[Identity]
    end

    %% User to Planning
    UI -->|"UserIntent"| PP

    %% Planning Phase internals
    PP -->|"classify against"| TS
    PP -->|"low-confidence fallback"| TG
    PP -->|"enriched context"| RE
    PP -.->|"budget check"| BT
    TG -->|"validates against"| RE
    TG -->|"registers into"| TS

    %% Planning to Execution
    PP -->|"PlanOutput { PlanningResult, TaskGraph }"| DAG

    %% Execution Phase internals
    DAG -->|"TaskGraph"| EE
    EE -->|"gates tools"| RG
    RG -->|"allowed"| TSYS
    EE -->|"classifies failures"| FC
    EE -.->|"checks limits"| ENF
    EE -.->|"checks cancellation"| CAN
    EE -->|"evaluates test scope"| QG
    EE -->|"invokes scoring"| SE
    EE -->|"verifies intent pre-dispatch"| AP
    AP -->|"re-approval / IntentMismatch halt"| EE

    %% Sequence Policy — composed-action gating
    EE -->|"ordered plan / dispatch prefix"| SP
    SP -->|"promote → requires_approval"| AP
    SP -->|"deny / findings"| EE
    SP -.->|"SequenceRuleMatched / SequencePolicyDenied"| ES
    SP -.->|"sequence_policy_findings"| AUD

    %% Policy Engine receives from both quality dimensions
    QG -->|"scope quality"| POL
    SE -->|"output quality scores"| POL
    POL -->|"gating actions"| EE

    %% Approval Binding & Identity
    AP -->|"approver identity"| ID
    AP -->|"signed approval records"| AUD
    AP -.->|"ApprovalRecorded / IntentMismatch / ScopeViolation"| ES
    ID -.->|"identity claims (author / approver)"| EE
    ID -.->|"identity block"| AUD

    %% Observability
    EE -.->|"publishes events"| ES
    PP -.->|"publishes events"| ES
    TSYS -.->|"publishes events"| ES
    ENF -.->|"BudgetWarning"| ES
    QG -.->|"publishes events"| ES
    SE -.->|"publishes events"| ES
    ES -->|"drains into"| SP
    ES -->|"builds"| AUD

    %% Audit also receives from quality modules
    SE -.->|"envelope extension"| AUD
    QG -.->|"outcome evidence"| AUD

    %% Cross-cutting
    CFG -.- PP
    CFG -.- DAG
    CFG -.- EE
    CFG -.- CAN
    CFG -.- ENF
    CFG -.- BT
    CFG -.- TSYS
    CFG -.- QG
    CFG -.- SE
    CFG -.- POL
    CFG -.- AP
    CFG -.- SP
    CFG -.- ID

    EH -.- PP
    EH -.- DAG
    EH -.- EE
    EH -.- TSYS
    EH -.- QG
    EH -.- SE
    EH -.- AP

    %% External scoring backends
    subgraph "External Systems (Protocol Adopters)"
        MCP[MCP Server\\ne.g. RuntimeAI]
        HTTP[REST Server\\nCustom Service]
        LOC[Local Script]
    end

    SE -->|"rigorix_evaluate_artifact (MCP)"| MCP
    SE -->|"Rigorix Scoring Protocol (HTTP)"| HTTP
    SE -->|"Rigorix Scoring Protocol (stdin/stdout)"| LOC

    %% Visual styling
    style QG fill:#6bb86b,stroke:#3d7a3d,color:#fff
    style SE fill:#4a90d9,stroke:#2c5f8a,color:#fff
    style POL fill:#d9a74a,stroke:#8a6b2c,color:#fff
    style AP fill:#e76f51,stroke:#b5452a,color:#fff
    style SP fill:#9b59b6,stroke:#6c3483,color:#fff
    style ID fill:#b56576,stroke:#803f4e,color:#fff
```

## Execution Lifecycle Flow

```mermaid
sequenceDiagram
    participant User as Developer
    participant PP as Planning Pipeline
    participant DAG as DAG Engine
    participant EE as Execution Engine
    participant QG as Quality Gates
    participant SE as Scored Evaluation
    participant POL as Policy Engine
    participant EV as Event Bus
    participant SP as State Persistence
    participant SEQ as Sequence Policy

    User->>PP: UserIntent
    PP->>PP: Budget check → Classify → Extract
    PP->>PP: Generate TaskGraph → Validate
    PP->>DAG: PlanOutput (graph + metadata)
    PP->>EV: Publish PlanningCompleted
    DAG->>DAG: Topological sort
    SP->>SP: Save ExecutionState (Pending)
    SEQ->>SEQ: Evaluate ordered plan (sequence rules)
    alt Sequence match (A → B)
        SEQ-->>EE: Promote B (requires_approval)
        Note over EE: run pauses — approval chain decides
    else No match
        SEQ-->>EE: Proceed
    end
    SEQ->>EV: Publish SequenceRuleMatched (on match)

    par Execute nodes (topological order)
        EE->>EE: Dequeue ready node
        EE->>EV: Publish NodeStarted
        EE->>SEQ: Evaluate dispatch prefix (dynamic plans)
        EE->>EE: Execute tool (with retry loop)
        EE->>EV: Publish NodeCompleted/Failed
    end

    EE->>QG: Evaluate GreenContract (test scope)
    alt scored_evaluation node present
        EE->>SE: Invoke scored evaluation
        SE->>EV: Publish ScoredEvaluationStarted
        SE->>SE: Backend evaluate(artifact, rubric)
        alt Success
            SE->>EV: Publish ScoredEvaluationCompleted
            SE->>SE: Persist result
        else Failure
            SE->>EV: Publish ScoredEvaluationFailed
            SE->>SE: Apply retry/fallback policy
        end
    end

    EE->>POL: Evaluate policy rules
    POL->>POL: Check ScoreAbove/ScoreBelow, GreenAt
    POL->>EE: Actions: block_merge / flag_for_review

    SP->>SP: Save final ExecutionState
    EV->>SP: Drain persisted events → ExecutionRecord
    SP-->>User: ExecutionRecord
```

---

*Last updated: 2026-09-04*
*Generated from session: 63c25384-1902-4b72-83bb-257f3f682af5*
*Amendment: Sequence Policy bounded context added (proposed, ADR-013 — docs only)*
