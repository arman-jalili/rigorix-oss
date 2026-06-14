//! Application layer interfaces and implementations for the Budget Tracking bounded context.
//!
//! @canonical .pi/architecture/modules/budget-tracking.md
//! Implements: Contract Freeze — service traits, DTOs, factory interfaces
//! Implementation: Issue #69 — LlmBudget, Issue #70 — LlmBudgetReservation
//!
//! This module defines:
//! - Service traits (use cases / application services)
//! - Input/Output DTOs with validation
//! - Factory interfaces for constructing domain objects
//! - Concrete implementations of all service and factory traits

pub mod dto;
pub mod factory;
pub mod llm_budget_factory_impl;
pub mod llm_budget_impl;
pub mod service;

pub use dto::*;
pub use factory::*;
pub use llm_budget_factory_impl::*;
pub use llm_budget_impl::*;
pub use service::*;
