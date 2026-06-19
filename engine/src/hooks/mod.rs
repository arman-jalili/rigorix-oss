//! Hook System bounded context.
//!
//! @canonical .pi/architecture/modules/hooks.md
//! Implements: Contract Freeze — hooks module root
//! Issue: #410
//!
//! Provides external script-based interception points around every tool
//! execution. Hooks run as shell commands receiving JSON payloads on stdin
//! and returning structured JSON decisions.
//!
//! # Architecture
//!
//! ```text
//! hooks/
//! ├── domain/           # Domain entities (HookEvent, HookRunResult, HookConfig, HookError)
//! │   ├── event.rs      # HookEvent enum (PreToolUse, PostToolUse, PostToolUseFailure)
//! │   ├── protocol.rs   # Hook Protocol — JSON stdin/stdout contracts
//! │   ├── result.rs     # HookRunResult struct
//! │   ├── config.rs     # HookConfig struct
//! │   ├── error.rs      # HookError enum
//! │   ├── abort.rs      # HookAbortSignal for cooperative cancellation
//! │   └── event_payload.rs  # Hook lifecycle event payloads
//! ├── application/      # Service traits, DTOs, factory interfaces
//! │   ├── service.rs    # HookRunnerService trait
//! │   ├── factory.rs    # HookRunnerFactory trait
//! │   └── dto/          # Input/Output DTOs with validation
//! ├── infrastructure/   # Repository interfaces
//! │   └── repository/   # HookCommandRepository trait
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
//! | HookEvent | Lifecycle point identification | #hook-event |
//! | Hook Protocol | JSON stdin/stdout contract | #hook-protocol |
//! | HookRunResult | Aggregated hook execution result | #hook-result |
//! | HookRunner | Hook command execution and aggregation | #hook-runner |
//! | HookConfig | Declarative hook command registration | #hook-config |

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
