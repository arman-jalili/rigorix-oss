//! Planning domain types — error types, event schemas.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md
//! Implements: Contract Freeze — PlanningCliError, PlanningCliEvent
//! Issue: issue-contract-freeze

pub mod error;
pub mod event;

pub use error::PlanningCliError;
pub use event::PlanningCliEvent;
