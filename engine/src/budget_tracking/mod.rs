//! Budget Tracking bounded context.
//!
//! @canonical .pi/architecture/modules/budget-tracking.md
//! Implements: Contract Freeze — budget-tracking module root
//! Issue: #68
//!
//! Monitors and enforces LLM token and call budgets per execution with RAII
//! reservation pattern. Every LLM call must reserve budget before invocation;
//! unreserved budget auto-rolls back on Drop. CancellationToken coordinates
//! budget exhaustion with the cancellation system.
//!
//! # Architecture
//!
//! ```text
//! budget_tracking/
//! ├── domain/           # Domain entities (LlmBudget, LlmBudgetReservation), errors, events
//! │   ├── budget.rs     # LlmBudget value object with usage tracking
//! │   ├── reservation.rs# LlmBudgetReservation RAII guard entity
//! │   ├── error.rs      # LlmBudgetError enum
//! │   └── event/        # BudgetEvent payload schemas
//! ├── application/      # Service traits, DTOs, factory interfaces
//! │   ├── service.rs    # LlmBudgetService, LlmBudgetReservation traits
//! │   ├── factory.rs    # LlmBudgetFactory trait
//! │   └── dto/          # Input/Output DTOs with validation
//! ├── infrastructure/   # Repository interfaces
//! │   └── repository/   # LlmBudgetRepository trait
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
