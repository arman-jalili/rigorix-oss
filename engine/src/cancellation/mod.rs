//! Cancellation bounded context.
//!
//! @canonical .pi/architecture/modules/cancellation.md
//! Implements: Contract Freeze — cancellation module root
//! Issue: issue-contract-freeze
//!
//! Manages graceful and immediate cancellation of running workflows. Uses
//! CancellationToken (tokio-util) for coordinated propagation to all
//! concurrent tasks, with two shutdown signal levels: Graceful (let running
//! tasks finish) and Immediate (abort all in-flight work).
//!
//! # Architecture
//!
//! ```text
//! cancellation/
//! ├── domain/           # Domain entities (ShutdownSignal), errors, events
//! │   ├── error.rs      # CancellationError enum
//! │   ├── signal.rs     # ShutdownSignal value object
//! │   └── event/        # CancellationEvent payload schemas
//! ├── application/      # Service traits, DTOs, factory interfaces
//! │   ├── service.rs    # CancellationService trait
//! │   ├── factory.rs    # CancellationToken factory interfaces
//! │   └── dto/          # Input/Output DTOs with validation
//! ├── infrastructure/   # Repository interfaces
//! │   └── repository/   # (reserved for future persistence)
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

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;

#[cfg(test)]
pub mod tests;
