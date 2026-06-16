# Cancellation — Two-Level Shutdown

```mermaid
graph TB
    subgraph "Signal Detection"
        SIGINT1[Ctrl+C pressed once]
        SIGINT2[Ctrl+C pressed twice within 2 seconds]
    end

    subgraph "Signal Processing"
        GRACEFUL[Emit Graceful ShutdownSignal]
        IMMEDIATE[Emit Immediate ShutdownSignal]
    end

    subgraph "Graceful Path"
        FINISH[Let in-flight nodes finish]
        STOP[Stop dequeuing new nodes]
        DRAIN[Wait for running tasks]
        COMPLETE[Emit ExecutionCancelled graceful]
    end

    subgraph "Immediate Path"
        ABORT[Abort all JoinSet tasks]
        CANCEL_TOK[Cancel CancellationToken]
        FAIL_ALL[Mark running nodes as Failed]
        COMPLETE2[Emit ExecutionCancelled immediate]
    end

    SIGINT1 --> GRACEFUL
    SIGINT2 --> IMMEDIATE
    GRACEFUL --> FINISH --> STOP --> DRAIN --> COMPLETE
    IMMEDIATE --> ABORT --> CANCEL_TOK --> FAIL_ALL --> COMPLETE2
```

## Implementation

```rust
// Signal handler detects single vs double Ctrl+C
pub enum ShutdownSignal {
    Graceful,   // Finish in-flight, then stop
    Immediate,  // Abort all immediately
}
```

*Part of: Cancellation module*
