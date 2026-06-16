# Audit — Envelope Lifecycle

```mermaid
graph TB
    subgraph "Collection"
        EVENTS[PersistedEvents from EventBus]
        HASH[PlanningHash from PlanningResult]
        META[Metadata: execution_id, template_id, timestamp]
    end

    subgraph "Envelope Creation"
        BUILD[Build AuditEnvelope]
        SIGN[Add integrity hash]
        ENQUEUE[Enqueue for delivery]
    end

    subgraph "Delivery"
        SEND[Send to audit backend]
        RETRY{Retry needed?}
        CB{Circuit breaker open?}
        FAIL[Log failure, retry later]
        DONE[Delivered OK]
    end

    EVENTS --> BUILD
    HASH --> BUILD
    META --> BUILD
    BUILD --> SIGN --> ENQUEUE

    ENQUEUE --> SEND
    SEND --> |Success| DONE
    SEND --> |Transient error| RETRY
    RETRY --> |Retries not maxed| SEND
    RETRY --> |Exhausted| CB
    CB --> |Open| FAIL
    CB --> |Half-open| SEND
    CB --> |Closed| SEND
    FAIL --> |Periodic retry| SEND
```

## CLI Audit Commands

```
rigorix audit list              # List audit envelopes (execution_id, date, status)
rigorix audit show <id>         # Show full envelope details  
rigorix audit diff <id1> <id2>  # Diff two execution plans (planning_hash comparison)
```

*Part of: Audit module*
