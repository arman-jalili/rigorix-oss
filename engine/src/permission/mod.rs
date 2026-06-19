//! Permission Enforcer bounded context.
//!
//! @canonical .pi/architecture/modules/permission-enforcer.md
//! Implements: Contract Freeze — permission-enforcer module root
//! Issue: issue-contract-freeze
//!
//! Provides a three-tier permission mode hierarchy (`ReadOnly` →
//! `WorkspaceWrite` → `DangerousFullAccess`) that gates every tool
//! invocation. The active permission mode caps the maximum risk level
//! a tool can execute. Tools requesting a higher `required_mode` than
//! the active mode are denied with structured reasoning.
//!
//! # Architecture
//!
//! ```text
//! permission/
//! ├── domain/             # Domain entities: PermissionMode, PermissionPolicy,
//! │   │                     PermissionOutcome, BashClassifier, PermissionConfig,
//! │   │                     PermissionError, PermissionEvent
//! │   ├── mode.rs         # PermissionMode enum (ReadOnly, WorkspaceWrite, DangerousFullAccess)
//! │   ├── policy.rs       # PermissionPolicy with authorize() logic
//! │   ├── outcome.rs      # PermissionOutcome = Allowed | Denied { reason }
//! │   ├── context.rs      # PermissionContext for temporal overrides
//! │   ├── bash_classifier.rs  # BashClassifier + CommandIntent
//! │   ├── config.rs       # PermissionConfig for allow/deny/ask rules
//! │   ├── prompter.rs     # PermissionPrompter trait for interactive confirmation
//! │   ├── error.rs        # PermissionError enum
//! │   └── event/          # PermissionEvent payload schemas
//! ├── application/        # Service traits, DTOs, factory interfaces
//! │   ├── enforcer.rs     # PermissionEnforcer trait
//! │   ├── factory.rs      # PermissionEnforcerFactory interface
//! │   └── dto/            # Input/Output DTOs
//! ├── infrastructure/     # Repository interfaces
//! │   └── repository/     # PermissionConfigRepository trait
//! └── interfaces/         # API contracts
//!     └── http/           # REST endpoint contracts
//! ```
//!
//! # Contract Freeze Notice
//!
//! ALL files in this module are frozen contracts.
//! - No implementation changes without explicit contract change approval
//! - Implementation PRs MUST reference these interfaces
//! - DTO schemas serve as the canonical data contract
//!
//! # Related Components
//!
//! - `ExecutionEnforcer` (in `crate::enforcement::application::service`) handles
//!   execution hard caps (retries, time, tool calls)
//! - `PermissionEnforcer` (this module) handles mode-based permission gating
//! - Both are used by the execution engine for different enforcement concerns

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
