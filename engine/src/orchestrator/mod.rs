//! Orchestrator bounded context.
//!
//! @canonical .pi/architecture/modules/orchestrator.md
//! Implements: Contract Freeze — orchestrator module root
//! Issue: #338
//!
//! Top-level entry point that wires the full Rigorix execution lifecycle into
//! a single operation. Sequences: config loading → planning (via PlanningPipeline)
//! → TaskGraph execution (via ExecutionEngine) → state persistence (via
//! StateManagerService) → event emission (via EventBus) → audit envelope
//! building (via AuditService).
//!
//! # Architecture
//!
//! ```text
//! orchestrator/
//! ├── domain/           # Domain entities (ExecutionRecord), config, errors, events
//! │   ├── config.rs     # OrchestratorConfig
//! │   ├── error.rs      # OrchestratorError enum
//! │   ├── record.rs     # ExecutionRecord aggregate
//! │   └── event/        # OrchestratorEvent payload schemas
//! ├── application/      # Service traits, builder, DTOs
//! │   ├── service.rs    # OrchestratorService trait
//! │   ├── builder.rs    # OrchestratorBuilder trait
//! │   └── dto/          # Input/Output DTOs with validation
//! ├── infrastructure/   # Repository interfaces
//! │   └── repository/   # ExecutionRecordRepository
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

pub use application::*;
pub use domain::*;
