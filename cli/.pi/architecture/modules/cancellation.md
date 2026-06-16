# Cancellation

## Module Status

**Status:** ✅ Implemented — contract freeze complete, proofing scripts active
**Last reviewed:** 2026-06-16
**Source session:** 71e2b81a-a7a1-48ee-ab8f-56284bbec92d
**Issues:** #274 (contract freeze), #276 (proofing), #277 (architecture readiness)

## Description

Two-level shutdown signals via tokio CancellationToken:
- **Graceful** (single Ctrl+C): let running tasks finish, then stop
- **Immediate** (double Ctrl+C): abort all in-flight work

The CLI's SignalHandler captures terminal signals and forwards them to the engine's CancellationService.

## Architecture

### Clean Architecture Layers

```
cancellation/
├── domain/           # CancellationCliError, CancellationCliEvent
│   ├── mod.rs
│   ├── error.rs      # CancellationCliError enum (typed CLI cancellation errors)
│   └── event/        # CancellationCliEvent payload schemas
│       └── mod.rs
├── application/      # Service traits, DTO schemas
│   ├── mod.rs
│   ├── service.rs    # SignalHandler trait + ShutdownLevel enum (frozen)
│   └── dto/          # GracefulShutdownInput/Output, SignalStatus types
│       └── mod.rs
├── infrastructure/   # Trait implementations, repository interfaces
│   ├── mod.rs
│   ├── signal.rs                    # Re-exports SignalHandler + ShutdownLevel
│   ├── signal_impl.rs               # SignalHandlerImpl (double-press detection)
│   └── repository/                  # CancellationCliRepository trait
│       └── mod.rs
└── interfaces/       # HTTP API contracts
    ├── mod.rs
    └── http/         # Endpoint definitions, request/response schemas
        └── mod.rs
```

### Data Flow

```
User Ctrl+C → OS Signal → SignalHandlerImpl (infrastructure)
                              ↓
                    ShutdownLevel::Graceful|Immediate
                              ↓
                    watch::Receiver monitored by orchestrator
                              ↓
                    Engine::CancellationService
```

### CI Proofing (Stage 15)

The following scripts run automatically in the hardening pipeline:

| Script | Checks | Exit |
|--------|--------|------|
| `check_cancellation_contracts.sh` | 13 checks — all interfaces have implementations | 0/1 |
| `check_cancellation_coverage.sh` | 8 checks — coverage across all 4 layers | 0/1 |
| `stage_cancellation_proofing.sh` | Wrapper: runs both + CI validation | 0/1 |

## Components

**CLI-facing (contract freeze):**
| Component | File | Module | Purpose |
|-----------|------|--------|---------|
| SignalHandler (trait) | `cli/src/cancellation/application/service.rs` | application | SignalHandler trait + ShutdownLevel (frozen) |
| SignalHandlerImpl | `cli/src/cancellation/infrastructure/signal_impl.rs` | infrastructure | Double-press Ctrl+C detection (2s window) |
| CancellationCliRepository (trait) | `cli/src/cancellation/infrastructure/repository/mod.rs` | infrastructure | Repository interface for signal state (frozen) |
| CancellationCliError | `cli/src/cancellation/domain/error.rs` | domain | Typed CLI cancellation error enum (frozen) |
| CancellationCliEvent | `cli/src/cancellation/domain/event/mod.rs` | domain | CLI cancellation event schemas (frozen) |
| GracefulShutdownInput/Output | `cli/src/cancellation/application/dto/mod.rs` | application | Shutdown DTOs (frozen) |
| SignalStatusInput/Output | `cli/src/cancellation/application/dto/mod.rs` | application | Status query DTOs (frozen) |
| CancellationStatusResponse | `cli/src/cancellation/interfaces/http/mod.rs` | interfaces | HTTP status response (frozen) |

**Engine dependencies (frozen contracts):**
| Component | Engine Source | Contract |
|-----------|--------------|----------|
| ShutdownSignal | `engine/src/cancellation/domain/signal.rs` | Two-level signal: Graceful, Immediate |
| CancellationService (trait) | `engine/src/cancellation/application/service.rs` | Cancellation propagation service |
| CancellationError | `engine/src/cancellation/domain/error.rs` | Typed error enum |

## Domain Events

| Event | Description | Triggered By |
|-------|-------------|-------------|
| CancellationCliEvent::GracefulShutdownRequested | Single Ctrl+C received | SignalHandlerImpl |
| CancellationCliEvent::ImmediateShutdownRequested | Second Ctrl+C within window | SignalHandlerImpl |
| CancellationCliEvent::GracePeriodExpired | Window expired without 2nd signal | SignalHandlerImpl |
| CancellationCliEvent::SignalHandlerInstalled | Signal handler installed | SignalHandlerImpl::install() |
| CancellationCliEvent::ShutdownSignalForwarded | Signal forwarded to engine | Orchestrator |

## Ubiquitous Language

| Term | Definition |
|------|-----------|
| ShutdownLevel | Two-level signal: Graceful (finish in-flight) or Immediate (abort now). |
| SignalHandler | Trait for capturing OS signals and converting to ShutdownLevel. |
| SignalHandlerImpl | Concrete implementation with double-press Ctrl+C detection. |
| GracefulShutdown | Cancellation level: finish in-flight node execution, then stop. Single Ctrl+C. |
| ImmediateShutdown | Cancellation level: abort all in-flight work immediately. Double Ctrl+C. |
| Double-press window | Time window (default 2s) during which a second Ctrl+C escalates to immediate. |

## Dependencies

- Depends on: `engine::cancellation` (all contracts frozen)
- Used by: `CLI Boundary` (SignalHandler forwards signals)
- Used by: `Execution Engine` (cooperative cancellation in executor)
- Used by: `Budget Tracking` (coordinated budget exhaustion → cancellation)

## Key Files

| File | Purpose |
|------|---------|
| `cli/src/cancellation/application/service.rs` | SignalHandler trait — canonical contract |
| `cli/src/cancellation/application/dto/mod.rs` | DTO schemas for shutdown operations |
| `cli/src/cancellation/domain/error.rs` | CancellationCliError — typed error enum |
| `cli/src/cancellation/domain/event/mod.rs` | CancellationCliEvent — event payload schemas |
| `cli/src/cancellation/infrastructure/repository/mod.rs` | CancellationCliRepository — repository interface |
| `cli/src/cancellation/infrastructure/signal_impl.rs` | SignalHandlerImpl — double-press detection |
| `cli/src/cancellation/interfaces/http/mod.rs` | HTTP API endpoint contracts |
| `cli/docs/runbook-cancellation.md` | Operations runbook |
| `cli/docs/dr-plan-cancellation.md` | Disaster recovery plan |
| `.pi/scripts/ci/check_cancellation_contracts.sh` | Contract implementation proofing script |
| `.pi/scripts/ci/check_cancellation_coverage.sh` | Coverage threshold proofing script |
| `.pi/scripts/ci/stage_cancellation_proofing.sh` | CI stage wrapper |

## ADRs

| ADR | Title | Status |
|-----|-------|--------|
| ADR-001 | Domain-Driven Design with Bounded Contexts | Proposed |
| ADR-007 | Ephemeral CLI — No Daemon for v1 | Proposed |

## Related Issues

| Issue | Description | Status |
|-------|-------------|--------|
| #274 | Contract freeze — define interfaces and contracts | ✅ Merged (PR #278) |
| #276 | Proofing — validation scripts + CI integration | ✅ Merged (PR #279) |
| #277 | Architecture readiness — runbook, DR, docs, CI enforcement | ✅ In progress |
