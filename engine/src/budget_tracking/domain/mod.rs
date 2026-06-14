//! Domain entities and interfaces for the Budget Tracking bounded context.
//!
//! @canonical .pi/architecture/modules/budget-tracking.md#domain
//! Implements: Contract Freeze — domain entities LlmBudget, LlmBudgetReservation,
//!              LlmBudgetError, BudgetEvent
//! Issue: #68
//!
//! This module defines the core domain types — `LlmBudget`, `LlmBudgetReservation`,
//! `LlmBudgetError`, and all budget-related events. These are pure domain objects
//! with no framework dependencies. They serve as the frozen contract that all
//! implementation must satisfy.
//!
//! # Contract Freeze
//! - No implementation logic beyond constructors and field accessors
//! - All validation must happen in the application layer (service traits)
//! - All persistence must happen behind repository interfaces

pub mod budget;
pub mod error;
pub mod event;
pub mod reservation;

pub use budget::*;
pub use error::LlmBudgetError;
pub use reservation::LlmBudgetReservationState;
