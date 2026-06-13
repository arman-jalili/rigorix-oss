//! Application layer interfaces for the Enforcement bounded context.
//!
//! @canonical .pi/architecture/modules/enforcement.md
//! Implements: Contract Freeze — service traits, DTOs, factory interfaces
//! Issue: issue-contract-freeze
//!
//! This module defines:
//! - Service traits (use cases / application services)
//! - Input/Output DTOs with validation
//! - Factory interfaces for constructing ExecutionEnforcer instances
//!
//! # Contract (Frozen)
//! - All service methods are async (return `impl Future`)
//! - All public methods return `Result<_, EnforcementError>`
//! - DTOs include validation annotations/documentation
//! - No implementation logic — only trait definitions

pub mod config_service_impl;
pub mod dto;
pub mod factory;
pub mod service;

pub use config_service_impl::*;
pub use dto::*;
pub use factory::*;
pub use service::*;
