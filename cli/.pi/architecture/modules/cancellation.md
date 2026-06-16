# Cancellation

## Module Status

**Status:** Engine contract frozen — CLI uses as library
**Last reviewed:** 2026-06-16
**Source session:** 71e2b81a-a7a1-48ee-ab8f-56284bbec92d

## Description

Two-level shutdown signals via tokio CancellationToken:
- **Graceful** (single Ctrl+C): let running tasks finish, then stop
- **Immediate** (double Ctrl+C): abort all in-flight work

The CLI's SignalHandler captures terminal signals and forwards them to the engine's CancellationService.

## Components

**CLI-facing:**
| Component | File (planned) | Purpose |
|-----------|---------------|---------|
| SignalHandler | `cli/src/signal.rs` | Captures Ctrl+C / SIGINT, differentiates single vs double press |

**Engine dependencies (frozen contracts):**
| Component | Engine Source | Contract |
|-----------|--------------|----------|
| ShutdownSignal | `engine/src/cancellation/domain/signal.rs` | Two-level signal: Graceful, Immediate |
| CancellationService (trait) | `engine/src/cancellation/application/service.rs` | Cancellation propagation service |
| CancellationError | `engine/src/cancellation/domain/error.rs` | Typed error enum |

## Domain Events

| Event | Description | Triggered By |
|-------|-------------|-------------|
| CancellationRequested | A cancellation signal was received | SignalHandler / CancellationService |
| ExecutionCancelled | Execution was terminated by cancellation | Orchestrator |

## Ubiquitous Language

| Term | Definition |
|------|-----------|
| ShutdownSignal | Two-level signal: Graceful (finish in-flight) or Immediate (abort now). |
| GracefulShutdown | Cancellation level: finish in-flight node execution, then stop. Single Ctrl+C. |
| ImmediateShutdown | Cancellation level: abort all in-flight work immediately. Double Ctrl+C. |

## Dependencies

- Depends on: `engine::cancellation` (all contracts frozen)
- Used by: `CLI Boundary` (SignalHandler forwards signals)
- Used by: `Execution Engine` (cooperative cancellation in executor)
- Used by: `Budget Tracking` (coordinated budget exhaustion → cancellation)
