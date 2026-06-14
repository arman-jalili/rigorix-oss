//! State Persistence bounded context.
//!
//! @canonical .pi/architecture/modules/state-persistence.md
//! Implements: Contract Freeze — state-persistence module root
//! Issue: issue-contract-freeze
//!
//! Persists execution state to disk using atomic write-rename for crash safety.
//! Tracks overall execution status (Pending, Running, Completed, Failed, Cancelled)
//! and per-node state (Pending, InProgress, Completed, Failed, Skipped). Supports
//! TUI graph persistence for viewing past executions.
//!
//! # Architecture
//!
//! ```text
//! state_persistence/
//! ├── domain/           # Domain entities (ExecutionState, NodeState, ExecutionGraph)
//! │   ├── state.rs      # ExecutionState, NodeState, NodeStatus, ExecutionStatus
//! │   ├── error.rs      # StateError enum
//! │   ├── graph.rs      # ExecutionGraph, GraphManager interface
//! │   ├── context.rs    # ExecutionRecord
//! │   └── event/        # StateEvent payload schemas
//! ├── application/      # Service traits, DTOs, factory interfaces
//! │   ├── service.rs    # StateManagerService trait
//! │   ├── factory.rs    # StateManagerFactory interface
//! │   └── dto/          # Input/Output DTOs with validation
//! ├── infrastructure/   # Repository interfaces
//! │   └── repository/   # StateRepository, GraphRepository traits
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
//! | ExecutionState | Serializable execution snapshot | #state |
//! | NodeState | Per-node state with status, output, retries | #node-state |
//! | StateManager | Atomic persistence with file locking | #manager |
//! | ExecutionGraph | Graph structure for TUI history view | #graph |
//! | GraphManager | Persistence for ExecutionGraph records | #graph-mgr |
//! | ExecutionRecord | Complete execution record (events + context) | #record |

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
