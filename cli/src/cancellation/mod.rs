//! Cancellation module — signal handlers for graceful shutdown.
//!
//! @canonical .pi/architecture/modules/cancellation.md
//! Implements: Contract Freeze — CLI Cancellation module (interfaces only)
//! Issue: issue-contract-freeze
//!
//! Captures Ctrl+C (SIGINT) with double-press detection.
//! Single press = graceful shutdown. Double press within 2s = immediate.
//!
//! # Architecture (Clean Architecture layers)
//!
//! ```text
//! cancellation/
//! ├── domain/           # CancellationCliError, CancellationCliEvent
//! │   ├── mod.rs
//! │   ├── error.rs      # CancellationCliError enum
//! │   └── event/        # CancellationCliEvent payload schemas
//! │       └── mod.rs
//! ├── application/      # Service traits, DTO schemas
//! │   ├── mod.rs
//! │   ├── service.rs    # SignalHandler trait + ShutdownLevel enum
//! │   └── dto/          # ShutdownInput/Output, SignalStatus types
//! │       └── mod.rs
//! ├── infrastructure/   # Trait implementations, repository interfaces
//! │   ├── mod.rs
//! │   ├── signal.rs                    # Re-exports SignalHandler + ShutdownLevel
//! │   ├── signal_impl.rs               # SignalHandlerImpl implementation
//! │   └── repository/                  # CancellationCliRepository trait
//! │       └── mod.rs
//! └── interfaces/       # HTTP API contracts
//!     ├── mod.rs
//!     └── http/         # Endpoint definitions, request/response schemas
//!         └── mod.rs
//! ```
//!
//! # Contract Freeze Notice
//!
//! ALL interface files in this module are frozen contracts.
//! - No implementation changes without explicit contract change approval
//! - Implementation PRs MUST reference these interfaces
//! - DTO schemas serve as the canonical data contract
//! - The SignalHandler trait is the primary service contract

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
