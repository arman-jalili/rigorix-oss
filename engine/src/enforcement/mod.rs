//! Enforcement bounded context.
//!
//! @canonical .pi/architecture/modules/enforcement.md
//! Implements: Contract Freeze — enforcement module root
//! Issue: issue-contract-freeze
//!
//! Defines and enforces safety limits during execution: resource budgets
//! (tokens, tool calls, execution time), tool call policies (allow/block/
//! confirm based on risk level), and execution hard limits. The
//! `ExecutionEnforcer` sits between the executor and tool execution,
//! gating every tool call and tracking resource consumption.
//!
//! # Architecture
//!
//! ```text
//! enforcement/
//! ├── domain/           # Domain entities (EnforcementConfig), errors, events
//! │   ├── config.rs     # EnforcementConfig aggregate, ResourceBudget, ToolPolicy
//! │   ├── error.rs      # EnforcementError enum
//! │   └── event/        # EnforcementEvent payload schemas
//! ├── application/      # Service traits, DTOs, factory interfaces
//! │   ├── service.rs    # ExecutionEnforcer trait
//! │   ├── factory.rs    # ExecutionEnforcerFactory interface
//! │   └── dto/          # Input/Output DTOs with validation
//! ├── infrastructure/   # Repository interfaces
//! │   └── repository/   # EnforcementPolicyRepository trait
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
//! # Related Components
//!
//! - `EnforcementPreset` (in `crate::configuration::domain::config`) selects
//!   the preset profile used to build the `EnforcementConfig`
//! - `ExecutionEvent::ToolExecuted` and `ExecutionEvent::BudgetWarning`
//!   (in `crate::event_system::domain::event`) are emitted by the enforcer

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;

#[cfg(test)]
pub mod tests;
