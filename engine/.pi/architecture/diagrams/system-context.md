# System Context Diagram

<!--
Canonical Reference: .pi/architecture/diagrams/system-context.md
Blueprint Source: Domain Exploration Session 63c25384
-->

## Context

Rigorix is a deterministic coding CLI built in Rust. It operates as a task graph compiler with execution profiles. The system context below shows how the 17 bounded contexts interact.

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
    end

    subgraph "Observability & Persistence"
        ES[Event System]
        SP[State Persistence]
        AUD[Audit]
    end

    subgraph "Cross-Cutting"
        CFG[Configuration]
        EH[Error Handling]
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

    %% Observability
    EE -.->|"publishes events"| ES
    PP -.->|"publishes events"| ES
    TSYS -.->|"publishes events"| ES
    ENF -.->|"BudgetWarning"| ES
    ES -->|"drains into"| SP
    ES -->|"builds"| AUD

    %% Cross-cutting
    CFG -.- PP
    CFG -.- DAG
    CFG -.- EE
    CFG -.- CAN
    CFG -.- ENF
    CFG -.- BT
    CFG -.- TSYS

    EH -.- PP
    EH -.- DAG
    EH -.- EE
    EH -.- TSYS
```

## Execution Lifecycle Flow

```mermaid
sequenceDiagram
    participant User as Developer
    participant PP as Planning Pipeline
    participant DAG as DAG Engine
    participant EE as Execution Engine
    participant EV as Event Bus
    participant SP as State Persistence

    User->>PP: UserIntent
    PP->>PP: Budget check → Classify → Extract
    PP->>PP: Generate TaskGraph → Validate
    PP->>DAG: PlanOutput (graph + metadata)
    PP->>EV: Publish PlanningCompleted
    DAG->>DAG: Topological sort
    SP->>SP: Save ExecutionState (Pending)
    par Execute nodes (topological order)
        EE->>EE: Dequeue ready node
        EE->>EV: Publish NodeStarted
        EE->>EE: Execute tool (with retry loop)
        EE->>EV: Publish NodeCompleted/Failed
    end
    SP->>SP: Save final ExecutionState
    EV->>SP: Drain persisted events → ExecutionRecord
    SP-->>User: ExecutionRecord
```

---

*Generated from session: 63c25384-1902-4b72-83bb-257f3f682af5*
*Date: 2026-06-13*
