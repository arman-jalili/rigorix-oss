//! Event System bounded context.
//!
//! @canonical .pi/architecture/modules/event-system.md
//! Implements: Contract Freeze — event system module root
//! Issue: #46
//!
//! Captures all execution events as an append-only log via tokio broadcast
//! channel with synchronous in-memory persistence. Supports subscriber fan-out
//! for real-time monitoring (ConsoleEventPrinter, TUI) and drain-at-end for
//! ExecutionRecord persistence.
//!
//! # Architecture
//!
//! ```text
//! event_system/
//! ├── domain/           # Domain entities (ExecutionEvent, PersistedEvent), errors
//! │   ├── event.rs      # ExecutionEvent enum (11 variants) + PersistedEvent
//! │   └── error.rs      # EventSystemError enum
//! ├── application/      # Service traits, DTOs, factory interfaces
//! │   ├── service.rs    # EventBusService trait
//! │   ├── factory.rs    # EventBusFactory trait
//! │   └── dto/          # Input/Output DTOs with validation
//! ├── infrastructure/   # Repository interfaces
//! │   └── repository/   # PersistedEventRepository trait
//! └── interfaces/       # API contracts
//!     └── http/         # REST endpoint contracts
//! ```
//!
//! # Contract Freeze Notice
//!
//! ALL files in this module are frozen contracts.
//! - No implementation changes without explicit contract change approval
//! - Implementation PRs MUST reference these interfaces
//! - DTO schemas serve as the canonical data contract
//!
//! # Components
//!
//! | Component | Description | Canonical Section |
//! |-----------|-------------|-------------------|
//! | EventBus | Central pub-sub with in-memory persistence | #bus |
//! | ExecutionEvent | Tagged union of 11 event variants | #events |
//! | PersistedEvent | Event with monotonic sequence number | #persisted |

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
