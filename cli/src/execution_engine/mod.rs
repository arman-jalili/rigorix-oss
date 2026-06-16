//! Execution Engine module — CLI wrapper for parallel DAG execution.
//!
//! @canonical .pi/architecture/modules/execution-engine.md
//! Implements: Contract Freeze — CLI Execution Engine module (interfaces only)
//! Issue: issue-contract-freeze
//!
//! Wraps the engine's ParallelExecutionService for CLI consumption.
//!
//! # Architecture
//!
//! ```text
//! execution_engine/
//! ├── domain/           # ExecutionCliError, ExecutionCliEvent
//! │   ├── mod.rs
//! │   ├── error.rs      # ExecutionCliError enum
//! │   └── event/        # ExecutionCliEvent payload schemas
//! │       └── mod.rs
//! ├── application/      # Service traits, DTO schemas
//! │   ├── mod.rs
//! │   ├── service.rs    # ExecutionCommandService trait
//! │   └── dto/          # ExecuteInput/Output, Status types
//! │       └── mod.rs
//! ├── infrastructure/   # Repository interfaces
//! │   ├── mod.rs
//! │   └── repository/   # ExecutionRepository trait
//! │       └── mod.rs
//! └── interfaces/       # HTTP API contracts
//!     ├── mod.rs
//!     └── http/         # Endpoint definitions
//!         └── mod.rs
//! ```

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
