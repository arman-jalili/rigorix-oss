//! Domain layer for the Execution Tools bounded context.
//!
//! @canonical .pi/architecture/modules/execution-tools.md#domain
//! Implements: Contract Freeze — EngineFacade trait, value objects, events, error types
//!
//! This module defines the core domain types for Execution Tools:
//! - Aggregate root: `EngineFacade` trait
//! - Value objects: `PlanTemplate`, `StepDefinition`, `ExecutionResult`,
//!   `ValidationResult`, `EnforcementStatus`, `CostBreakdown`, etc.
//! - Domain events: `ExecutionToolsEvent`
//! - Error types: `EngineFacadeError`, `HandlerError`, `PlanTemplateError`
//!
//! These are pure domain types with no framework dependencies. They serve as
//! the frozen contract that all implementation must satisfy.
//!
//! # Contract (Frozen)
//!
//! - No implementation logic beyond constructors and field accessors
//! - All types are serializable (Serialize + Deserialize)
//! - Value objects are immutable (no pub fields, no setters)
//! - EngineFacade exposes trait methods, not pub fields
//! - Domain events carry execution_id/session_id and timestamp

pub mod entity;
pub mod error;
pub mod event;
pub mod value;

pub use entity::{EngineFacade, SharedEngineFacade};
pub use error::{EngineFacadeError, HandlerError, ToolCallResult, ToolContentItem};
pub use event::ExecutionToolsEvent;
pub use value::{
    BudgetStatus, CircuitBreakerStatus, Constraints, CostBreakdown, CostEstimate,
    EnforcementStatus, ExecutionId, ExecutionResult, ExecutionStatus, PlanTemplate,
    PlanTemplateError, StepCost, StepDefinition, StepResult, ValidationResult,
};
