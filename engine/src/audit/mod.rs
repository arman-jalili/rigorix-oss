//! Audit bounded context.
//!
//! @canonical .pi/architecture/modules/audit.md
//! Implements: Contract Freeze — audit module root
//! Issue: #13
//!
//! Records execution audit trails via typed envelopes for governance, replay,
//! and external audit backends. Provides the audit envelope format, sender
//! with retry logic, queue management, and circuit breaker for resilient
//! delivery to remote audit services.
//!
//! # Architecture
//!
//! ```text
//! audit/
//! ├── domain/           # Domain entities (AuditEnvelope), errors, events
//! │   ├── envelope.rs   # AuditEnvelope value object
//! │   ├── error.rs      # AuditError enum
//! │   └── event/        # AuditEvent payload schemas
//! ├── application/      # Service traits, DTOs, factory interfaces
//! │   ├── service.rs    # AuditService, AuditSender, AuditQueue traits
//! │   ├── factory.rs    # AuditEnvelopeFactory, CircuitBreakerFactory traits
//! │   └── dto/          # Input/Output DTOs with validation
//! ├── infrastructure/   # Repository interfaces
//! │   └── repository/   # AuditEnvelopeRepository
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

// #1481 rule 2 (no dropped evidence): `let _ = <must_use>` is how five results
// on the evidence path were discarded. Denied here, where the rule is about
// evidence, rather than workspace-wide where ~50 unrelated best-effort sites
// would each need an exception. A site that genuinely discards must write
// `#[allow(clippy::let_underscore_must_use)]` with a `// sink: <id>` line naming
// the register row whose `does_not` column states the loss.
#![deny(clippy::let_underscore_must_use)]

pub mod application;
pub mod domain;
pub mod infrastructure;

#[cfg(test)]
pub mod failure_tests;
