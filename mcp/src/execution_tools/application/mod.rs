//! Application layer for the Execution Tools bounded context.
//!
//! @canonical .pi/architecture/modules/execution-tools.md#application
//! Implements: Contract Freeze — service traits, DTOs, factory interfaces
//!
//! This module defines:
//! - Service traits (use cases): ExecuteHandler, ValidatePlanHandler, CheckEnforcementHandler
//! - Input/Output DTOs with validation documentation
//! - Factory interfaces for constructing domain objects
//!
//! # Contract (Frozen)
//!
//! - All service methods are async (use async-trait)
//! - All public methods return domain error types
//! - DTOs include validation documentation
//! - Factory interfaces encapsulate complex construction

pub mod dto;
pub mod factory;
pub mod service;
pub mod service_impl;

pub use dto::*;
pub use factory::*;
pub use service::*;
